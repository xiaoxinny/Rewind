//! `SessionRecord` — DTO for a focus session.
//!
//! See implementation plan §8a. Landed as a placeholder struct in M1
//! so the rest of the workspace compiles. The SQLite migration lands
//! in M6.

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEndReason {
    Completed,
    IdleReset,
    Quit,
}

impl SessionEndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionEndReason::Completed => "completed",
            SessionEndReason::IdleReset => "idle_reset",
            SessionEndReason::Quit => "quit",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: Option<i64>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub active_ms: u64,
    pub end_reason: Option<SessionEndReason>,
}

impl SessionRecord {
    pub fn new(started_at: Timestamp) -> Self {
        Self {
            id: None,
            started_at,
            ended_at: None,
            active_ms: 0,
            end_reason: None,
        }
    }
}
