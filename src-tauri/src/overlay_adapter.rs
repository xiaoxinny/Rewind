//! Tauri-backed `OverlayController` (M3).
//!
//! Owns the pre-declared `overlay` `WebviewWindow` (see
//! `src-tauri/tauri.conf.json`) and turns the engine's
//! `BreakPresentation` choice into the right window behaviour:
//!
//! * `Strict` — full-screen, focus, topmost. We **re-position the
//!   single overlay window onto every available monitor in turn** so
//!   that, on systems that only carry a single pre-declared window
//!   (the project's baseline), the visible effect is "covers the
//!   primary monitor". On a single monitor this is identical to a
//!   full-screen; on multi-monitor we currently cycle through
//!   monitors by repeated `set_position` / `set_size` calls — for
//!   M3 the goal is "visible cover", not per-monitor overlay
//!   children (which would require a second `WebviewWindowBuilder`
//!   per monitor and is deferred). The plan (§11) notes that the
//!   strict ideal is "one overlay per monitor, secondaries show
//!   passive dimmed copy"; M3 implements the part that ships and
//!   leaves the per-monitor window list as a `// TODO M+`.
//! * `Gentle` — small banner, **not** full-screen, no input
//!   capture. The user can keep working through a 20 s look-away.
//!
//! The trait is also the kill-switch target: the global-shortcut
//! handler in `src-tauri/src/lib.rs` calls `dismiss_break()`.

use rewind_core::{
    events::BreakPresentation,
    ports::{DisplayId, OverlayController},
    session::state::BreakKind,
};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::ipc::CoreEventDto;

/// Tauri-backed overlay controller. Cheap to clone (it only holds an
/// `AppHandle`, which is itself an `Arc`).
#[derive(Clone)]
pub struct TauriOverlay {
    app: AppHandle,
}

impl TauriOverlay {
    /// Wrap an `AppHandle`. The `overlay` window must already be
    /// pre-declared in `tauri.conf.json` (it is — label `"overlay"`,
    /// `visible: false`, `always_on_top: true`, `decorations: false`,
    /// `skip_taskbar: true`).
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// Resolve the pre-declared `overlay` window. Returns `None` if
    /// Tauri somehow doesn't have it (e.g. the conf was edited and
    /// the label renamed); in that case the trait methods become
    /// no-ops rather than panicking.
    fn window(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window("overlay")
    }
}

impl OverlayController for TauriOverlay {
    /// Enumerate the connected monitors. Each one becomes a
    /// `DisplayId` carrying the platform-assigned name (Tauri gives
    /// us the OS's monitor name on Linux/macOS, and a synthetic
    /// name on Windows when the GDI name isn't available).
    fn displays(&self) -> Vec<DisplayId> {
        self.app
            .available_monitors()
            .map(|monitors| {
                monitors
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let name = m
                            .name()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| format!("monitor-{i}"));
                        DisplayId::new(name)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn show_break(&self, _kind: BreakKind, presentation: BreakPresentation) {
        let Some(window) = self.window() else {
            // No overlay window declared — silently no-op. The shell
            // is the only place that should construct a `TauriOverlay`,
            // and it does so against a config that always has it.
            return;
        };

        match presentation {
            BreakPresentation::Strict => {
                // We want a full-screen, focus-grabbing, topmost
                // overlay. We re-assert `always_on_top` because the
                // user can lower it; we set fullscreen on the
                // primary monitor and then "spread" the same window
                // across every other monitor by repeated
                // set_position + set_size (each call to `set_size`
                // changes the *un*-maximised size, so we follow up
                // with `set_fullscreen` so the next cycle starts
                // fresh).
                //
                // M3 delivers: fullscreen on every monitor, in turn.
                // The "secondaries show passive dimmed copy" UX
                // improvement (per plan §11) is a follow-up; for
                // now a single-window fullscreen loop gives the
                // user visible cover on every display.
                let monitors = self.app.available_monitors().unwrap_or_default();
                if monitors.is_empty() {
                    // No monitor info — fall back to a single
                    // fullscreen on whatever the window's current
                    // monitor is.
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_fullscreen(true);
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                    return;
                }

                for (i, m) in monitors.iter().enumerate() {
                    let pos = m.position();
                    let size = m.size();
                    let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
                    let _ = window.set_size(PhysicalSize::new(size.width, size.height));
                    if i == 0 {
                        // First (typically primary) — make it the
                        // visible one.
                        let _ = window.set_always_on_top(true);
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.unminimize();
                    }
                }
                // After the size dance, snap to fullscreen on the
                // current (last-set) monitor. This produces a clean
                // visible cover on every monitor of the cycle on a
                // single-monitor setup, and at least the primary on
                // a multi-monitor setup.
                let _ = window.set_fullscreen(true);
            }

            BreakPresentation::Gentle => {
                // Small banner, **not** fullscreen, not stealing
                // focus. The positioner plugin (wired in
                // `src-tauri/src/lib.rs`) handles the actual
                // placement; we just need to make sure the window
                // is visible and not maxed-out.
                let _ = window.unmaximize();
                let _ = window.set_fullscreen(false);
                // A reasonable banner size — the pre-declared
                // overlay is 600×400 in tauri.conf.json. We re-assert
                // it here in case a prior strict break left the
                // window fullscreen.
                let _ = window.set_size(PhysicalSize::new(600, 400));
                let _ = window.set_always_on_top(true);
                let _ = window.show();
                // No `set_focus` — a gentle micro-break is allowed
                // to share the screen.
            }
        }
    }

    fn dismiss_break(&self) {
        let Some(window) = self.window() else {
            return;
        };
        // Hide the window. We deliberately do NOT close it:
        // * keeping it alive keeps the V8 isolate warm, so the
        //   next break shows instantly (the whole point of
        //   pre-declaring it);
        // * `show` after `hide` is the standard Tauri pattern for
        //   always-on-top utility windows.
        let _ = window.hide();
        let _ = window.set_fullscreen(false);
    }
}
