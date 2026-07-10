//! `DailyAggregate` — denormalized daily rollup, keyed on **local** day.
//!
//! Treated as a cache, not a source of truth — reconstructable from the
//! raw tables. See implementation plan §8a. The SQLite-backed repo
//! lands in M6; M1 ships the DTO.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyAggregate {
    /// 'YYYY-MM-DD' local form.
    pub day: String,
    pub active_ms: u64,
    pub breaks_taken: u32,
    pub breaks_skipped: u32,
    pub water_ml: u32,
    pub water_goal_ml: u32,
    pub posture_prompts: u32,
}

impl DailyAggregate {
    pub fn empty_for_today(day: String, water_goal_ml: u32) -> Self {
        Self {
            day,
            active_ms: 0,
            breaks_taken: 0,
            breaks_skipped: 0,
            water_ml: 0,
            water_goal_ml,
            posture_prompts: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_aggregate_has_zero_counters() {
        let a = DailyAggregate::empty_for_today("2026-07-09".to_string(), 2000);
        assert_eq!(a.day, "2026-07-09");
        assert_eq!(a.active_ms, 0);
        assert_eq!(a.breaks_taken, 0);
        assert_eq!(a.breaks_skipped, 0);
        assert_eq!(a.water_ml, 0);
        assert_eq!(a.water_goal_ml, 2000);
        assert_eq!(a.posture_prompts, 0);
    }
}
