//! Hydration scheduler (DP-4).
//!
//! Adaptive, **capped** reminders — over-reminding risks overhydration
//! (hyponatremia). The scheduler keeps only local bookkeeping and is
//! deterministic: `maybe_remind` is a pure query; callers record a real
//! fire with [`HydrationScheduler::mark_reminded`].

use crate::clock::Millis;
use crate::config::HydrationConfig;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

const MINUTES_PER_DAY: u32 = 24 * 60;
const MS_PER_MINUTE: Millis = 60_000;

/// Configuration knobs. Defaults mirror §7h verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Stateful hydration reminder scheduler.
#[derive(Debug, Default, Clone)]
pub struct HydrationScheduler {
    pub config: HydrationSchedulerConfig,
    consumed_ml: u32,
    last_log_mono: Option<Millis>,
    last_reminder_mono: Option<Millis>,
    today_log: Vec<(Millis, u32)>,
}

impl HydrationScheduler {
    pub fn new() -> Self {
        Self::with_config(HydrationSchedulerConfig::default())
    }

    pub fn with_config(config: HydrationSchedulerConfig) -> Self {
        Self {
            config,
            consumed_ml: 0,
            last_log_mono: None,
            last_reminder_mono: None,
            today_log: Vec::new(),
        }
    }

    pub fn from_app_config(cfg: &HydrationConfig, min_gap_ms: u32) -> Self {
        Self::with_config(HydrationSchedulerConfig {
            goal_ml: cfg.goal_ml,
            glass_ml: cfg.glass_ml,
            min_gap_ms: Millis::from(min_gap_ms),
        })
    }

    pub fn update_config(&mut self, config: HydrationSchedulerConfig) {
        self.config = config;
    }

    pub fn consumed(&self) -> u32 {
        self.consumed_ml
    }

    pub fn goal(&self) -> u32 {
        self.config.goal_ml
    }

    pub fn last_log_mono(&self) -> Option<Millis> {
        self.last_log_mono
    }

    pub fn last_reminder_mono(&self) -> Option<Millis> {
        self.last_reminder_mono
    }

    pub fn today_log(&self) -> &[(Millis, u32)] {
        &self.today_log
    }

    /// Log water consumption (called from `CoreCommand::LogWater`). A
    /// quick-log counts as a hydration touch, so it defers the next
    /// reminder by at least the configured minimum gap.
    pub fn log_water(&mut self, amount_ml: u32, now: Millis) {
        self.consumed_ml = self.consumed_ml.saturating_add(amount_ml);
        self.last_log_mono = Some(now);
        self.today_log.push((now, amount_ml));
    }

    /// Record that a hydration reminder was actually surfaced.
    pub fn mark_reminded(&mut self, now: Millis) {
        self.last_reminder_mono = Some(now);
    }

    /// Reset for a new local day (called from `Engine` when the local
    /// `Date` changes; no 24-hour assumptions here).
    pub fn reset_day(&mut self) {
        self.consumed_ml = 0;
        self.last_log_mono = None;
        self.last_reminder_mono = None;
        self.today_log.clear();
    }

    /// Returns `Some(reminder)` if it's time to nudge the user about
    /// hydration. `waking_window` is `(start_minute, end_minute)` in the
    /// local clock (minutes from midnight), computed by `AppConfig` /
    /// the engine.
    pub fn maybe_remind(&self, now: Millis, waking_window: (u32, u32)) -> Option<Reminder> {
        if self.consumed_ml >= self.config.goal_ml {
            return None;
        }

        let window = WindowProgress::from_now(now, waking_window)?;
        if window.total_minutes == 0 {
            return None;
        }

        if self.last_activity_mono().is_none() && window.elapsed_minutes == 0 {
            return None;
        }

        // Responsible-hydration cap: if the user is at least 95% of
        // the ideal pace for this point in the waking window, don't nag.
        let ideal_ml = (u64::from(window.elapsed_minutes) * u64::from(self.config.goal_ml))
            / u64::from(window.total_minutes);
        if u64::from(self.consumed_ml) * 100 >= ideal_ml.saturating_mul(95) {
            return None;
        }

        let remaining_ml = self.config.goal_ml.saturating_sub(self.consumed_ml);
        let remaining_intervals = (window.remaining_minutes / 30).max(1);
        let adaptive_interval_min = (remaining_ml / remaining_intervals).max(30);
        let effective_gap_ms = self
            .config
            .min_gap_ms
            .max(Millis::from(adaptive_interval_min).saturating_mul(MS_PER_MINUTE));

        let elapsed_since_anchor = match self.last_activity_mono() {
            Some(anchor) => now.saturating_sub(anchor),
            None => Millis::from(window.elapsed_minutes).saturating_mul(MS_PER_MINUTE),
        };

        if elapsed_since_anchor < effective_gap_ms {
            return None;
        }

        let earliest = now
            .saturating_sub(elapsed_since_anchor)
            .saturating_add(effective_gap_ms);
        Some(Reminder {
            kind: ReminderKind::Hydration,
            priority: Priority::Low,
            earliest,
        })
    }

    fn last_activity_mono(&self) -> Option<Millis> {
        match (self.last_log_mono, self.last_reminder_mono) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowProgress {
    elapsed_minutes: u32,
    remaining_minutes: u32,
    total_minutes: u32,
}

impl WindowProgress {
    fn from_now(now: Millis, waking_window: (u32, u32)) -> Option<Self> {
        let start = waking_window.0 % MINUTES_PER_DAY;
        let end = waking_window.1 % MINUTES_PER_DAY;
        let total = window_span_minutes(start, end)?;
        let current = local_minute_from_unix_ms(now);
        let elapsed = elapsed_minutes_in_window(current, start, end)?;
        let remaining = total.saturating_sub(elapsed).max(1);
        Some(Self {
            elapsed_minutes: elapsed,
            remaining_minutes: remaining,
            total_minutes: total,
        })
    }
}

fn window_span_minutes(start: u32, end: u32) -> Option<u32> {
    match end.cmp(&start) {
        std::cmp::Ordering::Greater => Some(end - start),
        std::cmp::Ordering::Less => Some(MINUTES_PER_DAY - start + end),
        std::cmp::Ordering::Equal => None,
    }
}

fn elapsed_minutes_in_window(current: u32, start: u32, end: u32) -> Option<u32> {
    if start < end {
        if (start..end).contains(&current) {
            Some(current - start)
        } else {
            None
        }
    } else if start > end {
        if current >= start {
            Some(current - start)
        } else if current < end {
            Some(MINUTES_PER_DAY - start + current)
        } else {
            None
        }
    } else {
        None
    }
}

fn local_minute_from_unix_ms(ms: Millis) -> u32 {
    let secs = (ms / 1_000).min(i64::MAX as Millis) as i64;
    let utc =
        time::OffsetDateTime::from_unix_timestamp(secs).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = utc.to_offset(offset).time();
    u32::from(local.hour()) * 60 + u32::from(local.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(goal_ml: u32) -> HydrationSchedulerConfig {
        HydrationSchedulerConfig {
            goal_ml,
            glass_ml: 250,
            min_gap_ms: 30 * 60 * 1_000,
        }
    }

    fn local_ms_at(hour: u8, minute: u8) -> Millis {
        let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        let base_utc_midnight = 10_000_i64 * 24 * 60 * 60;
        let local_secs = i64::from(hour) * 3_600 + i64::from(minute) * 60;
        let utc_secs = base_utc_midnight + local_secs - i64::from(offset.whole_seconds());
        utc_secs as Millis * 1_000
    }

    #[test]
    fn log_water_increments_total_and_records_today_log() {
        let mut s = HydrationScheduler::new();
        s.log_water(250, 1_000);
        s.log_water(250, 2_000);
        assert_eq!(s.consumed(), 500);
        assert_eq!(s.today_log(), &[(1_000, 250), (2_000, 250)]);
        assert_eq!(s.last_log_mono(), Some(2_000));
    }

    #[test]
    fn adaptive_interval_first_morning_not_due_yet() {
        let s = HydrationScheduler::with_config(cfg(2_000));
        let now = local_ms_at(9, 15);
        assert_eq!(s.maybe_remind(now, (9 * 60, 21 * 60)), None);
    }

    #[test]
    fn adaptive_interval_after_quarter_of_day_due_when_behind_pace() {
        let mut s = HydrationScheduler::with_config(cfg(2_000));
        // 25% of a 2 L goal is 500 ml; 450 ml is behind the 95% cap.
        s.log_water(450, local_ms_at(9, 5));
        let now = local_ms_at(12, 0);
        let r = s.maybe_remind(now, (9 * 60, 21 * 60)).unwrap();
        assert_eq!(r.kind, ReminderKind::Hydration);
        assert!(r.earliest <= now);
    }

    #[test]
    fn cap_when_ahead_of_pace() {
        let mut s = HydrationScheduler::with_config(cfg(2_000));
        s.log_water(500, local_ms_at(9, 5));
        let now = local_ms_at(12, 0);
        assert_eq!(s.maybe_remind(now, (9 * 60, 21 * 60)), None);
    }

    #[test]
    fn min_gap_suppresses_back_to_back_reminders_after_mark_reminded() {
        let mut s = HydrationScheduler::with_config(cfg(250));
        let first = local_ms_at(10, 0);
        assert!(s.maybe_remind(first, (9 * 60, 21 * 60)).is_some());
        s.mark_reminded(first);
        assert_eq!(s.maybe_remind(first + 60_000, (9 * 60, 21 * 60)), None);
        assert!(s
            .maybe_remind(first + 31 * 60 * 1_000, (9 * 60, 21 * 60))
            .is_some());
    }

    #[test]
    fn quick_log_defers_next_reminder() {
        let mut s = HydrationScheduler::with_config(cfg(250));
        let log_at = local_ms_at(10, 0);
        s.log_water(25, log_at);
        assert_eq!(
            s.maybe_remind(log_at + 10 * 60_000, (9 * 60, 21 * 60)),
            None
        );
        assert!(s
            .maybe_remind(log_at + 31 * 60_000, (9 * 60, 21 * 60))
            .is_some());
    }

    #[test]
    fn reset_day_clears_state() {
        let mut s = HydrationScheduler::new();
        s.log_water(750, 1_000);
        s.mark_reminded(2_000);
        s.reset_day();
        assert_eq!(s.consumed(), 0);
        assert_eq!(s.today_log(), &[]);
        assert_eq!(s.last_log_mono(), None);
        assert_eq!(s.last_reminder_mono(), None);
    }

    #[test]
    fn outside_waking_window_is_suppressed() {
        let s = HydrationScheduler::with_config(cfg(250));
        assert_eq!(s.maybe_remind(local_ms_at(7, 0), (9 * 60, 21 * 60)), None);
        assert_eq!(s.maybe_remind(local_ms_at(22, 0), (9 * 60, 21 * 60)), None);
    }

    #[test]
    fn overnight_waking_window_handles_after_midnight() {
        let s = HydrationScheduler::with_config(cfg(250));
        let r = s.maybe_remind(local_ms_at(1, 30), (22 * 60, 6 * 60));
        assert!(r.is_some());
    }

    #[test]
    fn maybe_remind_is_pure_for_same_state() {
        let s = HydrationScheduler::with_config(cfg(250));
        let now = local_ms_at(10, 0);
        let a = s.maybe_remind(now, (9 * 60, 21 * 60));
        let b = s.maybe_remind(now, (9 * 60, 21 * 60));
        assert_eq!(a, b);
    }

    #[test]
    fn dst_spring_forward_23hour_day_does_not_crash() {
        let s = HydrationScheduler::with_config(cfg(2_000));
        let now = 23 * 60 * 60 * 1_000;
        let _ = s.maybe_remind(now, (0, 23 * 60));
    }

    #[test]
    fn dst_fall_back_25hour_day_does_not_crash() {
        let s = HydrationScheduler::with_config(cfg(2_000));
        let now = 25 * 60 * 60 * 1_000;
        let _ = s.maybe_remind(now, (0, 23 * 60));
    }

    #[test]
    fn from_app_config_derives_scheduler_config() {
        let cfg = HydrationConfig {
            goal_ml: 1800,
            glass_ml: 300,
            wake_start: "08:00".to_string(),
            wake_end: "20:00".to_string(),
        };
        let s = HydrationScheduler::from_app_config(&cfg, 1234);
        assert_eq!(s.config.goal_ml, 1800);
        assert_eq!(s.config.glass_ml, 300);
        assert_eq!(s.config.min_gap_ms, 1234);
    }
}
