//! Posture scheduler.
//!
//! Stand/stretch nudge every **40 min**; prefer coalescing onto a
//! nearby rest break. See implementation plan §7h. Coalescing
//! requires the rest-break cadence to be visible, so the real
//! scheduler lands alongside the reminder coordinator in M5.

use std::time::Duration;

use crate::clock::Millis;
use crate::config::PostureConfig;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

#[derive(Debug, Default, Clone)]
pub struct PostureScheduler {
    interval: Duration,
    next_due_mono: Option<Millis>,
}

impl PostureScheduler {
    pub fn new() -> Self {
        Self {
            interval: Duration::from_secs(40 * 60),
            next_due_mono: None,
        }
    }

    pub fn from_config(config: &PostureConfig) -> Self {
        Self {
            interval: config.interval(),
            next_due_mono: None,
        }
    }

    /// Arm the first nudge for `now + interval`. Called by the engine
    /// when entering `Focus` from a paused/initial state.
    pub fn arm(&mut self, now: Millis) {
        self.next_due_mono = Some(now.saturating_add(self.interval.as_millis() as Millis));
    }

    /// Returns `Some(reminder)` if `now >= next_due_mono`. The scheduler
    /// re-arms for the next interval.
    pub fn maybe_remind(&mut self, now: Millis) -> Option<Reminder> {
        let due = self.next_due_mono?;
        if now < due {
            return None;
        }
        self.next_due_mono = Some(now.saturating_add(self.interval.as_millis() as Millis));
        Some(Reminder {
            kind: ReminderKind::Posture,
            priority: Priority::Medium,
            earliest: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_does_not_fire() {
        let mut s = PostureScheduler::new();
        assert_eq!(s.maybe_remind(0), None);
        assert_eq!(s.maybe_remind(1_000_000_000), None);
    }

    #[test]
    fn arm_then_tick_below_due() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        assert_eq!(s.maybe_remind(1_000), None);
    }

    #[test]
    fn arm_then_tick_at_due_fires_and_rearms() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        let r = s.maybe_remind(40 * 60 * 1_000).unwrap();
        assert_eq!(r.kind, ReminderKind::Posture);
        // Next remind fires 40 min later.
        assert_eq!(s.maybe_remind(40 * 60 * 1_000 + 1), None);
        assert!(s.maybe_remind(2 * 40 * 60 * 1_000).is_some());
    }

    #[test]
    fn interval_honours_config() {
        let mut s = PostureScheduler::from_config(&PostureConfig {
            interval_min: 1,
        });
        s.arm(0);
        assert!(s.maybe_remind(60_000).is_some());
    }
}
