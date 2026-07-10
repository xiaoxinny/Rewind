//! `rewind-adapters` — real OS adapter implementations behind the
//! traits defined in `rewind_core::ports`.
//!
//! Landed:
//!   * `idle/user_idle.rs` (M2): `user-idle` crate, Win/macOS/X11.
//!     Gated behind the `x11-idle` feature (off by default) so the
//!     Tauri dylib doesn't link libX11.a.
//!   * `idle/wayland.rs`   (M2): Wayland-protocol scaffolding; KWin/Sway
//!                                path is a follow-up (see M2.md).
//!   * `idle/degraded.rs`  (M2): timer-only fallback for GNOME Wayland.
//!   * `idle/mod.rs`       (M2): per-OS factory `pick()`.
//!
//! Scope (per the plan §4): `rewind-adapters` holds the *platform-only*
//! adapter (idle). The Tauri-bound adapters (tray, overlay,
//! notification, autostart) live in `src-tauri/src/` because they
//! depend on Tauri types — that crate already pulls `tauri` so
//! there's no new dependency to thread through. See
//! `src-tauri/src/overlay_adapter.rs`, `src-tauri/src/adapters.rs`,
//! and `src-tauri/src/lib.rs` (the `build_tray` + `register_kill_switch`
//! helpers) for the Tauri-bound side of things.

pub mod idle;

// --- M2: idle adapters & factory ---------------------------------------
pub use idle::{pick as pick_idle_source, DegradedIdleSource, TimerOnlyIdleSource};

#[cfg(feature = "x11-idle")]
pub use idle::UserIdleSource;

pub use idle::WaylandIdleSource;
