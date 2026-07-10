//! `rewind-storage` — SQLite-backed implementation of
//! `rewind_core::ports::HistoryRepo`.
//!
//! M0: stub. Lands in M6 with the first migration
//! (`migrations/0001_init.sql` per implementation plan §8a) and an
//! `sqlx::SqlitePool` factory.

pub mod repo;

// TODO M6: add `pub mod migrations;` exposing the embedded
// TODO M6:   `migrations/0001_init.sql` via `sqlx::migrate!`.
// TODO M6: add the concrete `SqliteHistoryRepo` and wire
// TODO M6:   `rewind_core::ports::HistoryRepo` to it.
