//! Background tick loop — drives the engine at 1 Hz.
//!
//! Lives in its own tokio task spawned from `lib::run::setup`. Each
//! tick: read the idle observation from the chosen `IdleSource`,
//! advance the engine, and dispatch the resulting `CoreEvent`s to
//! (a) the frontend via `app.emit("core-event", …)` and (b) the tray
//! adapter so the tooltip stays in sync.
//!
//! The loop has no explicit shutdown — it runs for the lifetime of
//! the Tauri process and exits when the process does. The `stop()`
//! function exists for future graceful-shutdown support (M6).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rewind_core::{Clock, Engine};
use rewind_core::ports::OverlayController;
use tauri::{AppHandle, Emitter, Manager};

use crate::adapters::Adapters;
use crate::ipc::CoreEventDto;

/// Spawn the tick loop as a tokio task. Returns the JoinHandle so
/// callers can `abort()` it on shutdown.
pub fn start(
    app: AppHandle,
    engine: Arc<Mutex<Engine>>,
    adapters: Adapters,
    clock: Arc<dyn Clock>,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            let now = clock.now();
            let monotonic = clock.monotonic();
            let idle = adapters.idle.idle_time().unwrap_or_default();

            // Drive the engine. Hold the lock for the duration of the
            // call so the IPC commands can't see a torn state.
            let events = {
                let mut guard = match engine.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                guard.tick(now, idle)
            };

            // 1) Emit each event to the frontend.
            for ev in &events {
                let dto = CoreEventDto::from(ev.clone());
                let _ = app.emit("core-event", dto);
            }

            // 2) Side-effects via adapters.
            for ev in &events {
                use rewind_core::CoreEvent::*;
                match ev {
                    ShowBreak { kind, presentation, exercise_id } => {
                        adapters.overlay.show_break(*kind, presentation.clone());
                        // Forward the exercise id to the overlay window
                        // via a separate event so the Svelte overlay can
                        // pick which component to render.
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.emit(
                                "show-exercise",
                                serde_json::json!({
                                    "kind": kind.to_string(),
                                    "presentation": presentation,
                                    "exerciseId": exercise_id,
                                }),
                            );
                        }
                    }
                    DismissBreak => {
                        adapters.overlay.dismiss_break();
                    }
                    TrayStatus(status) => {
                        if let Some(tray) = app.tray_by_id("rewind-tray") {
                            let _ = tray.set_tooltip(Some(status.tooltip_line.clone()));
                        }
                    }
                    HydrationUpdated(p) => {
                        // Tray icon could change here in the future
                        // (e.g. a water-droplet overlay when behind on
                        // hydration). For M1 the tooltip is enough.
                    }
                    Tick { remaining, .. } => {
                        // Update tray tooltip on each tick so it counts
                        // down. We always recompute the full text from
                        // the engine state to keep things simple.
                        let guard = engine.lock();
                        if let Ok(engine) = guard {
                            let status = engine.compute_tray_status(now);
                            if let Some(tray) = app.tray_by_id("rewind-tray") {
                                let _ = tray.set_tooltip(Some(status.tooltip_line));
                            }
                            // Keep the `remaining` argument from going
                            // unused — it's surfaced to the frontend
                            // via the Tick CoreEvent above; the adapter
                            // path uses the recomputed status so the
                            // copy matches what other adapter callers
                            // see.
                            let _ = (remaining, monotonic);
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

/// Stop the tick loop. Currently a no-op — the task exits when the
/// process exits. Kept for future graceful-shutdown support (M6).
pub async fn stop() {
    // No handle kept in this design; the loop terminates with the
    // tokio runtime.
}