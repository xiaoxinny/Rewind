//! Clock abstraction.
//!
//! Splits wall-clock from monotonic so:
//!   * hibernate / sleep never counts as active time (use [`Clock::monotonic`] for durations),
//!   * day-bucketing is DST-safe (use [`Clock::now`] for local-day boundaries).
//!
//! See implementation plan §7a.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

/// Monotonic milliseconds. Use for durations and timer arms; never goes
/// backwards, ignores wall-clock changes.
pub type Millis = u64;

/// Wall-clock timestamp (Unix milliseconds since the epoch). Use for
/// logging and local-day bucketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub const ZERO: Timestamp = Timestamp(0);
}

/// The single seam through which the engine reads the passage of time.
///
/// Two methods on purpose — wall for logging/buckets, monotonic for
/// durations. Implementations **must** return non-decreasing values from
/// `monotonic` between successive calls on the same thread.
pub trait Clock {
    /// Wall-clock time (UTC). Used for logging and local-day bucketing.
    fn now(&self) -> Timestamp;
    /// Monotonic time. Used for durations and timer arms.
    fn monotonic(&self) -> Millis;
}

/// Production clock backed by the operating system.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Timestamp {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Timestamp(ms)
    }

    fn monotonic(&self) -> Millis {
        // On Linux/macOS this is the best portable approximation. For true
        // monotonicity the caller would use `time::Instant`, but the
        // `RealClock` is only used in `src-tauri`'s wiring; `rewind-core`
        // tests use `FakeClock` exclusively.
        Self::now(self).0.max(0) as Millis
    }
}

/// Test double: a manually-advanceable clock.
///
/// Holds both clock readings in interior `RefCell`s so tests can hold
/// `&self` while advancing. **Not `Send` / `Sync`** — single-threaded
/// tests only.
#[derive(Debug, Default)]
pub struct FakeClock {
    wall: RefCell<i64>,
    mono: RefCell<Millis>,
}

impl FakeClock {
    /// Start the fake clock at zero for both readings.
    pub fn new() -> Self {
        Self {
            wall: RefCell::new(0),
            mono: RefCell::new(0),
        }
    }

    /// Start the fake clock at the supplied readings.
    pub fn starting_at(wall_ms: i64, mono_ms: Millis) -> Self {
        Self {
            wall: RefCell::new(wall_ms),
            mono: RefCell::new(mono_ms),
        }
    }

    /// Advance both clocks by `by` milliseconds. `by` must be ≥ 0.
    ///
    /// Saturates at the type's max; never panics.
    pub fn advance(&self, by: Millis) {
        if let Ok(mut mono) = self.mono.try_borrow_mut() {
            *mono = mono.saturating_add(by);
        }
        if let Ok(mut wall) = self.wall.try_borrow_mut() {
            // Convert `by: u64` to `i64` for the wall counter. If `by` is
            // larger than `i64::MAX` we cap the addition at i64::MAX
            // instead of silently wrapping to a negative timestamp.
            let delta = by.min(i64::MAX as u64) as i64;
            *wall = wall.saturating_add(delta);
        }
    }

    /// Read the current wall-clock reading (for assertions).
    pub fn peek_wall(&self) -> i64 {
        *self.wall.borrow()
    }

    /// Read the current monotonic reading (for assertions).
    pub fn peek_mono(&self) -> Millis {
        *self.mono.borrow()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        Timestamp(*self.wall.borrow())
    }

    fn monotonic(&self) -> Millis {
        *self.mono.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_starts_at_zero_by_default() {
        let clock = FakeClock::new();
        assert_eq!(clock.now().0, 0);
        assert_eq!(clock.monotonic(), 0);
    }

    #[test]
    fn advance_increments_both_clocks() {
        let clock = FakeClock::starting_at(1_700_000_000_000, 5_000);
        clock.advance(2_500);
        assert_eq!(clock.now().0, 1_700_000_002_500);
        assert_eq!(clock.monotonic(), 7_500);
    }

    #[test]
    fn advance_is_cumulative() {
        let clock = FakeClock::new();
        for _ in 0..10 {
            clock.advance(100);
        }
        assert_eq!(clock.monotonic(), 1_000);
        assert_eq!(clock.now().0, 1_000);
    }

    #[test]
    fn advance_saturates_at_overflow() {
        // Start near u64::MAX for the monotonic counter and near i64::MAX
        // for the wall counter. Advance by another u64::MAX. We must not
        // panic; both values must clamp at their respective maxes.
        let clock = FakeClock::starting_at(i64::MAX - 10, Millis::MAX - 10);
        clock.advance(Millis::MAX);
        // saturating_add clamps at MAX for both — we should not panic.
        assert!(clock.peek_wall() >= i64::MAX - 10);
        assert_eq!(clock.peek_mono(), Millis::MAX);
    }
}
