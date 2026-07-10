//! `BreakRecord` — DTO for a single break (micro or rest).
//!
//! See implementation plan §8a. Landed as a placeholder struct in M1
//! so the rest of the workspace compiles. The SQLite migration and
//! the round-trip from `sqlx` land in M6.

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::session::state::BreakKind;

/// Outcome class for `BreakRecord`. Stored verbatim as TEXT in SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakOutcome {
    Completed,
    Skipped,
    Postponed,
    Natural,
}

impl BreakOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            BreakOutcome::Completed => "completed",
            BreakOutcome::Skipped => "skipped",
            BreakOutcome::Postponed => "postponed",
            BreakOutcome::Natural => "natural",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreakRecord {
    pub id: Option<i64>,
    pub session_id: i64,
    pub kind: BreakKind,
    pub scheduled_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub ended_at: Option<Timestamp>,
    pub outcome: BreakOutcome,
    pub exercise_id: Option<String>,
}

impl BreakRecord {
    pub fn new(session_id: i64, kind: BreakKind, scheduled_at: Timestamp) -> Self {
        Self {
            id: None,
            session_id,
            kind,
            scheduled_at,
            started_at: None,
            ended_at: None,
            outcome: BreakOutcome::Completed,
            exercise_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_outcome_strings() {
        assert_eq!(BreakOutcome::Completed.as_str(), "completed");
        assert_eq!(BreakOutcome::Skipped.as_str(), "skipped");
        assert_eq!(BreakOutcome::Postponed.as_str(), "postponed");
        assert_eq!(BreakOutcome::Natural.as_str(), "natural");
    }

    #[test]
    fn new_break_record_defaults() {
        let r = BreakRecord::new(7, BreakKind::Micro, Timestamp(1_000));
        assert_eq!(r.id, None);
        assert_eq!(r.outcome, BreakOutcome::Completed);
        assert_eq!(r.exercise_id, None);
    }
}
