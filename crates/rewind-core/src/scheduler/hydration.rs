//! Hydration scheduler (DP-4).
//!
//! Adaptive, **capped** reminders — over-reminding risks overhydration
//! (hyponatremia). See implementation plan §7h. The real adaptive
//! logic lands in M5; M1 keeps the type placeholders so the Engine
//! can compose without missing imports.

use crate::clock::Millis;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

/// Configuration knobs. Defaults mirror §7h verbatim.
#[derive(Debug, Clone, Copy)]
pub struct HydrationSchedulerConfig {
    pub goal_ml: u32,
    pub glass_ml: u32,
    pub min_gap_ms: Millis,
}

impl Default for HydrationSchedulerConfig {
    fn default() -> Self {
        Self {
            goal_ml: 2000,
            glass_ml: 250,
            min_gap_ms: 30 * 60 * 1_000,
        }
    }
}

/// Stateful hydration reminder scheduler. The data lives here so the
/// engine can `tick`/`reset` it without leaking bookkeeping into the
/// engine struct.
#[derive(Debug, Default, Clone)]
pub struct HydrationScheduler {
    pub config: HydrationSchedulerConfig,
    consumed_ml: u32,
    last_log_mono: Option<Millis>,
    last_reminder_mono: Option<Millis>,
}

impl HydrationScheduler {
    pub fn new() -> Self {
        Self {
            config: HydrationSchedulerConfig::default(),
            consumed_ml: 0,
            last_log_mono: None,
            last_reminder_mono: None,
        }
    }

    pub fn with_config(config: HydrationSchedulerConfig) -> Self {
        Self {
            config,
            consumed_ml: 0,
            last_log_mono: None,
            last_reminder_mono: None,
        }
    }

    pub fn consumed(&self) -> u32 {
        self.consumed_ml
    }

    pub fn goal(&self) -> u32 {
        self.config.goal_ml
    }

    /// Log water consumption (called from `CoreCommand::LogWater`).
    pub fn log_water(&mut self, amount_ml: u32, now: Millis) {
        self.consumed_ml = self.consumed_ml.saturating_add(amount_ml);
        self.last_log_mono = Some(now);
    }

    /// Reset for a new day (called from `Engine` at local midnight).
    pub fn reset_day(&mut self) {
        self.consumed_ml = 0;
        self.last_log_mono = None;
        self.last_reminder_mono = None;
    }

    /// Returns `Some(reminder)` if it's time to nudge the user about
    /// hydration. The M5 implementation will fold in ideal-pace
    /// capping; this stub enforces only the minimum-gap guarantee.
    pub fn maybe_remind(&mut self, now: Millis) -> Option<Reminder> {
        if let Some(prev) = self.last_reminder_mono {
            if now.saturating_sub(prev) < self.config.min_gap_ms {
                return None;
            }
        }
        self.last_reminder_mono = Some(now);
        Some(Reminder {
            kind: ReminderKind::Hydration,
            priority: Priority::Low,
            earliest: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_water_increments_total() {
        let mut s = HydrationScheduler::new();
        s.log_water(250, 1_000);
        s.log_water(250, 2_000);
        assert_eq!(s.consumed(), 500);
    }

    #[test]
    fn min_gap_suppresses_back_to_back_reminders() {
        let mut s = HydrationScheduler::new();
        let r1 = s.maybe_remind(0);
        assert!(r1.is_some());
        let r2 = s.maybe_remind(60_000);
        // 60s < 30 min default gap → suppressed.
        assert!(r2.is_none());
        let r3 = s.maybe_remind(31 * 60 * 1_000);
        assert!(r3.is_some());
    }

    #[test]
    fn reset_day_clears_state() {
        let mut s = HydrationScheduler::new();
        s.log_water(750, 1_000);
        s.reset_day();
        assert_eq!(s.consumed(), 0);
    }
}
