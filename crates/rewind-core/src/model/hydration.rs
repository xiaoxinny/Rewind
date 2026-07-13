//! `HydrationEntry` — DTO for a single water-log.
//!
//! Serialised across the IPC bridge and persisted in SQLite.
//! The struct mirrors the `hydration` table schema in
//! `crates/rewind-storage/src/migrations/0001_init.sql`.

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HydrationSource {
    Reminder,
    Manual,
}

impl HydrationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            HydrationSource::Reminder => "reminder",
            HydrationSource::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HydrationEntry {
    pub id: Option<i64>,
    pub logged_at: Timestamp,
    pub amount_ml: u32,
    pub source: HydrationSource,
}

impl HydrationEntry {
    pub fn manual(amount_ml: u32, logged_at: Timestamp) -> Self {
        Self {
            id: None,
            logged_at,
            amount_ml,
            source: HydrationSource::Manual,
        }
    }
}
