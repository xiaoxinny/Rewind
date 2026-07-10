//! `rewind-storage` — SQLite-backed implementation of
//! `rewind_core::ports::HistoryRepo`.
//!
//! M0–M5: stub. M6 lands the schema (`migrations/0001_init.sql` per
//! implementation plan §8a), the `SqliteHistoryRepo`, the shell-side
//! `open_db()` helper, and the test suite.
//!
//! The crate **does not depend on `tauri`** — it uses `sqlx::SqlitePool`
//! directly. The shell layer (`src-tauri`) opens the connection at
//! boot and hands the pool to the repo. See implementation plan §4.

pub mod repo;
pub mod storage_error;

pub use repo::{connect, migrate, SqliteHistoryRepo};
pub use storage_error::{Result as StorageResult, StorageError};

// Re-export the migration directory so `sqlx::migrate!()` can be
// invoked from anywhere (including shell-side tests).
//
// `sqlx::migrate!()` macro looks at compile-time for a migrations
// folder relative to the crate that calls it. We expose a thin
// wrapper so the shell can drive migrations through the storage crate
// without having to know the directory path.
//
// The path is relative to the *storage* crate root because that's
// where `sqlx::migrate!` resolves it. Don't move the migrations
// elsewhere without updating this constant.
pub const MIGRATIONS_DIR: &str = "./migrations";
