//! Adapter trait boundary — the seams between `rewind-core` and the
//! outside world (OS, UI, persistence).
//!
//! See implementation plan §7b. Each port has a real impl in
//! `rewind-adapters` (or `rewind-storage` for `HistoryRepo`) and a
//! fake in tests.

use std::time::Duration;

use thiserror::Error;

use crate::clock::Timestamp;
use crate::events::{BreakPresentation, Notification, TrayMenuItem, TrayStatus};
use crate::model::{
    aggregate::DailyAggregate, break_record::BreakRecord, hydration::HydrationEntry,
    session::SessionRecord,
};
use crate::session::state::BreakKind;

// ---------------------------------------------------------------------------
// Idle source (§7b, §7f DP-2)
// ---------------------------------------------------------------------------

/// How trustworthy the adapter considers its last reading. The engine
/// reads this on every tick and downgrades to **timer-only mode** when
/// the answer is anything but `Reliable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleReliability {
    /// Last `idle_time` reading is trustworthy. The engine may pause
    /// or reset based on it.
    Reliable,
    /// Best-effort only — usually fine but may lie. The engine keeps
    /// its idle-driven pause/reset logic off as a precaution.
    Unreliable,
    /// No data available at all. Use timer-only mode (no idle
    /// pause/reset; the UI shows an honest "screen-time tracking
    /// limited on this session" note).
    Unavailable,
}

#[derive(Debug, Clone, Error)]
pub enum IdleError {
    /// The platform-specific adapter reported a transient failure. The
    /// engine should retry next tick rather than degrading.
    #[error("idle adapter is temporarily unavailable: {0}")]
    Transient(String),

    /// The adapter is fundamentally unsupported on this platform /
    /// session type (e.g. GNOME Wayland). Callers should switch to a
    /// `degraded` source and set `reliability()` to `Unavailable`.
    #[error("idle detection is unavailable on this platform: {0}")]
    Unsupported(String),
}

/// The single most important adapter. Real impls in `rewind-adapters`
/// (Win/macOS/X11 via `user-idle`; Wayland via `ext-idle-notify-v1`;
/// `degraded::DegradedIdleSource` for GNOME/headless).
///
/// Synchronous & cheap; the shell layer turns this into tokio async.
/// `Send + Sync` because the engine lives behind an `Arc<Mutex<…>>`
/// in a tokio task spawned from Tauri's `setup` closure.
pub trait IdleSource: Send + Sync {
    /// Seconds since last user input. **No window titles, app names,
    /// keystrokes** — only the duration.
    fn idle_time(&self) -> Result<Duration, IdleError>;

    /// Trustworthiness of `idle_time()`. Engine reads this once at
    /// startup and on each `ConfigUpdated`.
    fn reliability(&self) -> IdleReliability;
}

// ---------------------------------------------------------------------------
// Notifier (§7b, §7g)
// ---------------------------------------------------------------------------

/// Gentle-mode notification surface (system toast on platforms that
/// support it; in-app banner as a fallback). The Strictness-bypass
/// break overlay has its own adapter (`OverlayController`).
pub trait Notifier {
    fn notify(&self, n: Notification);
}

// ---------------------------------------------------------------------------
// Tray (§7b)
// ---------------------------------------------------------------------------

/// Tray icon + tooltip + menu surface. Lives behind the trait so a
/// fake (`NoopTray`) can be used in headless tests.
pub trait Tray {
    /// Refresh the icon + tooltip. Called whenever the engine emits
    /// `CoreEvent::TrayStatus`.
    fn set_status(&self, status: TrayStatus);

    /// Rebuild the menu. Items are clickable; their ids come back as
    /// `CoreCommand`s via the shell.
    fn set_menu(&self, items: &[TrayMenuItem]);
}

// ---------------------------------------------------------------------------
// Overlay (§11, M3+)
// ---------------------------------------------------------------------------

/// Identifies a monitor. Format is platform-defined.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayId(pub String);

impl DisplayId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for DisplayId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Owns the overlay windows. M1 keeps a trivial `TauriOverlay` that
/// just records the request — the real window management lands in M3.
pub trait OverlayController {
    fn displays(&self) -> Vec<DisplayId>;
    fn show_break(&self, kind: BreakKind, p: BreakPresentation);
    fn dismiss_break(&self);
}

// ---------------------------------------------------------------------------
// Autostart (§4, M6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Error)]
pub enum AutostartError {
    #[error("autostart backend rejected the change: {0}")]
    Backend(String),
}

pub trait Autostart {
    fn set_enabled(&self, on: bool) -> Result<(), AutostartError>;
    fn is_enabled(&self) -> bool;
}

// ---------------------------------------------------------------------------
// History repo (§8a, M6 — stub satisfied today via `NoopHistoryRepo`)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// History repo errors (§8a, M6).
// ---------------------------------------------------------------------------

/// Errors a `HistoryRepo` can report. Defined here (in core, not in
/// `rewind-storage`) so the trait boundary stays free of sqlx. The
/// storage crate wraps `sqlx::Error` into `Backend` for the shell.
#[derive(Debug, Clone, Error)]
pub enum HistoryRepoError {
    /// Migration runner failed (file missing / checksum mismatch /
    /// unsupported target).
    #[error("history migration failed: {0}")]
    Migration(String),

    /// The DB driver returned an error.
    #[error("history backend error: {0}")]
    Backend(String),

    /// A row was malformed (e.g. invalid `BreakKind` text label)
    /// and could not be deserialised back into the typed model.
    #[error("history row malformed: {0}")]
    Malformed(String),
}

/// Convenience alias used by `HistoryRepo` implementations.
pub type RepoResult<T> = std::result::Result<T, HistoryRepoError>;

// ---------------------------------------------------------------------------
// History repo (§8a, M6 — `SqliteHistoryRepo` is in `rewind-storage`)
// ---------------------------------------------------------------------------

/// Append-only history access. Real impl lives in `rewind-storage`
/// (`SqliteHistoryRepo` — see `crates/rewind-storage/src/repo.rs`).
/// The engine does not depend on this trait; the shell does. The
/// shape mirrors `crates/rewind-storage/src/migrations/0001_init.sql`
/// 1-for-1.
///
/// **M6** — the trait went from unit-returning to `async fn` so a
/// SQLite I/O wait can never block the tick loop. The engine is
/// unaffected (it does not depend on this trait). The shell calls
/// these from inside the tokio runtime.
#[async_trait::async_trait]
pub trait HistoryRepo: Send + Sync {
    /// Persist a single session record. Returns the new row id.
    /// Implementations are expected to be **fail-loud** — a database
    /// outage should never be silently swallowed.
    async fn append_session(&self, s: &SessionRecord) -> RepoResult<i64>;

    /// Persist a single break record. Returns the new row id.
    async fn append_break(&self, b: &BreakRecord) -> RepoResult<i64>;

    /// Persist a single hydration entry. Returns the new row id.
    async fn append_hydration(&self, h: &HydrationEntry) -> RepoResult<i64>;

    /// Insert or update the daily rollup for `a.day`. Idempotent —
    /// called on every engine `Tick()` so re-issuing with the same
    /// counters must be safe.
    async fn upsert_daily(&self, a: &DailyAggregate) -> RepoResult<()>;

    /// Read today's rollup. Returns a zero-counter
    /// `DailyAggregate` for "today" when no row exists yet.
    async fn today(&self, now: Timestamp) -> RepoResult<DailyAggregate>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_id_smoke() {
        let id = DisplayId::new("monitor-0");
        assert_eq!(id.to_string(), "monitor-0");
        assert_ne!(id, DisplayId::new("monitor-1"));
    }
}
