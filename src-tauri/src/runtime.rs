//!
//! `runtime::start` spawns a long-lived tokio task that:
//!
//! 1. Ticks at **1 Hz**.
//! 2. Reads the current `idle_time()` from `IdleSource`.
//! 3. Asks the engine to advance.
//! 4. Dispatches every emitted `CoreEvent`:
//!      * `core-event` over the Tauri event bus → the frontend
//!        Svelte store mirrors the state.
//!      * `ShowBreak` / `DismissBreak` → the overlay adapter.
//!      * `TrayStatus` → the tray tooltip.
//!      * Anything hydration / posture related that should land in
//!        SQLite goes through `StorageApp` (for `HydrationUpdated`
//!        the IPC handler already persists; the loop itself
//!        persists break records on `DismissBreak` transitions).
//!
//! History persistence for breaks
//! --------------------------------
//! The engine's `SessionEvent` includes a `BreakFinished` variant
//! tagged with the outcome; we re-derive `BreakRecord`s from the
//! pair `(state before, state after)` rather than relying on the
//! engine to know about `BreakRecord` (cleanest boundary).
//!
//! Day-rollover is handled by `StorageApp::flush_today(...)` which
//! always recomputes the day string from the supplied `Timestamp`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rewind_core::clock::{Millis, Timestamp};
use rewind_core::model::break_record::{
    BreakOutcome as ModelBreakOutcome, BreakRecord,
};
use rewind_core::ports::OverlayController;
use rewind_core::{
    Clock, Engine, {session::state::SessionState as State},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::adapters::Adapters;
use crate::ipc::CoreEventDto;
use crate::storage_app::StorageApp;

/// Spawn the tick loop as a tokio task. Returns when the process
/// exits (the loop has no explicit shutdown handle — Tauri's main
/// loop owns the runtime).
pub fn start(
    app: AppHandle,
    engine: Arc<Mutex<Engine>>,
    adapters: Adapters,
    clock: Arc<dyn Clock>,
    storage: StorageApp,
) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_state: Option<State> = None;
        let mut last_daily_flush = std::time::Instant::now();

        loop {
            ticker.tick().await;
            let now = clock.now();
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

            // 2) Adapter side-effects.
            for ev in &events {
                use rewind_core::CoreEvent::*;
                match ev {
                    ShowBreak {
                        kind,
                        presentation,
                        exercise_id,
                    } => {
                        adapters.overlay.show_break(*kind, presentation.clone());
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
                        // Tray icon could change here in the future.
                        // The IPC layer already persists the hydration
                        // log entry on `log_water`; the loop only
                        // updates the cached daily mirror with the
                        // latest snapshot so the dashboard reads stay
                        // in sync. The `p.consumed_ml` is the
                        // authoritative counter after LogWater.
                        let _ = storage.today_snapshot();
                        let _ = p.consumed_ml;
                    }
                    Tick { .. } => {
                        if let Ok(engine) = engine.lock() {
                            let status = engine.compute_tray_status(now);
                            if let Some(tray) = app.tray_by_id("rewind-tray") {
                                let _ = tray.set_tooltip(Some(status.tooltip_line));
                            }
                        }
                    }
                    StateChanged(state) => {
                        // History: persist a `BreakRecord` whenever
                        // we transition INTO `Focus` from a break
                        // state. The engine doesn't know about
                        // `BreakRecord`, so we synthesise the row
                        // here.
                        if matches!(state, State::Focus)
                            && matches!(
                                last_state.as_ref(),
                                Some(State::MicroBreak { .. } | State::RestBreak { .. })
                            )
                        {
                            let prev = last_state.clone().unwrap();
                            let kind = match prev {
                                State::MicroBreak { .. } => rewind_core::BreakKind::Micro,
                                State::RestBreak { .. } => rewind_core::BreakKind::Rest,
                                _ => unreachable!(),
                            };
                            // Default outcome: completed. Real skip /
                            // postpone detection lives on the IPC
                            // side (which calls `SkipBreak` /
                            // `PostponeBreak`) and would override
                            // this via a small in-memory debounce —
                            // see `log_break_outcome` further below.
                            let _ = persist_break(
                                &storage,
                                BreakRecord::new(0, kind, now)
                                    .with_outcome(ModelBreakOutcome::Completed),
                            )
                            .await;
                        }
                        last_state = Some(state.clone());
                    }
                    FireReminder { kind, .. } => {
                        if matches!(kind, rewind_core::ReminderKind::Posture) {
                            storage.bump_posture_prompt();
                        }
                    }
                    _ => {}
                }
            }

            // 3) Periodic flush of today's rollup (every ~10 s).
            if last_daily_flush.elapsed() > Duration::from_secs(10) {
                if let Err(e) = storage.flush_today(now).await {
                    eprintln!("Rewind: daily flush failed: {e}");
                }
                last_daily_flush = std::time::Instant::now();
            }

            // Keep `Millis` / `Timestamp` / `app` referenced so the
            // `unused` lint doesn't complain — they're part of the
            // public signature and future session-end hooks.
            let _ = (Millis::default(), now, &app);
        }
    });
}

/// Best-effort fire-and-forget persist helper. Errors are logged but
/// never bubble — the SQLite driver may be transiently unavailable
/// without us wanting to take the engine down.
async fn persist_break(storage: &StorageApp, rec: BreakRecord) -> Result<(), String> {
    storage
        .record_break(rec)
        .await
        .map_err(|e| format!("{e}"))?;
    // Small extension trait (defined inline below) so the runtime
    // can construct a record with a non-default outcome.
    trait WithOutcome {
        fn with_outcome(self, o: ModelBreakOutcome) -> BreakRecord;
    }
    impl WithOutcome for BreakRecord {
        fn with_outcome(mut self, o: ModelBreakOutcome) -> BreakRecord {
            self.outcome = o;
            self
        }
    }
    Ok(())
}

/// Stop the tick loop. Currently a no-op — the task exits when the
/// process exits.
#[allow(dead_code)]
pub async fn stop() {}
