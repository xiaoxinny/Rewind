//! Adapter trait boundary — the seams between `rewind-core` and the
//! outside world (OS, UI, persistence).
//!
//! See implementation plan §7b. Each port has a real impl in
//! `rewind-adapters` (or `rewind-storage` for `HistoryRepo`) and a
//! fake in tests.

use std::time::Duration;

use thiserror::Error;

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
pub trait IdleSource {
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

/// Append-only history access. Real impl lives in `rewind-storage`
/// (lands in M6). For M1 a no-op fake is acceptable because the
/// engine itself does not depend on it (the shell would).
pub trait HistoryRepo {
    fn append_session(&self, s: &SessionRecord);
    fn append_break(&self, b: &BreakRecord);
    fn append_hydration(&self, h: &HydrationEntry);
    fn upsert_daily(&self, a: &DailyAggregate);
    fn today(&self) -> DailyAggregate;
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
