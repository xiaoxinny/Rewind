//! `rewind-adapters` — real OS adapter implementations behind the
//! traits defined in `rewind_core::ports`.
//!
//! M0 is a stub: no real adapters yet. Each module lands in its
//! milestone:
//!   * `idle/user_idle.rs` (M2): `user-idle` crate, Win/macOS/X11.
//!   * `idle/wayland.rs`   (M2): `ext-idle-notify-v1` / `kwin_idle`.
//!   * `idle/degraded.rs`  (M2): timer-only fallback for GNOME Wayland.
//!   * `notification.rs`   (M3): wraps `tauri-plugin-notification`.
//!   * `tray.rs`           (M1): wraps `TrayIconBuilder`.
//!   * `overlay.rs`        (M3): wraps `WebviewWindowBuilder`.
//!   * `autostart.rs`      (M6): wraps `tauri-plugin-autostart`.

pub mod idle;
pub mod overlay;

// TODO M1: add `tray` module and `pub mod tray`.
// TODO M2: add `idle/user_idle`, `idle/wayland`, `idle/degraded` and
// TODO M2:   wire the per-OS `IdleSource` factory.
// TODO M3: add `notification` module (Tauri notification bridge).
// TODO M6: add `autostart` module.
