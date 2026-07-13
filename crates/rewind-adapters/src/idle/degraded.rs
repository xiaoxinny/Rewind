//! Timer-only idle sources.
//!
//! Two backends live here:
//!
//! * [`DegradedIdleSource`] — final fallback when no real idle data
//!   is available (GNOME Wayland, headless CI, sandboxed Flatpak
//!   with no D-Bus access, anything that fails to construct a
//!   `UserIdleSource`). Always returns `Duration::ZERO` and reports
//!   [`IdleReliability::Unavailable`]. The engine uses this signal
//!   to switch to **timer-only mode** (no idle pause/reset) and the
//!   UI shows the honest "screen-time tracking limited on this
//!   session" note.
//!
//! * [`TimerOnlyIdleSource`] — test-only double. Same `Duration::ZERO`
//!   answer but reports [`IdleReliability::Unreliable`] so tests
//!   that want a "running but fake" idle (i.e. a source the engine
//!   *can* call `idle_time()` on without complaint, but that
//!   returns nonsense) get exactly that.

use std::time::Duration;

use rewind_core::ports::{IdleError, IdleReliability, IdleSource};

/// Always-unavailable fallback. See module docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct DegradedIdleSource;

impl DegradedIdleSource {
    pub fn new() -> Self {
        Self
    }
}

impl IdleSource for DegradedIdleSource {
    fn idle_time(&self) -> Result<Duration, IdleError> {
        // Always zero. The whole point of this source is to
        // *not* know — returning zero lets the engine proceed
        // on its own timer.
        Ok(Duration::ZERO)
    }

    fn reliability(&self) -> IdleReliability {
        // Telling the engine "I have no data, do not trust me
        // for pause/reset" — the engine's reliability gate will
        // switch to timer-only mode.
        IdleReliability::Unavailable
    }
}

/// Test-only double: returns `Duration::ZERO` but reports
/// `Unreliable` (i.e. a source the engine *can* poll but whose
/// numbers it should not trust for pause/reset decisions).
///
/// Used by engine unit tests that want a source that is "alive
/// enough not to error but fake enough that the engine doesn't
/// act on the values".
#[derive(Debug, Clone, Copy, Default)]
pub struct TimerOnlyIdleSource;

impl TimerOnlyIdleSource {
    pub fn new() -> Self {
        Self
    }
}

impl IdleSource for TimerOnlyIdleSource {
    fn idle_time(&self) -> Result<Duration, IdleError> {
        Ok(Duration::ZERO)
    }

    fn reliability(&self) -> IdleReliability {
        // The engine **keeps timer-only mode on** but still
        // polls the source each tick — distinct from
        // `DegradedIdleSource`, which makes the engine skip the
        // idle-policy branch entirely in some implementations.
        IdleReliability::Unreliable
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rewind_core::ports::IdleSource;

    #[test]
    fn degraded_returns_zero() {
        let s = DegradedIdleSource::new();
        assert_eq!(s.idle_time().unwrap(), Duration::ZERO);
    }

    #[test]
    fn degraded_reliability_is_unavailable() {
        let s = DegradedIdleSource::new();
        assert_eq!(s.reliability(), IdleReliability::Unavailable);
    }

    #[test]
    fn degraded_is_default_and_copy() {
        // Both default and copy hold — cheap ergonomic affordances.
        let _a: DegradedIdleSource = Default::default();
        let b = DegradedIdleSource::new();
        let _c = b; // Copy
    }

    #[test]
    fn timer_only_returns_zero() {
        let s = TimerOnlyIdleSource::new();
        assert_eq!(s.idle_time().unwrap(), Duration::ZERO);
    }

    #[test]
    fn timer_only_reliability_is_unreliable() {
        let s = TimerOnlyIdleSource::new();
        assert_eq!(s.reliability(), IdleReliability::Unreliable);
    }

    #[test]
    fn timer_only_distinct_from_degraded() {
        // The two are intentionally not interchangeable: the
        // engine's reliability gate treats `Unreliable`
        // differently from `Unavailable`. This test guards
        // against an accidental copy-paste that would collapse
        // the two.
        let d = DegradedIdleSource::new().reliability();
        let t = TimerOnlyIdleSource::new().reliability();
        assert_ne!(d, t);
    }
}
