//! Reminder anti-collision coordinator (DP-3).
//!
//! Producers enqueue reminder candidates for the current engine tick; the
//! coordinator surfaces at most one. It is deliberately pure and
//! deterministic: all inputs are explicit (`now`, candidates, pause /
//! coalesce flags, and the coordinator's own last-fired bookkeeping).
//!
//! Rules:
//!   1. Single-flight: at most one surfaced reminder at a time; state
//!      machine breaks outrank all reminders.
//!   2. Minimum spacing (`quiet_gap = 90 s`): after any reminder or break
//!      dismissal, suppress the next non-break reminder for 90 s.
//!   3. Priority + anti-starvation: pick highest priority; bump a
//!      lower-priority reminder that has waited ≥ 2× its own interval.
//!   4. Coalescing: piggyback hydration/posture onto the tail of a
//!      nearby rest break instead of firing separately.
//!   5. Idle/quiet-hours gate: paused/quiet windows suppress reminders.

use std::collections::BTreeMap;

use crate::clock::Millis;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

/// Configuration knobs for the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordinatorConfig {
    /// Minimum spacing after any reminder or break dismissal.
    pub quiet_gap_ms: Millis,
    /// Starvation threshold expressed as a multiple of the reminder's
    /// interval. Default `2` means "bump after 2× interval waited".
    pub starvation_factor: u32,
    /// Window used by the engine to decide that an upcoming rest break
    /// is close enough to coalesce hydration/posture onto it.
    pub coalesce_window_ms: Millis,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            quiet_gap_ms: 90_000,
            starvation_factor: 2,
            coalesce_window_ms: 10 * 60 * 1_000,
        }
    }
}

/// Full DP-3 reminder coordinator.
#[derive(Debug, Clone)]
pub struct ReminderCoordinator {
    pub config: CoordinatorConfig,
    last_fired_at: BTreeMap<ReminderKind, Millis>,
    last_any_fired_or_dismissed_at: Option<Millis>,
    candidates: Vec<Reminder>,
    paused: bool,
    coalesce_defer_until: Option<Millis>,
}

impl Default for ReminderCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ReminderCoordinator {
    pub fn new() -> Self {
        Self::with_config(CoordinatorConfig::default())
    }

    pub fn with_config(config: CoordinatorConfig) -> Self {
        Self {
            config,
            last_fired_at: BTreeMap::new(),
            last_any_fired_or_dismissed_at: None,
            candidates: Vec::new(),
            paused: false,
            coalesce_defer_until: None,
        }
    }

    /// Reset candidates for a new engine tick.
    pub fn clear(&mut self) {
        self.candidates.clear();
    }

    /// Register a reminder candidate for the current tick.
    pub fn push(&mut self, r: Reminder) {
        self.candidates.push(r);
    }

    /// Suppress all reminders while paused or inside quiet hours.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Defer candidates until an upcoming rest break starts. The engine
    /// sets this when the rest break target is within the coalescing
    /// window; passing `None` clears the deferral.
    pub fn set_coalesce_window(&mut self, until_ms: Option<Millis>) {
        self.coalesce_defer_until = until_ms;
    }

    /// Record that a reminder actually reached the user. Candidate
    /// generation alone must not call this; the engine invokes it only
    /// after emitting `CoreEvent::FireReminder`.
    pub fn mark_fired(&mut self, kind: ReminderKind, now: Millis) {
        self.last_fired_at.insert(kind, now);
        self.last_any_fired_or_dismissed_at = Some(now);
    }

    /// Record that a break just disappeared. This enforces the same
    /// quiet gap after break dismissal as after reminders.
    pub fn mark_break_dismissed(&mut self, now: Millis) {
        self.last_any_fired_or_dismissed_at = Some(now);
    }

    /// Last time a particular reminder kind fired.
    pub fn last_fired_at(&self, kind: ReminderKind) -> Option<Millis> {
        self.last_fired_at.get(&kind).copied()
    }

    /// Pick the next reminder to fire, if any.
    pub fn next(&self, now: Millis) -> Option<Reminder> {
        if self.paused {
            return None;
        }

        if let Some(until) = self.coalesce_defer_until {
            if now < until {
                return None;
            }
        }

        if let Some(prev) = self.last_any_fired_or_dismissed_at {
            if now.saturating_sub(prev) < self.config.quiet_gap_ms {
                return None;
            }
        }

        self.candidates
            .iter()
            .copied()
            .filter(|r| r.earliest <= now)
            .map(|r| ScoredReminder::new(r, now, self.config.starvation_factor))
            .max_by(|a, b| a.cmp_for_pick(b))
            .map(|s| s.reminder)
    }
}

#[derive(Debug, Clone, Copy)]
struct ScoredReminder {
    reminder: Reminder,
    effective_priority: Priority,
    /// Integer starve score scaled by 1_000 to keep ordering stable
    /// without floating point comparisons.
    starve_score_milli: u64,
}

impl ScoredReminder {
    fn new(reminder: Reminder, now: Millis, starvation_factor: u32) -> Self {
        let interval = interval_hint_ms(reminder.kind).max(1);
        let waited = now.saturating_sub(reminder.earliest);
        let starve_score_milli = waited.saturating_mul(1_000) / interval;
        let starved = waited >= interval.saturating_mul(u64::from(starvation_factor.max(1)));
        let effective_priority = if starved {
            bump_priority(reminder.priority)
        } else {
            reminder.priority
        };
        Self {
            reminder,
            effective_priority,
            starve_score_milli,
        }
    }

    fn cmp_for_pick(&self, other: &Self) -> std::cmp::Ordering {
        self.effective_priority
            .cmp(&other.effective_priority)
            .then_with(|| self.starve_score_milli.cmp(&other.starve_score_milli))
            // Older due time wins ties.
            .then_with(|| other.reminder.earliest.cmp(&self.reminder.earliest))
            // Stable deterministic final tie-break by kind.
            .then_with(|| self.reminder.kind.cmp(&other.reminder.kind))
    }
}

fn bump_priority(p: Priority) -> Priority {
    match p {
        Priority::Low => Priority::Medium,
        Priority::Medium => Priority::High,
        Priority::High => Priority::High,
    }
}

/// Reminder's own interval for anti-starvation. The per-pillar scheduler
/// owns the precise cadence; the coordinator uses these v1 defaults as
/// stable hints so a candidate that has waited multiple cycles is bumped.
fn interval_hint_ms(kind: ReminderKind) -> Millis {
    match kind {
        ReminderKind::EyeBreak => 20 * 60 * 1_000,
        ReminderKind::Hydration => 30 * 60 * 1_000,
        ReminderKind::Posture => 40 * 60 * 1_000,
    }
}

/// Helper used by the schedulers — assign a stable priority slot to
/// a `ReminderKind`. Mirrors `From<ReminderKind> for Priority`.
pub fn priority_for(kind: ReminderKind) -> Priority {
    Priority::from(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reminder(kind: ReminderKind, earliest: Millis) -> Reminder {
        Reminder {
            kind,
            priority: Priority::from(kind),
            earliest,
        }
    }

    #[test]
    fn highest_priority_wins() {
        let mut c = ReminderCoordinator::new();
        c.push(reminder(ReminderKind::Hydration, 100));
        c.push(reminder(ReminderKind::EyeBreak, 100));
        c.push(reminder(ReminderKind::Posture, 100));
        let picked = c.next(100).unwrap();
        assert_eq!(picked.kind, ReminderKind::EyeBreak);
    }

    #[test]
    fn earliest_filter_excludes_future_candidates() {
        let mut c = ReminderCoordinator::new();
        c.push(reminder(ReminderKind::Hydration, 1_000));
        assert_eq!(c.next(500), None);
        let r = c.next(2_000).unwrap();
        assert_eq!(r.kind, ReminderKind::Hydration);
    }

    #[test]
    fn quiet_gap_suppresses_back_to_back_reminders_even_across_kinds() {
        let mut c = ReminderCoordinator::with_config(CoordinatorConfig {
            quiet_gap_ms: 90_000,
            ..CoordinatorConfig::default()
        });
        c.push(reminder(ReminderKind::Hydration, 0));
        assert_eq!(c.next(0).unwrap().kind, ReminderKind::Hydration);
        c.mark_fired(ReminderKind::Hydration, 0);

        c.clear();
        c.push(reminder(ReminderKind::Posture, 10_000));
        assert_eq!(c.next(10_000), None);

        let r = c.next(90_000).unwrap();
        assert_eq!(r.kind, ReminderKind::Posture);
    }

    #[test]
    fn starvation_bumps_lower_priority_one_slot() {
        let mut c = ReminderCoordinator::new();
        let now = 2 * 30 * 60 * 1_000;
        // Hydration waited 2× its 30-min interval, so Low → Medium.
        c.push(reminder(ReminderKind::Hydration, 0));
        // Fresh posture remains Medium; starvation score tie-break picks hydration.
        c.push(reminder(ReminderKind::Posture, now));
        let r = c.next(now).unwrap();
        assert_eq!(r.kind, ReminderKind::Hydration);
    }

    #[test]
    fn starvation_does_not_jump_more_than_one_priority_slot() {
        let mut c = ReminderCoordinator::new();
        let now = 10 * 30 * 60 * 1_000;
        c.push(reminder(ReminderKind::Hydration, 0));
        c.push(reminder(ReminderKind::EyeBreak, now));
        let r = c.next(now).unwrap();
        assert_eq!(r.kind, ReminderKind::EyeBreak);
    }

    #[test]
    fn coalescing_defer_suppresses_until_break_time() {
        let mut c = ReminderCoordinator::new();
        c.set_coalesce_window(Some(10 * 60 * 1_000));
        c.push(reminder(ReminderKind::Hydration, 0));
        assert_eq!(c.next(9 * 60 * 1_000), None);
        assert_eq!(
            c.next(10 * 60 * 1_000).unwrap().kind,
            ReminderKind::Hydration
        );
    }

    #[test]
    fn paused_suppresses_all_candidates() {
        let mut c = ReminderCoordinator::new();
        c.set_paused(true);
        c.push(reminder(ReminderKind::EyeBreak, 0));
        assert_eq!(c.next(0), None);
        c.set_paused(false);
        assert!(c.next(0).is_some());
    }

    #[test]
    fn mark_fired_records_per_kind_timestamp() {
        let mut c = ReminderCoordinator::new();
        c.mark_fired(ReminderKind::Posture, 42);
        assert_eq!(c.last_fired_at(ReminderKind::Posture), Some(42));
        assert_eq!(c.last_fired_at(ReminderKind::Hydration), None);
    }

    #[test]
    fn break_dismissal_enforces_quiet_gap() {
        let mut c = ReminderCoordinator::new();
        c.mark_break_dismissed(1_000);
        c.push(reminder(ReminderKind::Hydration, 1_001));
        assert_eq!(c.next(1_001), None);
        assert!(c.next(91_000).is_some());
    }

    #[test]
    fn clear_drops_tick_candidates() {
        let mut c = ReminderCoordinator::new();
        c.push(reminder(ReminderKind::Hydration, 0));
        assert!(c.next(0).is_some());
        c.clear();
        assert_eq!(c.next(0), None);
    }

    #[test]
    fn default_config_matches_section_7g() {
        let c = CoordinatorConfig::default();
        assert_eq!(c.quiet_gap_ms, 90_000);
        assert_eq!(c.starvation_factor, 2);
        assert_eq!(c.coalesce_window_ms, 10 * 60 * 1_000);
    }
}
