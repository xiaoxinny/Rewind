//! Reminder anti-collision coordinator (DP-3).
//!
//! Four producers enqueue candidates; the coordinator surfaces at most
//! one per tick. Rules (§7g):
//!   1. Single-flight: at most one surfaced reminder at a time; state
//!      machine breaks outrank all reminders.
//!   2. Minimum spacing (`quiet_gap = 90 s`): after any reminder/break
//!      dismisses, suppress the next non-break reminder for 90 s.
//!   3. Priority + anti-starvation: pick highest priority; bump a
//!      lower-priority reminder that has waited > 2× its own interval.
//!   4. Coalescing: piggyback hydration/posture onto the tail of a
//!      rest break when one is due within ±10 min.
//!   5. Idle/quiet-hours gate: if `Paused` or inside configured quiet
//!      hours, queue or drop per policy.
//!
//! M1 keeps this as a placeholder struct + minimal surface so the
//! Engine can hold one. The full arbitration logic lands in M5.

use crate::clock::Millis;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};

/// Configuration knobs for the coordinator. M1 ships defaults that
/// mirror §7g verbatim; the per-pillar schedulers (hydration,
/// posture) plug in in M5.
#[derive(Debug, Clone, Copy)]
pub struct CoordinatorConfig {
    pub quiet_gap_ms: Millis,
    pub starvation_factor: u32,
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

/// Placeholder coordinator. Holds the candidates the engine has
/// collected so far and exposes `next(now, last_fired_at)` once the
/// per-pillar schedulers exist (M5).
#[derive(Debug, Default, Clone)]
pub struct ReminderCoordinator {
    pub config: CoordinatorConfig,
    candidates: Vec<Reminder>,
}

impl ReminderCoordinator {
    pub fn new() -> Self {
        Self {
            config: CoordinatorConfig::default(),
            candidates: Vec::new(),
        }
    }

    pub fn with_config(config: CoordinatorConfig) -> Self {
        Self {
            config,
            candidates: Vec::new(),
        }
    }

    /// Reset for a new tick (call once per `tick`).
    pub fn clear(&mut self) {
        self.candidates.clear();
    }

    /// Register a reminder candidate. The coordinator picks the
    /// highest-priority one due at `now`.
    pub fn push(&mut self, r: Reminder) {
        self.candidates.push(r);
    }

    /// Pick the next reminder to fire, if any. M5 implements the full
    /// arbitration rules; for now we just return the highest-priority
    /// candidate whose `earliest <= now`.
    pub fn next(&self, now: Millis) -> Option<Reminder> {
        self.candidates
            .iter()
            .filter(|r| r.earliest <= now)
            .copied()
            .max_by_key(|r| {
                // Higher priority wins; ties broken by earliest time
                // (older first). We sort descending on priority.
                (r.priority, std::cmp::Reverse(Millis::MAX - r.earliest))
            })
    }
}

/// Helper used by the M5 schedulers — assign a stable priority slot to
/// a `ReminderKind`. Mirrors `From<ReminderKind> for Priority` but is
/// package-local to avoid an import cycle.
#[allow(dead_code)]
pub fn priority_for(kind: ReminderKind) -> Priority {
    match kind {
        ReminderKind::EyeBreak => Priority::High,
        ReminderKind::Posture => Priority::Medium,
        ReminderKind::Hydration => Priority::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_priority_wins() {
        let mut c = ReminderCoordinator::new();
        c.push(Reminder {
            kind: ReminderKind::Hydration,
            priority: Priority::Low,
            earliest: 100,
        });
        c.push(Reminder {
            kind: ReminderKind::EyeBreak,
            priority: Priority::High,
            earliest: 100,
        });
        c.push(Reminder {
            kind: ReminderKind::Posture,
            priority: Priority::Medium,
            earliest: 100,
        });
        let picked = c.next(100).unwrap();
        assert_eq!(picked.kind, ReminderKind::EyeBreak);
    }

    #[test]
    fn earliest_filter_excludes_future_candidates() {
        let mut c = ReminderCoordinator::new();
        c.push(Reminder {
            kind: ReminderKind::Hydration,
            priority: Priority::Low,
            earliest: 1_000,
        });
        assert_eq!(c.next(500), None);
        let r = c.next(2_000).unwrap();
        assert_eq!(r.kind, ReminderKind::Hydration);
    }

    #[test]
    fn default_config_matches_section_7g() {
        let c = CoordinatorConfig::default();
        assert_eq!(c.quiet_gap_ms, 90_000);
        assert_eq!(c.starvation_factor, 2);
        assert_eq!(c.coalesce_window_ms, 10 * 60 * 1_000);
    }
}
