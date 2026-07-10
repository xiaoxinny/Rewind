//! `rewind-adapters` — real OS adapter implementations behind the
//! traits defined in `rewind_core::ports`.
//!
//! Landed:
//!   * `idle/user_idle.rs` (M2): `user-idle` crate, Win/macOS/X11.
//!   * `idle/wayland.rs`   (M2): Wayland-protocol scaffolding; KWin/Sway
//!                                path is a follow-up (see M2.md).
//!   * `idle/degraded.rs`  (M2): timer-only fallback for GNOME Wayland.
//!   * `idle/mod.rs`       (M2): per-OS factory `pick()`.
//!
//! Still stubbed:
//!   * `notification.rs`   (M3): wraps `tauri-plugin-notification`.
//!   * `tray.rs`           (M1): wraps `TrayIconBuilder`.
//!   * `overlay.rs`        (M3): wraps `WebviewWindowBuilder`.
//!   * `autostart.rs`      (M6): wraps `tauri-plugin-autostart`.

pub mod idle;

// --- M2: idle adapters & factory ---------------------------------------
pub use idle::{pick as pick_idle_source, DegradedIdleSource, TimerOnlyIdleSource};

#[cfg(feature = "x11-idle")]
pub use idle::UserIdleSource;

pub use idle::WaylandIdleSource;

// TODO M1: add `tray` module and `pub mod tray`.
// TODO M3: add `notification` module.
// TODO M6: add `autostart` module.
