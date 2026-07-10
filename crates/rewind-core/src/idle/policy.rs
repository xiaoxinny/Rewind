//! Idle pause/reset policy (DP-2).
//!
//! Pure functions over `(idle Duration, current SessionState, IdleConfig)`. The
//! `Engine` keeps the small piece of sticky state ("was the most recent
//! pause triggered by idle, and did it ever cross the reset threshold?")
//! — the policy itself is stateless, which makes it trivially testable
//! with a fake clock.
//!
//! Recommended defaults (§7f): pause 90 s → `Paused{Idle}`, reset 300 s
//! → on return reset the cycle, resume on `idle < 10 s`.

use std::time::Duration;

use crate::config::IdleConfig;
use crate::session::state::{BreakKind, SessionState};

/// The action the engine should take on this tick in response to an
/// idle observation. The engine knows whether to act on it (e.g. only
/// `Resume` after a previous `Pause`, only `Reset` after `Pause ∧ idle
/// ever crossed reset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleAction {
    /// Do nothing this tick.
    None,
    /// Transition to `Paused{Idle}`.
    Pause,
    /// Transition out of `Paused{Idle}` and back to where we were.
    Resume,
    /// Transition out of `Paused{Idle}` and **reset** the cycle to
    /// `Focus`. The absent interval counts as a natural break.
    Reset,
}

/// Compute the policy action for the current `idle` reading.
///
/// `state` is the *current* `SessionState` the engine is in (used to
/// short-circuit when paused). `config.enabled` gates the whole policy
/// — the shell flips it off on platforms where the `IdleSource` is
/// `Unreliable` or `Unavailable` (M2).
pub fn evaluate(idle: Duration, state: &SessionState, config: &IdleConfig) -> IdleAction {
    if !config.enabled {
        return IdleAction::None;
    }

    let pause = config.pause();
    let reset = config.reset();
    let resume = config.resume();

    match state {
        // If the user is already paused, decide between resuming and
        // resetting based on whether they ever crossed the reset
        // threshold while away.
        SessionState::Paused {
            reason: crate::session::state::PauseReason::Idle,
        } => {
            if idle >= reset {
                // They've been away long enough that the absence
                // counts as a natural break — reset the cycle.
                IdleAction::Reset
            } else if idle < resume {
                // They've come back (hysteresis band) and weren't
                // away long enough to warrant a reset — resume.
                IdleAction::Resume
            } else {
                // Still in the dead-band (between `resume` and
                // `reset`). Stay paused.
                IdleAction::None
            }
        }

        // Paused{Manual} only returns to active via PauseToggle — the
        // idle policy never touches it.
        SessionState::Paused {
            reason: crate::session::state::PauseReason::Manual,
        } => IdleAction::None,

        // During a micro/rest break — if the user idles for the full
        // break, the absence **satisfies** it (logged as `natural`).
        // The engine doesn't need an action here; the SessionMachine
        // notices in its own tick handler. We return None.
        SessionState::MicroBreak { .. } | SessionState::RestBreak { .. } => {
            // The engine's own `natural-satisfies-break` rule covers
            // this — no action.
            if idle >= Duration::from_secs(u64::MAX / 2) {
                IdleAction::None
            } else {
                IdleAction::None
            }
        }

        // In any other state, "user is sufficiently idle" → Pause.
        _ => {
            if idle >= pause {
                IdleAction::Pause
            } else {
                IdleAction::None
            }
        }
    }
}

/// Was the current break "satisfied naturally" by a full-break idle
/// gap? Returns `Some(kind)` if `idle >= break_duration`, `None`
/// otherwise. Called when the SessionMachine sees the break timer
/// expire **and** the idle reading is large enough to mean "user
/// wasn't there". Distinct from `evaluate` because the answer depends
/// on the *currently active* break's length, not the global reset
/// threshold.
pub fn natural_break_satisfied(
    idle: Duration,
    state: &SessionState,
    micro_sec: u32,
    rest_sec: u32,
) -> Option<BreakKind> {
    match state {
        SessionState::MicroBreak { .. } => {
            if idle >= Duration::from_secs(u64::from(micro_sec)) {
                Some(BreakKind::Micro)
            } else {
                None
            }
        }
        SessionState::RestBreak { .. } => {
            if idle >= Duration::from_secs(u64::from(rest_sec)) {
                Some(BreakKind::Rest)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::PauseReason;

    fn cfg() -> IdleConfig {
        IdleConfig::default() // pause=90s, reset=300s, resume=10s
    }

    #[test]
    fn below_pause_threshold_no_action() {
        let state = SessionState::Focus;
        assert_eq!(
            evaluate(Duration::from_secs(0), &state, &cfg()),
            IdleAction::None
        );
        assert_eq!(
            evaluate(Duration::from_secs(89), &state, &cfg()),
            IdleAction::None
        );
        // Exactly at pause threshold counts as "exceeded" (>=).
        assert_eq!(
            evaluate(Duration::from_secs(90), &state, &cfg()),
            IdleAction::Pause
        );
    }

    #[test]
    fn above_pause_below_reset_pauses() {
        let state = SessionState::Focus;
        assert_eq!(
            evaluate(Duration::from_secs(91), &state, &cfg()),
            IdleAction::Pause
        );
        assert_eq!(
            evaluate(Duration::from_secs(120), &state, &cfg()),
            IdleAction::Pause
        );
        // Still below reset — Pause holds.
        assert_eq!(
            evaluate(Duration::from_secs(299), &state, &cfg()),
            IdleAction::Pause
        );
    }

    #[test]
    fn at_and_above_reset_pauses_when_active() {
        // While the user is still in `Focus`, the action is always
        // `Pause`. The `Reset` verdict is only delivered while we're
        // already in Paused{Idle}.
        let state = SessionState::Focus;
        assert_eq!(
            evaluate(Duration::from_secs(300), &state, &cfg()),
            IdleAction::Pause
        );
        assert_eq!(
            evaluate(Duration::from_secs(900), &state, &cfg()),
            IdleAction::Pause
        );
    }

    #[test]
    fn paused_idle_below_resume_resumes() {
        let state = SessionState::Paused {
            reason: PauseReason::Idle,
        };
        assert_eq!(
            evaluate(Duration::from_secs(0), &state, &cfg()),
            IdleAction::Resume
        );
        assert_eq!(
            evaluate(Duration::from_secs(9), &state, &cfg()),
            IdleAction::Resume
        );
        // 10s is the resume threshold — returning IdleAction::None
        // here means "stay paused".
        assert_eq!(
            evaluate(Duration::from_secs(10), &state, &cfg()),
            IdleAction::None
        );
    }

    #[test]
    fn paused_idle_in_hysteresis_dead_band() {
        // Between `resume` (10s) and `reset` (300s) — stay paused.
        // Deliberately do nothing on these ticks.
        let state = SessionState::Paused {
            reason: PauseReason::Idle,
        };
        assert_eq!(
            evaluate(Duration::from_secs(45), &state, &cfg()),
            IdleAction::None
        );
        assert_eq!(
            evaluate(Duration::from_secs(120), &state, &cfg()),
            IdleAction::None
        );
        assert_eq!(
            evaluate(Duration::from_secs(299), &state, &cfg()),
            IdleAction::None
        );
    }

    #[test]
    fn paused_idle_at_reset_resets() {
        let state = SessionState::Paused {
            reason: PauseReason::Idle,
        };
        assert_eq!(
            evaluate(Duration::from_secs(300), &state, &cfg()),
            IdleAction::Reset
        );
        assert_eq!(
            evaluate(Duration::from_secs(1_000), &state, &cfg()),
            IdleAction::Reset
        );
    }

    #[test]
    fn paused_manual_never_auto_resumes() {
        let state = SessionState::Paused {
            reason: PauseReason::Manual,
        };
        assert_eq!(
            evaluate(Duration::from_secs(0), &state, &cfg()),
            IdleAction::None
        );
        assert_eq!(
            evaluate(Duration::from_secs(900), &state, &cfg()),
            IdleAction::None
        );
    }

    #[test]
    fn disabled_config_never_fires() {
        let state = SessionState::Focus;
        let mut c = cfg();
        c.enabled = false;
        assert_eq!(
            evaluate(Duration::from_secs(900), &state, &c),
            IdleAction::None
        );
        let state = SessionState::Paused {
            reason: PauseReason::Idle,
        };
        assert_eq!(
            evaluate(Duration::from_secs(900), &state, &c),
            IdleAction::None
        );
        assert_eq!(
            evaluate(Duration::from_secs(0), &state, &c),
            IdleAction::None
        );
    }

    #[test]
    fn in_break_does_not_pause() {
        // During a break we already have an active countdown; we
        // don't pause on top of it. The engine handles natural-break
        // satisfaction separately.
        let state = SessionState::MicroBreak {
            remaining_ms: 5_000,
        };
        assert_eq!(
            evaluate(Duration::from_secs(500), &state, &cfg()),
            IdleAction::None
        );
        let state = SessionState::RestBreak {
            remaining_ms: 60_000,
        };
        assert_eq!(
            evaluate(Duration::from_secs(500), &state, &cfg()),
            IdleAction::None
        );
    }

    #[test]
    fn natural_break_satisfied_micro() {
        let state = SessionState::MicroBreak {
            remaining_ms: 1_000,
        };
        // Idle less than micro duration — not satisfied.
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(15), &state, 20, 300),
            None
        );
        // Exactly the duration — satisfied.
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(20), &state, 20, 300),
            Some(BreakKind::Micro)
        );
        // Above.
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(120), &state, 20, 300),
            Some(BreakKind::Micro)
        );
    }

    #[test]
    fn natural_break_satisfied_rest() {
        let state = SessionState::RestBreak {
            remaining_ms: 1_000,
        };
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(200), &state, 20, 300),
            None
        );
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(300), &state, 20, 300),
            Some(BreakKind::Rest)
        );
        assert_eq!(
            natural_break_satisfied(Duration::from_secs(3_600), &state, 20, 300),
            Some(BreakKind::Rest)
        );
    }

    #[test]
    fn natural_break_not_checked_outside_break() {
        // `natural_break_satisfied` returns None for non-break states.
        for state in [
            SessionState::Focus,
            SessionState::PreBreak {
                kind: BreakKind::Micro,
                remaining_ms: 1,
            },
            SessionState::Postponed {
                kind: BreakKind::Rest,
                until_ms: 0,
            },
            SessionState::Paused {
                reason: PauseReason::Idle,
            },
        ] {
            assert_eq!(
                natural_break_satisfied(Duration::from_secs(99_999), &state, 20, 300),
                None
            );
        }
    }
}
