//! `Engine` — owns the `SessionMachine`, the `ReminderCoordinator`,
//! the hydration & posture schedulers, and the current `AppConfig`.
//!
//! Two methods only:
//!   * `tick(now: Timestamp, idle: Duration) -> Vec<CoreEvent>`
//!   * `handle(cmd: CoreCommand) -> Vec<CoreEvent>`
//!
//! Timers are stored as **target monotonic timestamps**, not
//! decrementing counters, so a fake clock can jump time instantly in
//! tests.

use std::time::Duration;

use crate::clock::{Millis, Timestamp};
use crate::config::AppConfig;
use crate::events::{BreakPresentation, CoreCommand, CoreEvent, HydrationProgress, TrayStatus};
use crate::idle::policy as idle_policy;
use crate::idle::IdleAction;
use crate::scheduler::coordinator::ReminderCoordinator;
use crate::scheduler::hydration::{HydrationScheduler, HydrationSchedulerConfig};
use crate::scheduler::posture::{PostureScheduler, PostureSchedulerConfig};
use crate::scheduler::reminder::ReminderKind;
use crate::session::machine::{SessionEvent, SessionMachine};
use time::OffsetDateTime;

/// Internal key constants used to query the `SessionMachine` for
/// individual focus-timer targets. Kept private; tests don't reach
/// into them directly.
mod scheduler_key {
    pub(crate) const MICRO: u8 = 0;
    pub(crate) const REST: u8 = 1;
}

/// Convert the machine's wall timestamp into a monotonic msec
/// (single source of truth for the engine).
#[inline]
fn monotonic_ms(now: Timestamp) -> Millis {
    now.0.max(0) as Millis
}

fn hydration_scheduler_config(cfg: &AppConfig) -> HydrationSchedulerConfig {
    HydrationSchedulerConfig {
        goal_ml: cfg.hydration.goal_ml,
        glass_ml: cfg.hydration.glass_ml,
        min_gap_ms: 30 * 60 * 1_000,
    }
}

fn posture_scheduler_config(cfg: &AppConfig) -> PostureSchedulerConfig {
    PostureSchedulerConfig {
        interval_ms: cfg.posture.interval().as_millis() as Millis,
        min_gap_ms: 30 * 60 * 1_000,
    }
}

fn local_date(now: Timestamp) -> time::Date {
    let secs = now.0.div_euclid(1_000);
    let utc = OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    utc.to_offset(offset).date()
}

fn local_minute_of_day(now: Timestamp) -> u32 {
    let secs = now.0.div_euclid(1_000);
    let utc = OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let t = utc.to_offset(offset).time();
    u32::from(t.hour()) * 60 + u32::from(t.minute())
}

fn is_minute_in_window(minute: u32, start: u32, end: u32) -> bool {
    let minute = minute % (24 * 60);
    let start = start % (24 * 60);
    let end = end % (24 * 60);
    if start == end {
        false
    } else if start < end {
        (start..end).contains(&minute)
    } else {
        minute >= start || minute < end
    }
}

/// Compose the SessionMachine + schedulers + idle policy into the one
/// pure state transformer the rest of the app sees.
#[derive(Debug, Clone)]
pub struct Engine {
    machine: SessionMachine,
    coordinator: ReminderCoordinator,
    hydration: HydrationScheduler,
    posture: PostureScheduler,
    config: AppConfig,
    last_local_day: time::Date,
    /// Cached last tray status so the shell can observe identity.
    #[allow(dead_code)]
    last_tray_status: Option<TrayStatus>,
}

impl Engine {
    /// Fresh engine, with the SessionMachine in `Focus` and both
    /// micro + rest timers armed from `now`.
    pub fn new(now: Timestamp, config: AppConfig) -> Self {
        let mono = monotonic_ms(now);
        let mut posture = PostureScheduler::with_config(posture_scheduler_config(&config));
        posture.arm(mono);
        let hydration = HydrationScheduler::with_config(hydration_scheduler_config(&config));
        let machine = SessionMachine::new(mono, &config);
        let last_local_day = local_date(now);
        let mut engine = Self {
            machine,
            coordinator: ReminderCoordinator::new(),
            hydration,
            posture,
            config,
            last_local_day,
            last_tray_status: None,
        };
        engine.last_tray_status = Some(engine.compute_tray_status(now));
        engine
    }

    /// Read-only access to the engine's current high-level state.
    pub fn state(&self) -> crate::session::state::SessionState {
        self.machine.state()
    }

    /// Current `AppConfig`.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// The next user-visible event duration (for the dashboard).
    pub fn remaining(&self, now: Timestamp) -> Duration {
        self.machine.remaining(monotonic_ms(now))
    }

    /// Drive the engine with the latest observation. Returns the list
    /// of `CoreEvent`s the shell should dispatch.
    pub fn tick(&mut self, now: Timestamp, idle: Duration) -> Vec<CoreEvent> {
        let mono = monotonic_ms(now);
        let mut out = Vec::new();

        // 1. Apply the idle policy on the **current** state.
        let action = idle_policy::evaluate(idle, &self.machine.state(), &self.config.idle);
        match action {
            IdleAction::Pause => {
                let machine_evs = self.machine.idle_pause(mono);
                out.extend(map_subevents(machine_evs));
            }
            IdleAction::Resume => {
                let machine_evs = self.machine.idle_resume(mono);
                out.extend(map_subevents(machine_evs));
            }
            IdleAction::Reset => {
                let machine_evs = self.machine.idle_reset(mono);
                out.extend(map_subevents(machine_evs));
            }
            IdleAction::None => {}
        }

        // 2. Drive the SessionMachine forward.
        let machine_evs = self.machine.tick(mono, idle);
        out.extend(map_subevents(machine_evs));
        self.after_machine_events(mono, &out);

        // 3. Reminder arbitration (only when not in a break/paused/quiet hours).
        self.coordinator.clear();
        self.coordinator.set_coalesce_window(None);
        let state = self.machine.state();
        let local_day = local_date(now);
        if local_day != self.last_local_day {
            self.hydration.reset_day();
            self.last_local_day = local_day;
        }

        let paused_or_break =
            state.is_break() || matches!(state, crate::session::state::SessionState::Paused { .. });
        let in_quiet_hours = self.config.quiet_hours.enabled && {
            let (start, end) = self.config.quiet_hours_minutes();
            is_minute_in_window(local_minute_of_day(now), start, end)
        };
        self.coordinator
            .set_paused(paused_or_break || in_quiet_hours);

        if !paused_or_break {
            if let Some(rest_at) = self.machine.timer_target(scheduler_key::REST) {
                if rest_at >= mono
                    && rest_at.saturating_sub(mono) <= self.coordinator.config.coalesce_window_ms
                {
                    self.coordinator.set_coalesce_window(Some(rest_at));
                }
            }

            let waking_window = self.config.waking_window_minutes();
            if self.config.reminders.hydration {
                if let Some(r) = self.hydration.maybe_remind(mono, waking_window) {
                    self.coordinator.push(r);
                }
            }
            if self.config.reminders.posture {
                if let Some(r) = self.posture.maybe_remind(mono) {
                    self.coordinator.push(r);
                }
            }
            if let Some(rem) = self.coordinator.next(mono) {
                out.push(CoreEvent::FireReminder {
                    kind: rem.kind,
                    priority: rem.priority,
                    message: match rem.kind {
                        ReminderKind::Hydration => "Time for a glass of water".to_string(),
                        ReminderKind::Posture => "Stand up and stretch".to_string(),
                        ReminderKind::EyeBreak => "Look 20 ft away for 20 s".to_string(),
                    },
                });
                self.coordinator.mark_fired(rem.kind, mono);
                match rem.kind {
                    ReminderKind::Hydration => self.hydration.mark_reminded(mono),
                    ReminderKind::Posture => self.posture.mark_reminded(mono),
                    ReminderKind::EyeBreak => {}
                }
            }
        }

        // 4. Tray status.
        let tray = self.compute_tray_status(now);
        self.last_tray_status = Some(tray.clone());
        out.push(CoreEvent::TrayStatus(tray));

        // 5. Tick heartbeat for the frontend.
        let phase = self.machine.state();
        out.push(CoreEvent::Tick {
            phase,
            remaining: self.remaining(now),
            now: Some(now),
        });

        // Drop consecutive duplicate StateChanged events of the
        // same variant (caused by triggering both the idle policy
        // and the machine tick on the same boundary).
        dedupe_consecutive_state_changes(&mut out);
        out
    }

    /// Route a `CoreCommand` through the appropriate machine method.
    pub fn handle(&mut self, cmd: CoreCommand, now: Timestamp) -> Vec<CoreEvent> {
        let mono = monotonic_ms(now);
        match cmd {
            CoreCommand::StartFocus => {
                vec![CoreEvent::TrayStatus(self.compute_tray_status(now))]
            }
            CoreCommand::PauseToggle => {
                let evs = self.machine.pause_toggle(mono);
                let mut out = map_subevents(evs);
                self.after_machine_events(mono, &out);
                out.push(CoreEvent::TrayStatus(self.compute_tray_status(now)));
                out
            }
            CoreCommand::SkipBreak => {
                let evs = self.machine.skip_break(mono);
                let mut out = map_subevents(evs);
                self.after_machine_events(mono, &out);
                out.push(CoreEvent::TrayStatus(self.compute_tray_status(now)));
                out
            }
            CoreCommand::PostponeBreak => {
                let evs = self.machine.postpone(mono);
                let mut out = map_subevents(evs);
                self.after_machine_events(mono, &out);
                out.push(CoreEvent::TrayStatus(self.compute_tray_status(now)));
                out
            }
            CoreCommand::LogWater(amount_ml) => {
                self.hydration.log_water(amount_ml, mono);
                let progress =
                    HydrationProgress::new(self.hydration.consumed(), self.hydration.goal());
                vec![
                    CoreEvent::HydrationUpdated(progress),
                    CoreEvent::TrayStatus(self.compute_tray_status(now)),
                ]
            }
            CoreCommand::IdleObserved(_d) => {
                // Shell feeds idle via `tick`; this command
                // is reserved for future asynchronous pushes (e.g.
                // system events).
                Vec::new()
            }
            CoreCommand::ConfigUpdated(new_cfg) => {
                self.config = new_cfg.clone();
                self.hydration
                    .update_config(hydration_scheduler_config(&self.config));
                self.posture
                    .update_config(posture_scheduler_config(&self.config));
                let evs = self.machine.update_config(mono, new_cfg);
                let mut out = map_subevents(evs);
                self.after_machine_events(mono, &out);
                if matches!(
                    self.machine.state(),
                    crate::session::state::SessionState::Focus
                ) {
                    self.posture.arm(mono);
                }
                out.push(CoreEvent::TrayStatus(self.compute_tray_status(now)));
                out
            }
            CoreCommand::SetStrictness(s) => {
                self.machine.set_strictness(s);
                self.config.strictness = s;
                vec![CoreEvent::TrayStatus(self.compute_tray_status(now))]
            }
        }
    }

    /// Render the current tray line — used by tests and by the
    /// runtime to bootstrap the tray tooltip.
    #[allow(dead_code)]
    pub fn current_tray_status(&self, now: Timestamp) -> TrayStatus {
        self.compute_tray_status(now)
    }

    pub fn hydration_consumed(&self) -> u32 {
        self.hydration.consumed()
    }

    // -----------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------

    fn after_machine_events(&mut self, now: Millis, events: &[CoreEvent]) {
        for ev in events {
            match ev {
                CoreEvent::DismissBreak => {
                    self.coordinator.mark_break_dismissed(now);
                    self.posture.arm(now);
                }
                CoreEvent::StateChanged(crate::session::state::SessionState::Focus) => {
                    if self.posture.next_due_mono().is_none() {
                        self.posture.arm(now);
                    }
                }
                _ => {}
            }
        }
    }

    /// Compute the tray-status text + icon hint for the current
    /// engine state. Public so the shell can read it from a separate
    /// `EngineSnapshot` IPC command.
    pub fn compute_tray_status(&self, now: Timestamp) -> TrayStatus {
        let mono = monotonic_ms(now);
        match self.machine.state() {
            crate::session::state::SessionState::Focus => {
                let micro = self.machine.timer_target(scheduler_key::MICRO);
                let rest = self.machine.timer_target(scheduler_key::REST);
                let next = match (micro, rest) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    _ => None,
                };
                match next {
                    Some(target) if target > mono => {
                        TrayStatus::countdown(Duration::from_millis(target - mono))
                    }
                    _ => TrayStatus::countdown(Duration::ZERO),
                }
            }
            crate::session::state::SessionState::PreBreak { kind, remaining_ms } => {
                TrayStatus::pre_break(kind, Duration::from_millis(remaining_ms))
            }
            crate::session::state::SessionState::MicroBreak { remaining_ms } => {
                TrayStatus::in_break(
                    crate::session::state::BreakKind::Micro,
                    Duration::from_millis(remaining_ms),
                )
            }
            crate::session::state::SessionState::RestBreak { remaining_ms } => {
                TrayStatus::in_break(
                    crate::session::state::BreakKind::Rest,
                    Duration::from_millis(remaining_ms),
                )
            }
            crate::session::state::SessionState::Postponed { until_ms, .. } => {
                let total_secs = until_ms.saturating_sub(mono) / 1_000;
                let m = total_secs / 60;
                let s = total_secs % 60;
                TrayStatus {
                    title: "Rewind".to_string(),
                    tooltip_line: format!("Postponed — back in {m}:{s:02}"),
                    icon_hint: "postponed".to_string(),
                }
            }
            crate::session::state::SessionState::Paused { reason } => {
                let label = match reason {
                    crate::session::state::PauseReason::Idle => "idle",
                    crate::session::state::PauseReason::Manual => "you",
                };
                TrayStatus::paused(label)
            }
        }
    }
}

// Reach into the SessionMachine for the focus-timer targets via a
// small accessor we add on the impl block. Defined in the same
// module so it stays private to the workspace but is visible from
// `engine`.
impl SessionMachine {
    pub(crate) fn timer_target(&self, key: u8) -> Option<Millis> {
        match key {
            0 => self.timers.next_micro_at,
            1 => self.timers.next_rest_at,
            _ => None,
        }
    }
}

/// Convert a SessionMachine sub-event stream into CoreEvents for the
/// shell to dispatch.
fn map_subevents(sub: Vec<SessionEvent>) -> Vec<CoreEvent> {
    let mut out = Vec::with_capacity(sub.len());
    for s in sub {
        match s {
            SessionEvent::StateChanged(s) => out.push(CoreEvent::StateChanged(s)),
            SessionEvent::ShowBreak {
                kind,
                presentation_strict,
            } => {
                out.push(CoreEvent::ShowBreak {
                    kind,
                    presentation: if presentation_strict {
                        BreakPresentation::Strict
                    } else {
                        BreakPresentation::Gentle
                    },
                    exercise_id: None,
                });
            }
            SessionEvent::DismissBreak => out.push(CoreEvent::DismissBreak),
            SessionEvent::BreakFinished { .. } => {
                // BreakRecord persistence goes through
                // `HistoryRepo::append_break`. The DismissBreak +
                // StateChanged already convey the observable outcome.
            }
            SessionEvent::TrayLine(_) => {
                // Engine composes the canonical TrayStatus; we
                // discard the machine's freeform log line here so
                // it doesn't compete with the structured one.
            }
        }
    }
    out
}

/// Drop consecutive duplicate `StateChanged` events of the same value.
fn dedupe_consecutive_state_changes(events: &mut Vec<CoreEvent>) {
    let mut last: Option<crate::session::state::SessionState> = None;
    events.retain(|e| match e {
        CoreEvent::StateChanged(s) => {
            let keep = match &last {
                Some(prev) => prev != s,
                None => true,
            };
            if keep {
                last = Some(s.clone());
            }
            keep
        }
        _ => true,
    });
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::session::state::{BreakKind, SessionState, Strictness};

    fn at(ms: u64) -> Timestamp {
        Timestamp(ms as i64)
    }

    fn fresh_engine(now_ms: u64) -> Engine {
        Engine::new(at(now_ms), AppConfig::default())
    }

    #[test]
    fn engine_starts_in_focus() {
        let e = fresh_engine(0);
        assert!(matches!(e.state(), SessionState::Focus));
        assert!(e.last_tray_status.is_some());
    }

    #[test]
    fn full_focus_to_micro_to_focus_cycle_emits_expected_events() {
        let mut e = fresh_engine(0);
        // 20 min later: micro timer fires → PreBreak.
        let events = e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::StateChanged(SessionState::PreBreak {
                kind: BreakKind::Micro,
                ..
            })
        )));

        // 11 sec later: → MicroBreak (ShowBreak + StateChanged).
        let events = e.tick(at(20 * 60 * 1_000 + 11_000), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::ShowBreak {
                kind: BreakKind::Micro,
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::MicroBreak { .. }))));

        // 20 sec later: → Focus.
        let events = e.tick(at(20 * 60 * 1_000 + 11_000 + 20_000), Duration::ZERO);
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::DismissBreak)));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Focus))));
    }

    #[test]
    fn tick_emits_tick_heartbeat_and_tray() {
        let mut e = fresh_engine(0);
        let events = e.tick(at(1_000), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(ev, CoreEvent::Tick { .. })));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::TrayStatus(_))));
    }

    #[test]
    fn skip_break_command_routes_to_machine() {
        let mut e = fresh_engine(0);
        e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        e.tick(at(20 * 60 * 1_000 + 11_000), Duration::ZERO);
        assert!(matches!(e.state(), SessionState::MicroBreak { .. }));
        let events = e.handle(CoreCommand::SkipBreak, at(20 * 60 * 1_000 + 12_000));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::DismissBreak)));
        assert!(matches!(e.state(), SessionState::Focus));
    }

    #[test]
    fn skip_break_under_strict_is_silently_refused() {
        let mut cfg = AppConfig::default();
        cfg.strictness = Strictness::Strict;
        let mut e = Engine::new(at(0), cfg);
        e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        e.tick(at(20 * 60 * 1_000 + 11_000), Duration::ZERO);
        assert!(matches!(e.state(), SessionState::MicroBreak { .. }));
        let events = e.handle(CoreCommand::SkipBreak, at(20 * 60 * 1_000 + 12_000));
        assert!(!events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::DismissBreak)));
        assert!(matches!(e.state(), SessionState::MicroBreak { .. }));
    }

    #[test]
    fn log_water_emits_hydration_updated() {
        let mut e = fresh_engine(0);
        let events = e.handle(CoreCommand::LogWater(250), at(1_000));
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::HydrationUpdated(HydrationProgress {
                consumed_ml: 250,
                ..
            })
        )));
    }

    #[test]
    fn postpone_then_postpone_expires_lands_in_pre_break() {
        let mut e = fresh_engine(0);
        e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        let events = e.handle(CoreCommand::PostponeBreak, at(20 * 60 * 1_000 + 5_000));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Postponed { .. }))));
        let until = match e.state() {
            SessionState::Postponed { until_ms, .. } => until_ms,
            other => panic!("expected Postponed, got {other:?}"),
        };
        let events = e.tick(at(until + 1), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::StateChanged(SessionState::PreBreak {
                kind: BreakKind::Micro,
                ..
            })
        )));
    }

    #[test]
    fn idle_pause_then_resume() {
        let mut cfg = AppConfig::default();
        cfg.idle.pause_sec = 60;
        cfg.idle.resume_sec = 5;
        cfg.idle.reset_sec = 600;
        let mut e = Engine::new(at(0), cfg);

        let events = e.tick(at(0), Duration::from_secs(61));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Paused { .. }))));

        // Still idle — stay paused.
        let events = e.tick(at(50_000), Duration::from_secs(40));
        assert!(!events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Focus))));

        // Returns (idle drops below resume) → Focus.
        let events = e.tick(at(60_000), Duration::from_secs(2));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Focus))));
    }

    #[test]
    fn idle_pause_long_then_reset() {
        let mut cfg = AppConfig::default();
        cfg.idle.pause_sec = 30;
        cfg.idle.resume_sec = 5;
        cfg.idle.reset_sec = 200;
        let mut e = Engine::new(at(0), cfg);

        let events = e.tick(at(0), Duration::from_secs(31));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Paused { .. }))));

        // Long absence (>= reset) — Reset.
        let events = e.tick(at(50_000), Duration::from_secs(201));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Focus))));
    }

    #[test]
    fn tray_status_focus_has_countdown() {
        let e = fresh_engine(0);
        let s = e.current_tray_status(at(0));
        assert!(s.tooltip_line.starts_with("Next break in 20:"));
        assert_eq!(s.icon_hint, "focus");
    }

    #[test]
    fn tray_status_paused_text() {
        let mut cfg = AppConfig::default();
        cfg.idle.pause_sec = 5;
        let mut e = Engine::new(at(0), cfg);
        let _ = e.tick(at(0), Duration::from_secs(6));
        let s = e.current_tray_status(at(0));
        assert!(s.tooltip_line.contains("Paused"));
        assert_eq!(s.icon_hint, "paused");
    }

    #[test]
    fn config_updated_command_replaces_config() {
        let mut e = fresh_engine(0);
        let mut new_cfg = AppConfig::default();
        new_cfg.strictness = Strictness::Strict;
        let _ = e.handle(CoreCommand::ConfigUpdated(new_cfg.clone()), at(1_000));
        assert_eq!(e.config().strictness, Strictness::Strict);
    }

    #[test]
    fn set_strictness_command() {
        let mut e = fresh_engine(0);
        let _ = e.handle(CoreCommand::SetStrictness(Strictness::Normal), at(1_000));
        assert_eq!(e.config().strictness, Strictness::Normal);
    }

    #[test]
    fn natural_idle_satisfies_break_via_engine_tick() {
        let mut e = fresh_engine(0);
        e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        e.tick(at(20 * 60 * 1_000 + 11_000), Duration::ZERO);
        assert!(matches!(e.state(), SessionState::MicroBreak { .. }));
        let events = e.tick(at(20 * 60 * 1_000 + 12_000), Duration::from_secs(60));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::DismissBreak)));
    }

    #[test]
    fn pause_toggle_command_emits_pause_then_resume() {
        let mut e = fresh_engine(0);
        let events = e.handle(CoreCommand::PauseToggle, at(1_000));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Paused { .. }))));
        let events = e.handle(CoreCommand::PauseToggle, at(2_000));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::StateChanged(SessionState::Focus))));
    }

    #[test]
    fn tray_status_format_for_break() {
        let mut cfg = AppConfig::default();
        cfg.breaks.pre_break_warn = false;
        let mut e = Engine::new(at(0), cfg);
        e.tick(at(20 * 60 * 1_000), Duration::ZERO);
        let s = e.current_tray_status(at(20 * 60 * 1_000 + 5_000));
        assert!(s.tooltip_line.contains("Micro break"));
    }

    #[test]
    fn start_focus_is_a_tray_refresh() {
        let mut e = fresh_engine(0);
        let events = e.handle(CoreCommand::StartFocus, at(1_000));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, CoreEvent::TrayStatus(_))));
    }

    fn local_ms_at(day: i64, hour: u8, minute: u8) -> u64 {
        let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        let local_secs = day * 24 * 60 * 60 + i64::from(hour) * 3_600 + i64::from(minute) * 60;
        (local_secs - i64::from(offset.whole_seconds())) as u64 * 1_000
    }

    fn no_break_noise(mut cfg: AppConfig) -> AppConfig {
        cfg.breaks.micro_interval_min = 240;
        cfg.breaks.rest_interval_min = 240;
        cfg.breaks.pre_break_warn = false;
        cfg
    }

    fn has_reminder(events: &[CoreEvent], kind: ReminderKind) -> bool {
        events.iter().any(|ev| {
            matches!(
                ev,
                CoreEvent::FireReminder { kind: k, .. } if *k == kind
            )
        })
    }

    #[test]
    fn reminder_fires_when_hydration_due() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.posture = false;
        cfg.hydration.goal_ml = 250;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 31)), Duration::ZERO);
        assert!(has_reminder(&events, ReminderKind::Hydration));
    }

    #[test]
    fn hydration_reminder_suppressed_when_ahead_of_pace() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.posture = false;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let _ = e.handle(CoreCommand::LogWater(500), at(local_ms_at(day, 9, 5)));
        let events = e.tick(at(local_ms_at(day, 12, 0)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Hydration));
    }

    #[test]
    fn hydration_reminder_coalesced_onto_rest_break() {
        let day = 10_000;
        let mut cfg = AppConfig::default();
        cfg.breaks.micro_interval_min = 240;
        cfg.breaks.rest_interval_min = 35;
        cfg.breaks.pre_break_warn = false;
        cfg.reminders.posture = false;
        cfg.hydration.goal_ml = 250;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 31)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Hydration));
        let events = e.tick(at(local_ms_at(day, 9, 35)), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::ShowBreak {
                kind: BreakKind::Rest,
                ..
            }
        )));
    }

    #[test]
    fn posture_reminder_fires_at_default_interval() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.hydration = false;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 40)), Duration::ZERO);
        assert!(has_reminder(&events, ReminderKind::Posture));
    }

    #[test]
    fn posture_suppressed_during_break_or_pause() {
        let day = 10_000;
        let mut cfg = AppConfig::default();
        cfg.breaks.micro_interval_min = 40;
        cfg.breaks.rest_interval_min = 240;
        cfg.breaks.pre_break_warn = false;
        cfg.reminders.hydration = false;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 40)), Duration::ZERO);
        assert!(events.iter().any(|ev| matches!(
            ev,
            CoreEvent::ShowBreak {
                kind: BreakKind::Micro,
                ..
            }
        )));
        assert!(!has_reminder(&events, ReminderKind::Posture));

        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.hydration = false;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let _ = e.handle(CoreCommand::PauseToggle, at(local_ms_at(day, 9, 1)));
        let events = e.tick(at(local_ms_at(day, 9, 40)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Posture));
    }

    #[test]
    fn coordinator_quiet_gap_suppresses_back_to_back() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.hydration.goal_ml = 250;
        cfg.posture.interval_min = 31;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 31)), Duration::ZERO);
        assert!(has_reminder(&events, ReminderKind::Posture));
        let events = e.tick(at(local_ms_at(day, 9, 32)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Hydration));
    }

    #[test]
    fn quiet_hours_gate_suppresses_due_reminders() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.posture = false;
        cfg.hydration.goal_ml = 250;
        cfg.quiet_hours.enabled = true;
        cfg.quiet_hours.start = "09:00".to_string();
        cfg.quiet_hours.end = "10:00".to_string();
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let events = e.tick(at(local_ms_at(day, 9, 31)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Hydration));
    }

    #[test]
    fn hydration_quick_log_defers_engine_reminder() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.posture = false;
        cfg.hydration.goal_ml = 250;
        let mut e = Engine::new(at(local_ms_at(day, 9, 0)), cfg);
        let _ = e.handle(CoreCommand::LogWater(5), at(local_ms_at(day, 9, 20)));
        let events = e.tick(at(local_ms_at(day, 9, 31)), Duration::ZERO);
        assert!(!has_reminder(&events, ReminderKind::Hydration));
        let events = e.tick(at(local_ms_at(day, 9, 51)), Duration::ZERO);
        assert!(has_reminder(&events, ReminderKind::Hydration));
    }

    #[test]
    fn hydration_resets_on_local_day_rollover() {
        let day = 10_000;
        let mut cfg = no_break_noise(AppConfig::default());
        cfg.reminders.hydration = false;
        cfg.reminders.posture = false;
        let mut e = Engine::new(at(local_ms_at(day, 23, 50)), cfg);
        let _ = e.handle(CoreCommand::LogWater(250), at(local_ms_at(day, 23, 55)));
        assert_eq!(e.hydration_consumed(), 250);
        let _ = e.tick(at(local_ms_at(day + 1, 0, 10)), Duration::ZERO);
        assert_eq!(e.hydration_consumed(), 0);
    }
}
