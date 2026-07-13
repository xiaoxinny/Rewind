//! Posture scheduler.
//!
//! Stand/stretch nudge every **40 min** (default); prefer coalescing
//! onto a nearby rest break. Coalescing is implemented at
//! the engine/coordinator level — this scheduler just reports when a
//! posture reminder is due.

use std::time::Duration;

use crate::clock::Millis;
use crate::config::PostureConfig;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

/// Configuration knobs. Default is the 40-min interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostureSchedulerConfig {
    pub interval_ms: Millis,
    pub min_gap_ms: Millis,
}

impl Default for PostureSchedulerConfig {
    fn default() -> Self {
        Self {
            interval_ms: 40 * 60 * 1_000,
            min_gap_ms: 30 * 60 * 1_000,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PostureScheduler {
    pub config: PostureSchedulerConfig,
    next_due_mono: Option<Millis>,
    last_reminder_mono: Option<Millis>,
}

impl PostureScheduler {
    pub fn new() -> Self {
        Self::with_config(PostureSchedulerConfig::default())
    }

    pub fn with_config(config: PostureSchedulerConfig) -> Self {
        Self {
            config,
            next_due_mono: None,
            last_reminder_mono: None,
        }
    }

    pub fn from_config(config: &PostureConfig) -> Self {
        Self::from_app_config(config, 30 * 60 * 1_000)
    }

    pub fn from_app_config(config: &PostureConfig, min_gap_ms: u32) -> Self {
        Self::with_config(PostureSchedulerConfig {
            interval_ms: config.interval().as_millis() as Millis,
            min_gap_ms: Millis::from(min_gap_ms),
        })
    }

    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.config.interval_ms)
    }

    /// Re-arm with a new config (called by the engine on
    /// `ConfigUpdated`). Preserves the current `next_due_mono` so
    /// the user's existing posture timer isn't reset.
    pub fn update_config(&mut self, config: PostureSchedulerConfig) {
        self.config = config;
    }

    /// Arm the first nudge for `now + interval`. Called by the engine
    /// when entering `Focus` from a paused/initial state, and after
    /// a break ends.
    pub fn arm(&mut self, now: Millis) {
        self.next_due_mono = Some(now.saturating_add(self.config.interval_ms));
    }

    /// Record that a posture reminder was actually surfaced and start
    /// a new posture interval from that fire time.
    pub fn mark_reminded(&mut self, now: Millis) {
        self.last_reminder_mono = Some(now);
        self.arm(now);
    }

    /// Postpone the next posture nudge by an explicit duration.
    pub fn snooze(&mut self, now: Millis, by: Duration) {
        self.next_due_mono = Some(now.saturating_add(by.as_millis() as Millis));
    }

    /// Read-only access to the next-due timestamp. Used by the
    /// engine to decide when to coalesce a posture nudge onto an
    /// upcoming rest break.
    pub fn next_due_mono(&self) -> Option<Millis> {
        self.next_due_mono
    }

    pub fn last_reminder_mono(&self) -> Option<Millis> {
        self.last_reminder_mono
    }

    /// Returns `Some(reminder)` if `now >= next_due_mono` and the
    /// `min_gap_ms` is satisfied. It does not re-arm by itself; the
    /// engine calls `mark_reminded` only after the coordinator actually
    /// surfaces the reminder.
    pub fn maybe_remind(&self, now: Millis) -> Option<Reminder> {
        if let Some(prev) = self.last_reminder_mono {
            if now.saturating_sub(prev) < self.config.min_gap_ms {
                return None;
            }
        }
        let due = self.next_due_mono?;
        if now < due {
            return None;
        }
        Some(Reminder {
            kind: ReminderKind::Posture,
            priority: Priority::Medium,
            earliest: due,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_does_not_fire() {
        let s = PostureScheduler::new();
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
    fn arm_tick_mark_cycle() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        let due = 40 * 60 * 1_000;
        let r = s.maybe_remind(due).unwrap();
        assert_eq!(r.kind, ReminderKind::Posture);
        assert_eq!(r.earliest, due);
        assert_eq!(s.maybe_remind(due), Some(r));
        s.mark_reminded(due);
        assert_eq!(s.maybe_remind(due + 1), None);
        assert!(s.maybe_remind(2 * due).is_some());
    }

    #[test]
    fn interval_honours_config() {
        let mut s = PostureScheduler::with_config(PostureSchedulerConfig {
            interval_ms: 60_000,
            min_gap_ms: 0,
        });
        s.arm(0);
        assert!(s.maybe_remind(60_000).is_some());
    }

    #[test]
    fn next_due_mono_reports_armed_time() {
        let mut s = PostureScheduler::new();
        assert_eq!(s.next_due_mono(), None);
        s.arm(100);
        assert_eq!(s.next_due_mono(), Some(100 + 40 * 60 * 1_000));
    }

    #[test]
    fn snooze_defers_next_nudge() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        s.snooze(10_000, Duration::from_secs(5 * 60));
        assert_eq!(s.maybe_remind(10_000 + 4 * 60_000), None);
        assert!(s.maybe_remind(10_000 + 5 * 60_000).is_some());
    }

    #[test]
    fn min_gap_suppresses_short_interval_spam() {
        let mut s = PostureScheduler::with_config(PostureSchedulerConfig {
            interval_ms: 60_000,
            min_gap_ms: 30 * 60 * 1_000,
        });
        s.arm(0);
        assert!(s.maybe_remind(60_000).is_some());
        s.mark_reminded(60_000);
        assert_eq!(s.maybe_remind(2 * 60_000), None);
        assert!(s.maybe_remind(31 * 60_000).is_some());
    }

    #[test]
    fn restart_after_break_ends_rearms_from_now() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        s.arm(5 * 60_000);
        assert_eq!(s.next_due_mono(), Some(45 * 60_000));
        assert_eq!(s.maybe_remind(44 * 60_000), None);
        assert!(s.maybe_remind(45 * 60_000).is_some());
    }

    #[test]
    fn maybe_remind_is_pure_for_same_state() {
        let mut s = PostureScheduler::new();
        s.arm(0);
        let now = 40 * 60 * 1_000;
        let a = s.maybe_remind(now);
        let b = s.maybe_remind(now);
        assert_eq!(a, b);
    }

    #[test]
    fn from_app_config_sets_interval_and_min_gap() {
        let s = PostureScheduler::from_app_config(&PostureConfig { interval_min: 15 }, 1234);
        assert_eq!(s.config.interval_ms, 15 * 60 * 1_000);
        assert_eq!(s.config.min_gap_ms, 1234);
    }
}
