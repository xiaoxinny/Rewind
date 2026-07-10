//! The session/break state machine — the heart of Rewind (DP-1).
//!
//! See implementation plan §7e for the full transition table. Timers
//! are stored as **target monotonic timestamps**, not decrementing
//! counters — so a fake clock can jump time instantly in tests.
//!
//! ## Transition table (§7e, concise)
//!
//! ```text
//! Focus         micro timer expires → PreBreak{Micro} or MicroBreak
//! Focus         rest  timer expires → PreBreak{Rest}  or RestBreak
//! PreBreak      countdown ends      → MicroBreak / RestBreak
//! Micro/RestBreak  duration elapses → Focus        (DismissBreak; re-arm)
//! Micro/RestBreak  SkipBreak         → Focus        (DismissBreak; re-arm)
//! PreBreak/break  PostponeBreak      → Postponed    (DismissBreak)
//! Postponed     until reached       → PreBreak/break of same kind
//! Focus/Pre/Post idle > pause        → Paused{Idle}
//! any           PauseToggle         → Paused{Manual} ↔ resume
//! Paused{Idle}  user returns        → resume / reset
//! Micro/Rest    user idles full     → Focus        (BreakRecord{natural})
//! ```
//!
//! Methods on the machine emit **sub-events** (`SessionEvent`) used by
//! the engine to construct `CoreEvent`s. The machine itself never
//! touches the OS, never reads the clock, never holds an adapter —
//! caller-driven, deterministic.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::clock::Millis;
use crate::config::AppConfig;
use crate::session::state::{BreakKind, PauseReason, SessionState, Strictness};

/// Sub-events emitted by the state machine. The engine wraps these in
/// `CoreEvent` for the shell to forward to the frontend / adapters.
/// Keeping a small private variant lets us build up a "logical event"
/// without coupling the machine to the public `CoreEvent` shape (the
/// engine maps `ShowBreak { kind, presentation }`, `DismissBreak`,
/// `StateChanged`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    /// The high-level state changed.
    StateChanged(SessionState),
    /// A break is now visible to the user (overlay/banner).
    ShowBreak {
        kind: BreakKind,
        presentation_strict: bool,
    },
    /// The active break is no longer visible.
    DismissBreak,
    /// A `BreakRecord` should be logged with this outcome.
    BreakFinished {
        kind: BreakKind,
        outcome: BreakOutcome,
    },
    /// Tray tooltip text (always emitted on tick for the shell to
    /// forward as `CoreEvent::TrayStatus`).
    TrayLine(String),
}

/// How a break ended. Matches the SQLite TEXT class in `BreakOutcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakOutcome {
    Completed,
    Skipped,
    Postponed,
    Natural,
}

impl From<BreakOutcome> for crate::model::break_record::BreakOutcome {
    fn from(o: BreakOutcome) -> Self {
        use crate::model::break_record::BreakOutcome as Bo;
        match o {
            BreakOutcome::Completed => Bo::Completed,
            BreakOutcome::Skipped => Bo::Skipped,
            BreakOutcome::Postponed => Bo::Postponed,
            BreakOutcome::Natural => Bo::Natural,
        }
    }
}

/// Target monotonic timestamps for the currently armed timers. The
/// machine compares these against `now` on each tick. They're
/// `Option` because not every state holds every timer (e.g. inside a
/// break, micro/rest intervals are paused).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Timers {
    /// When the next micro-break should start firing (entering
    /// `PreBreak{Micro}` or `MicroBreak`).
    pub(crate) next_micro_at: Option<Millis>,
    /// When the next rest-break should start firing.
    pub(crate) next_rest_at: Option<Millis>,
    /// When the current `PreBreak` countdown ends.
    pre_break_ends_at: Option<Millis>,
    /// When the current `MicroBreak` or `RestBreak` duration ends.
    break_ends_at: Option<Millis>,
    /// When a postponed break should re-arm.
    postponed_until: Option<Millis>,
}

impl Timers {
    fn clear_break_timers(&mut self) {
        self.pre_break_ends_at = None;
        self.break_ends_at = None;
    }
}

/// The state machine. Public surface: `new`, `tick`, `handle(cmd)`,
/// `state`, `remaining`, `postpone_count_today`. All timers are
/// relative to the supplied `now` — the machine is stateless w.r.t.
/// the real clock.
#[derive(Debug, Clone)]
pub struct SessionMachine {
    state: SessionState,
    pub(crate) timers: Timers,
    config: AppConfig,
    /// Number of postposes used in the current break-in-flight. Resets
    /// when we transition back to `Focus`.
    postpone_count: u32,
    /// Last monotonic time the machine was ticked or had a command
    /// applied. Used to compute remaining time without absolute
    /// timestamps leaking out.
    last_mono: Millis,
}

impl SessionMachine {
    /// Construct a fresh machine in `Focus` with both micro & rest
    /// timers armed from `now`.
    pub fn new(now: Millis, config: &AppConfig) -> Self {
        let micro = config.breaks.micro_interval().as_millis() as Millis;
        let rest = config.breaks.rest_interval().as_millis() as Millis;
        let mut m = Self {
            state: SessionState::Focus,
            timers: Timers {
                next_micro_at: Some(now.saturating_add(micro)),
                next_rest_at: Some(now.saturating_add(rest)),
                ..Timers::default()
            },
            config: config.clone(),
            postpone_count: 0,
            last_mono: now,
        };
        // Emit the initial StateChanged so subscribers wire up.
        // The machine itself emits via `tick` — but `new` is silent;
        // the engine is expected to take the state into account on
        // its first tick.
        let _ = m.last_mono;
        m
    }

    /// Read-only access to the current state. Cheap; copies the enum.
    pub fn state(&self) -> SessionState {
        self.state.clone()
    }

    /// How long until the next **user-visible** event for this state:
    /// either the `PreBreak` countdown ends, the active break ends,
    /// the postponed delay fires, or the next micro/rest break
    /// starts. Returns `None` for `Focus` (no event pending) and
    /// for `Paused`/`Manual` (frozen).
    pub fn remaining(&self, now: Millis) -> Duration {
        let now = now.max(self.last_mono);
        let candidates: [Option<Millis>; 4] = [
            self.timers.pre_break_ends_at,
            self.timers.break_ends_at,
            self.timers.postponed_until,
            match self.state {
                SessionState::Focus => Some(
                    self.timers
                        .next_micro_at
                        .unwrap_or(now)
                        .min(self.timers.next_rest_at.unwrap_or(now)),
                ),
                _ => None,
            },
        ];
        let mut best = None::<Millis>;
        for c in candidates.into_iter().flatten() {
            if c >= now {
                best = Some(match best {
                    None => c,
                    Some(b) => b.min(c),
                });
            }
        }
        match best {
            Some(target) => Duration::from_millis(target - now),
            None => Duration::ZERO,
        }
    }

    /// How many postposes have been used for the current break chain
    /// (resets when we land back in `Focus`). Used by the engine /
    /// IPC layer to gate further `PostponeBreak` requests.
    pub fn postpone_count(&self) -> u32 {
        self.postpone_count
    }

    /// Driver: advance to `now` and return the sub-events the engine
    /// should turn into `CoreEvent`s. `idle` is the latest idle
    /// reading — used to satisfy a break "naturally" if the user has
    /// been away for the full break duration.
    pub fn tick(&mut self, now: Millis, idle: Duration) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        let _ = self.last_mono;
        self.last_mono = now;

        match &self.state {
            SessionState::Focus => {
                // Both micro and rest timers might fire; pick whichever
                // is due first. Micro beats rest at the same target
                // (more frequent = more important to the user).
                let micro_due = self
                    .timers
                    .next_micro_at
                    .map(|t| now >= t)
                    .unwrap_or(false);
                let rest_due = self
                    .timers
                    .next_rest_at
                    .map(|t| now >= t)
                    .unwrap_or(false);
                if micro_due || rest_due {
                    // Prefer rest if it's due and (micro not due OR
                    // rest is sooner). For M1 simplicity: prefer rest
                    // if due, else micro.
                    let kind = if rest_due {
                        BreakKind::Rest
                    } else {
                        BreakKind::Micro
                    };
                    self.enter_pre_break(kind, now, &mut events);
                }
            }

            SessionState::PreBreak {
                kind,
                remaining_ms: _,
            } => {
                let ends = self.timers.pre_break_ends_at.unwrap_or(now);
                if now >= ends {
                    let kind = *kind;
                    self.transition_to_break(kind, now, &mut events);
                }
            }

            SessionState::MicroBreak { .. } | SessionState::RestBreak { .. } => {
                // Check natural satisfaction first (§7e last row).
                let micro_dur = self.config.breaks.micro_duration().as_secs();
                let rest_dur = self.config.breaks.rest_duration().as_secs();
                let break_dur = match self.state {
                    SessionState::MicroBreak { .. } => micro_dur,
                    SessionState::RestBreak { .. } => rest_dur,
                    _ => 0,
                };
                if !matches!(self.state, SessionState::Focus | SessionState::Paused { .. })
                    && idle.as_secs() >= break_dur
                {
                    let kind = self.state.break_kind().unwrap_or(BreakKind::Micro);
                    self.end_break(kind, BreakOutcome::Natural, now, &mut events);
                    return events;
                }

                let ends = self.timers.break_ends_at.unwrap_or(now);
                if now >= ends {
                    let kind = self.state.break_kind().unwrap_or(BreakKind::Micro);
                    self.end_break(kind, BreakOutcome::Completed, now, &mut events);
                }
            }

            SessionState::Postponed { kind, until_ms: _ } => {
                let until = self.timers.postponed_until.unwrap_or(now);
                if now >= until {
                    let kind = *kind;
                    self.enter_pre_break(kind, now, &mut events);
                }
            }

            SessionState::Paused { .. } => {
                // Frozen; nothing to do here — the engine handles
                // resume/reset via the idle policy.
            }
        }

        // Emit the "current state" tray line whenever the machine
        // ticked. Cheap; always there.
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));

        events
    }

    // -----------------------------------------------------------------
    // Commands — fed from `CoreCommand` after the engine maps.
    // -----------------------------------------------------------------

    /// Manual pause / resume toggle. Starts and ends `Paused{Manual}`.
    /// `now` is recorded but not used by the timer (frozen).
    pub fn pause_toggle(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        let new_state = match &self.state {
            SessionState::Paused { reason: PauseReason::Manual } => SessionState::Focus,
            // From anything else (Focus, PreBreak, break, Postponed,
            // Paused{Idle}) → Paused{Manual}.
            _ => SessionState::Paused {
                reason: PauseReason::Manual,
            },
        };
        // If we're entering Paused, freeze timers (no-op internally —
        // timers aren't decremented, so "freeze" means "we don't
        // compare against them"). If leaving Paused, snap to Focus
        // and re-arm if needed.
        let prev = std::mem::replace(&mut self.state, new_state.clone());
        if matches!(new_state, SessionState::Focus) && matches!(prev, SessionState::Paused { .. }) {
            self.arm_focus_timers(now);
        }
        events.push(SessionEvent::StateChanged(new_state));
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));
        events
    }

    /// Skip the current `Micro/RestBreak` or pending `PreBreak`.
    /// Disallowed under `Strict` strictness.
    pub fn skip_break(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        if !self.config.strictness.allows_skip() {
            // Silent no-op under Strict — the engine logs a refusal
            // if it wants to.
            return events;
        }
        let kind = match &self.state {
            SessionState::MicroBreak { .. } => Some(BreakKind::Micro),
            SessionState::RestBreak { .. } => Some(BreakKind::Rest),
            SessionState::PreBreak { kind, .. } => Some(*kind),
            _ => None,
        };
        if let Some(kind) = kind {
            self.end_break(kind, BreakOutcome::Skipped, now, &mut events);
        }
        events
    }

    /// Postpone the current/pending break by `postponeDuration`.
    /// Bounded by `maxPostpones`.
    pub fn postpone(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        if self.postpone_count >= self.config.breaks.max_postpones {
            // Refused.
            return events;
        }
        let kind = match &self.state {
            SessionState::MicroBreak { .. } => Some(BreakKind::Micro),
            SessionState::RestBreak { .. } => Some(BreakKind::Rest),
            SessionState::PreBreak { kind, .. } => Some(*kind),
            _ => None,
        };
        if let Some(kind) = kind {
            self.postpone_count = self.postpone_count.saturating_add(1);
            let delay = self.config.breaks.postpone_duration().as_millis() as Millis;
            self.timers.clear_break_timers();
            self.timers.postponed_until = Some(now.saturating_add(delay));
            let new = SessionState::Postponed {
                kind,
                until_ms: self.timers.postponed_until.unwrap(),
            };
            self.state = new.clone();
            events.push(SessionEvent::DismissBreak);
            events.push(SessionEvent::StateChanged(new));
            events.push(SessionEvent::TrayLine(format!(
                "{} | {}",
                self.state.label(),
                self.format_tooltip(now)
            )));
        }
        events
    }

    /// Called by the engine when an `IdleAction::Pause` decision is
    /// made. Transitions to `Paused{Idle}`.
    pub fn idle_pause(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        if matches!(self.state, SessionState::Paused { .. }) {
            return events;
        }
        // Preserve any pre-existing `DismissBreak` semantics: if
        // a break is in flight we drop it without logging an
        // outcome — the user wasn't there.
        if matches!(self.state, SessionState::MicroBreak { .. } | SessionState::RestBreak { .. }) {
            events.push(SessionEvent::DismissBreak);
            self.timers.clear_break_timers();
        }
        let new = SessionState::Paused {
            reason: PauseReason::Idle,
        };
        self.state = new.clone();
        events.push(SessionEvent::StateChanged(new));
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));
        events
    }

    /// Called by the engine when an `IdleAction::Resume` decision is
    /// made: re-arm focus timers and return to `Focus`.
    pub fn idle_resume(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        if !matches!(
            self.state,
            SessionState::Paused {
                reason: PauseReason::Idle
            }
        ) {
            return events;
        }
        self.state = SessionState::Focus;
        self.arm_focus_timers(now);
        events.push(SessionEvent::StateChanged(SessionState::Focus));
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));
        events
    }

    /// Called by the engine when an `IdleAction::Reset` decision is
    /// made: return to `Focus` *and* re-arm focus timers from
    /// scratch (treating the absence as a natural break).
    pub fn idle_reset(&mut self, now: Millis) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        if !matches!(
            self.state,
            SessionState::Paused {
                reason: PauseReason::Idle
            }
        ) {
            return events;
        }
        self.state = SessionState::Focus;
        self.arm_focus_timers(now);
        self.postpone_count = 0;
        events.push(SessionEvent::StateChanged(SessionState::Focus));
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));
        events
    }

    /// Live config swap (`CoreCommand::ConfigUpdated`). Re-arms timers
    /// if the new intervals differ.
    pub fn update_config(&mut self, now: Millis, config: AppConfig) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.last_mono = now;
        self.config = config;
        // If we're currently in Focus, re-arm so the new intervals
        // apply. Inside a break or paused, the live config takes
        // effect on the next state-arm.
        if matches!(self.state, SessionState::Focus) {
            self.arm_focus_timers(now);
        }
        events.push(SessionEvent::TrayLine(format!(
            "{} | {}",
            self.state.label(),
            self.format_tooltip(now)
        )));
        events
    }

    /// Convenience: replace the strictness field without rebuilding
    /// the whole config.
    pub fn set_strictness(&mut self, s: Strictness) {
        self.config.strictness = s;
    }

    // -----------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------

    fn arm_focus_timers(&mut self, now: Millis) {
        let micro = self.config.breaks.micro_interval().as_millis() as Millis;
        let rest = self.config.breaks.rest_interval().as_millis() as Millis;
        self.timers.next_micro_at = Some(now.saturating_add(micro));
        self.timers.next_rest_at = Some(now.saturating_add(rest));
        self.postpone_count = 0;
    }

    fn enter_pre_break(&mut self, kind: BreakKind, now: Millis, events: &mut Vec<SessionEvent>) {
        // Clear the focus timer that fired (it'll be re-armed when
        // we land back in Focus).
        match kind {
            BreakKind::Micro => self.timers.next_micro_at = None,
            BreakKind::Rest => self.timers.next_rest_at = None,
        }
        let pre = self.config.breaks.pre_break_duration();
        match pre {
            None => {
                // Skip the PreBreak countdown entirely.
                self.transition_to_break(kind, now, events);
            }
            Some(dur) => {
                let ends = now.saturating_add(dur.as_millis() as Millis);
                self.timers.pre_break_ends_at = Some(ends);
                let new = SessionState::PreBreak {
                    kind,
                    remaining_ms: dur.as_millis() as Millis,
                };
                self.state = new.clone();
                events.push(SessionEvent::StateChanged(new));
            }
        }
    }

    fn transition_to_break(
        &mut self,
        kind: BreakKind,
        now: Millis,
        events: &mut Vec<SessionEvent>,
    ) {
        let dur = match kind {
            BreakKind::Micro => self.config.breaks.micro_duration(),
            BreakKind::Rest => self.config.breaks.rest_duration(),
        };
        let ends = now.saturating_add(dur.as_millis() as Millis);
        self.timers.pre_break_ends_at = None;
        self.timers.break_ends_at = Some(ends);
        let new = match kind {
            BreakKind::Micro => SessionState::MicroBreak {
                remaining_ms: dur.as_millis() as Millis,
            },
            BreakKind::Rest => SessionState::RestBreak {
                remaining_ms: dur.as_millis() as Millis,
            },
        };
        self.state = new.clone();
        let strict = matches!(self.config.strictness, Strictness::Strict);
        events.push(SessionEvent::ShowBreak {
            kind,
            presentation_strict: strict,
        });
        events.push(SessionEvent::StateChanged(new));
    }

    fn end_break(
        &mut self,
        kind: BreakKind,
        outcome: BreakOutcome,
        now: Millis,
        events: &mut Vec<SessionEvent>,
    ) {
        self.timers.clear_break_timers();
        self.state = SessionState::Focus;
        self.arm_focus_timers(now);
        events.push(SessionEvent::DismissBreak);
        events.push(SessionEvent::BreakFinished { kind, outcome });
        events.push(SessionEvent::StateChanged(SessionState::Focus));
    }

    /// Format the tooltip string used by the tray. Delegated to
    /// `CoreEvent::TrayStatus` constructors on the engine side; this
    /// is a short label for the machine log.
    fn format_tooltip(&self, now: Millis) -> String {
        match self.state {
            SessionState::Focus => {
                let micro = self.timers.next_micro_at.unwrap_or(now);
                let rest = self.timers.next_rest_at.unwrap_or(now);
                let next = micro.min(rest);
                let secs = next.saturating_sub(now) / 1_000;
                format!("next break {secs}s")
            }
            SessionState::PreBreak { remaining_ms, .. } => {
                format!("start in {}s", remaining_ms / 1_000)
            }
            SessionState::MicroBreak { remaining_ms } => {
                format!("micro {}s", remaining_ms / 1_000)
            }
            SessionState::RestBreak { remaining_ms } => {
                format!("rest {}s", remaining_ms / 1_000)
            }
            SessionState::Postponed { until_ms, .. } => {
                let secs = until_ms.saturating_sub(now) / 1_000;
                format!("postponed, resume in {secs}s")
            }
            SessionState::Paused { reason } => match reason {
                PauseReason::Manual => "paused (you)".to_string(),
                PauseReason::Idle => "paused (idle)".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn default_cfg() -> AppConfig {
        AppConfig::default()
    }

    #[test]
    fn starts_in_focus_with_armed_timers() {
        let m = SessionMachine::new(0, &default_cfg());
        assert!(matches!(m.state(), SessionState::Focus));
        assert!(m.timers.next_micro_at.is_some());
        assert!(m.timers.next_rest_at.is_some());
    }

    #[test]
    fn focus_to_micro_break_full_cycle() {
        // 20-min micro interval, 10-sec pre-break, 20-sec break.
        let mut m = SessionMachine::new(0, &default_cfg());
        // Advance to t = 20 min — micro timer fires.
        let at = 20 * 60 * 1_000;
        let events = m.tick(at, Duration::ZERO);
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::StateChanged(SessionState::PreBreak { kind: BreakKind::Micro, .. })
        )));

        // Advance through the pre-break countdown (10s).
        let events = m.tick(at + 11_000, Duration::ZERO);
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::StateChanged(SessionState::MicroBreak { .. })
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::ShowBreak {
                kind: BreakKind::Micro,
                ..
            }
        )));

        // Advance through the 20-sec break to completion.
        let events = m.tick(at + 11_000 + 20_000, Duration::ZERO);
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::BreakFinished {
                kind: BreakKind::Micro,
                outcome: BreakOutcome::Completed
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::DismissBreak)));
        assert!(matches!(m.state(), SessionState::Focus));
        // After completing, micro timer is re-armed.
        assert!(m.timers.next_micro_at.is_some());
    }

    #[test]
    fn pre_break_countdown_zero_remaining_to_break() {
        // Edge: pre-break ends exactly at `now`.
        let mut m = SessionMachine::new(0, &default_cfg());
        // Simulate reaching pre-break end.
        let pre_end = 20 * 60 * 1_000 + 10_000;
        m.timers.next_micro_at = Some(20 * 60 * 1_000);
        m.timers.pre_break_ends_at = Some(pre_end);
        m.state = SessionState::PreBreak {
            kind: BreakKind::Micro,
            remaining_ms: 0,
        };
        let events = m.tick(pre_end, Duration::ZERO);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::ShowBreak { .. })));
        assert!(matches!(m.state(), SessionState::MicroBreak { .. }));
    }

    #[test]
    fn strict_disallows_skip() {
        let mut cfg = default_cfg();
        cfg.strictness = Strictness::Strict;
        let mut m = SessionMachine::new(0, &cfg);
        // Drop us into MicroBreak directly.
        m.state = SessionState::MicroBreak {
            remaining_ms: 10_000,
        };
        let events = m.skip_break(0);
        assert!(events.is_empty());
        assert!(matches!(m.state(), SessionState::MicroBreak { .. }));
    }

    #[test]
    fn gentle_allows_skip_and_logs_skipped() {
        let mut m = SessionMachine::new(0, &default_cfg());
        m.state = SessionState::MicroBreak { remaining_ms: 10_000 };
        let events = m.skip_break(0);
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::BreakFinished {
                kind: BreakKind::Micro,
                outcome: BreakOutcome::Skipped
            }
        )));
        assert!(matches!(m.state(), SessionState::Focus));
    }

    #[test]
    fn postpone_respects_max_postpones() {
        let mut cfg = default_cfg();
        cfg.breaks.max_postpones = 2;
        let mut m = SessionMachine::new(0, &cfg);
        m.state = SessionState::PreBreak {
            kind: BreakKind::Micro,
            remaining_ms: 5_000,
        };
        m.timers.pre_break_ends_at = Some(0);

        // 1st postpone: ok.
        let events = m.postpone(0);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::StateChanged(SessionState::Postponed { .. }))));
        assert_eq!(m.postpone_count(), 1);

        // After timer expires, we land back in PreBreak.
        let until = m.timers.postponed_until.unwrap();
        m.tick(until + 1, Duration::ZERO);
        assert!(matches!(m.state(), SessionState::PreBreak { .. }));

        // 2nd postpone: ok.
        let events = m.postpone(0);
        assert_eq!(m.postpone_count(), 2);

        // After timer expires.
        let until = m.timers.postponed_until.unwrap();
        m.tick(until + 1, Duration::ZERO);
        assert!(matches!(m.state(), SessionState::PreBreak { .. }));

        // 3rd postpone: refused (max_postpones = 2 means we already
        // used 2).
        let events = m.postpone(0);
        assert!(events.is_empty());
        assert_eq!(m.postpone_count(), 2);
    }

    #[test]
    fn postpone_count_resets_after_returning_to_focus() {
        let mut cfg = default_cfg();
        cfg.breaks.max_postpones = 3;
        let mut m = SessionMachine::new(0, &default_cfg());
        m.state = SessionState::PreBreak {
            kind: BreakKind::Micro,
            remaining_ms: 5_000,
        };
        m.timers.pre_break_ends_at = Some(0);
        let _ = m.postpone(0);
        assert_eq!(m.postpone_count(), 1);
        // Land back in Focus.
        let until = m.timers.postponed_until.unwrap();
        m.tick(until + 1, Duration::ZERO);
        m.tick(until + 1 + 11_000, Duration::ZERO);
        // Now in MicroBreak; let it complete.
        m.tick(until + 1 + 11_000 + 20_000, Duration::ZERO);
        assert!(matches!(m.state(), SessionState::Focus));
        assert_eq!(m.postpone_count(), 0);
    }

    #[test]
    fn manual_pause_and_resume() {
        let mut m = SessionMachine::new(0, &default_cfg());
        let events = m.pause_toggle(100);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::StateChanged(SessionState::Paused { reason: PauseReason::Manual }))));
        let events = m.pause_toggle(200);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::StateChanged(SessionState::Focus))));
    }

    #[test]
    fn manual_pause_during_break_releases_back_to_focus_with_rearm() {
        let mut m = SessionMachine::new(0, &default_cfg());
        m.state = SessionState::MicroBreak { remaining_ms: 5_000 };
        m.timers.break_ends_at = Some(5_000);
        let _ = m.pause_toggle(0);
        assert!(matches!(
            m.state(),
            SessionState::Paused {
                reason: PauseReason::Manual
            }
        ));
        let _ = m.pause_toggle(6_000);
        assert!(matches!(m.state(), SessionState::Focus));
        assert!(m.timers.next_micro_at.is_some());
        assert!(m.timers.next_rest_at.is_some());
    }

    #[test]
    fn idle_pause_then_resume_returns_to_focus() {
        let mut m = SessionMachine::new(0, &default_cfg());
        let _ = m.idle_pause(0);
        assert!(matches!(
            m.state(),
            SessionState::Paused {
                reason: PauseReason::Idle
            }
        ));
        let _ = m.idle_resume(1_000);
        assert!(matches!(m.state(), SessionState::Focus));
    }

    #[test]
    fn idle_pause_during_break_drops_overlay_with_no_outcome() {
        let mut m = SessionMachine::new(0, &default_cfg());
        m.state = SessionState::MicroBreak { remaining_ms: 5_000 };
        m.timers.break_ends_at = Some(5_000);
        let events = m.idle_pause(0);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::DismissBreak)));
        // No BreakFinished event — the user wasn't there to finish.
        assert!(!events
            .iter()
            .any(|e| matches!(e, SessionEvent::BreakFinished { .. })));
        assert!(matches!(
            m.state(),
            SessionState::Paused {
                reason: PauseReason::Idle
            }
        ));
    }

    #[test]
    fn idle_reset_rearms_focus_timers() {
        let mut m = SessionMachine::new(0, &default_cfg());
        let _ = m.idle_pause(0);
        // Pretend we were paused and now resetting.
        let events = m.idle_reset(10_000);
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::StateChanged(SessionState::Focus))));
        assert!(m.timers.next_micro_at.is_some());
        // Reset clears postpone counter.
        let mut cfg = default_cfg();
        cfg.breaks.max_postpones = 3;
        m.update_config(20_000, cfg);
        m.state = SessionState::PreBreak {
            kind: BreakKind::Micro,
            remaining_ms: 5_000,
        };
        m.timers.pre_break_ends_at = Some(25_000);
        m.postpone_count = 2;
        let _ = m.idle_pause(30_000);
        let _ = m.idle_reset(31_000);
        assert_eq!(m.postpone_count(), 0);
    }

    #[test]
    fn natural_idle_satisfies_micro_break() {
        let mut m = SessionMachine::new(0, &default_cfg());
        m.state = SessionState::MicroBreak { remaining_ms: 1_000 };
        m.timers.break_ends_at = Some(20_000);
        // idle = 25s (>= 20s micro duration) → natural satisfaction.
        let events = m.tick(15_000, Duration::from_secs(25));
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::BreakFinished {
                kind: BreakKind::Micro,
                outcome: BreakOutcome::Natural
            }
        )));
    }

    #[test]
    fn remaining_returns_zero_for_focus_when_no_focus_event_imminent_or_nonzero() {
        let mut m = SessionMachine::new(0, &default_cfg());
        // right after construction: 20 min from now to micro fire.
        // (At t=0 we *can* report, since the next arm is at t+20m.)
        let r = m.remaining(0);
        assert!(r > Duration::ZERO);
        // Reach the micro-fire time exactly: zero remaining until pre-break end.
        let next = m.timers.next_micro_at.unwrap();
        m.tick(next, Duration::ZERO); // enter PreBreak
        // Now we're in PreBreak with remaining_ms = 10s.
        if let SessionState::PreBreak { remaining_ms, .. } = m.state() {
            assert_eq!(Duration::from_millis(remaining_ms), Duration::from_secs(10));
        } else {
            panic!("expected PreBreak");
        }
    }

    #[test]
    fn pre_break_disabled_skips_to_break() {
        let mut cfg = default_cfg();
        cfg.breaks.pre_break_warn = false;
        let mut m = SessionMachine::new(0, &cfg);
        let at = 20 * 60 * 1_000;
        let events = m.tick(at, Duration::ZERO);
        // No PreBreak state.
        assert!(!events.iter().any(|e| matches!(
            e,
            SessionEvent::StateChanged(SessionState::PreBreak { .. })
        )));
        // Direct to MicroBreak.
        assert!(matches!(m.state(), SessionState::MicroBreak { .. }));
        assert!(events
            .iter()
            .any(|e| matches!(e, SessionEvent::ShowBreak { .. })));
    }

    #[test]
    fn update_config_rearms_when_in_focus() {
        let mut m = SessionMachine::new(0, &default_cfg());
        let mut cfg2 = default_cfg();
        cfg2.breaks.micro_interval_min = 5;
        let _ = m.update_config(1_000, cfg2);
        // New interval: 5 min from t=1_000 → arm at 5*60_000 + 1_000 = 301_000.
        let expected = 5 * 60 * 1_000 + 1_000;
        assert_eq!(m.timers.next_micro_at, Some(expected));
    }
}
