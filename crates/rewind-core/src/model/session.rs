//! `SessionRecord` — DTO for a focus session.
//!
//! Serialised across the IPC bridge and persisted in SQLite.
//! The struct mirrors the `session` table schema in
//! `crates/rewind-storage/src/migrations/0001_init.sql`.

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
