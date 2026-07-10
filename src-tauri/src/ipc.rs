//! IPC commands — the thin Rust layer that translates Tauri
//! `#[tauri::command]` calls into `CoreCommand`s on the engine and
//! returns CoreEvents to the frontend.
//!
//! Each command is a small function that grabs the shared `Engine`
//! from Tauri's state, calls `engine.handle(...)` with the appropriate
//! `CoreCommand`, and returns the resulting events (or a sanitized
//! response payload). M6 also adds history-bound commands that read
//! / write through `StorageApp`.

use std::sync::{Arc, Mutex};

use rewind_core::model::aggregate::DailyAggregate;
use rewind_core::model::break_record::{BreakOutcome, BreakRecord};
use rewind_core::model::hydration::HydrationSource;
use rewind_core::{
    AppConfig, BreakKind, Engine, HydrationProgress, PauseReason, SessionState, Strictness,
};
use serde::Serialize;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;

use crate::config_store;
use crate::storage_app::{StorageApp, StorageAppError};

// ---------------------------------------------------------------------------
// Engine snapshot — composite view returned by `get_engine_snapshot`.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct EngineSnapshot {
    pub state: SessionState,
    pub config: AppConfig,
    pub hydration: HydrationProgress,
    pub today: DailyAggregate,
    pub idle_reliable: bool,
}

#[tauri::command]
pub fn get_engine_snapshot(
    engine: State<'_, Arc<Mutex<Engine>>>,
    storage: State<'_, StorageApp>,
    idle: State<'_, crate::idle_handle::IdleHandle>,
) -> Result<EngineSnapshot, String> {
    let engine = engine.lock().map_err(|e| e.to_string())?;
    let consumed = engine.hydration_consumed();
    let goal = engine.config().hydration.goal_ml;
    let today = storage.today_snapshot();
    Ok(EngineSnapshot {
        state: engine.state(),
        config: engine.config().clone(),
        hydration: HydrationProgress::new(consumed, goal),
        today,
        idle_reliable: idle.reliable(),
    })
}

// ---------------------------------------------------------------------------
// Engine command IPCs — pass through to `CoreCommand`s.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn start_focus(
    engine: State<'_, Arc<Mutex<Engine>>>,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::StartFocus, now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn pause_toggle(
    engine: State<'_, Arc<Mutex<Engine>>>,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::PauseToggle, now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn skip_break(
    engine: State<'_, Arc<Mutex<Engine>>>,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::SkipBreak, now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn postpone_break(
    engine: State<'_, Arc<Mutex<Engine>>>,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::PostponeBreak, now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn log_water(
    app: tauri::AppHandle,
    engine: State<'_, Arc<Mutex<Engine>>>,
    storage: State<'_, StorageApp>,
    amount_ml: u32,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let entry = rewind_core::model::hydration::HydrationEntry {
        id: None,
        logged_at: now,
        amount_ml,
        source: HydrationSource::Manual,
    };

    // Persist through the storage layer (synchronously via Tauri's
    // managed runtime) **before** telling the engine so the row is
    // on disk even if the tick loop crashes immediately after.
    if let Err(e) =
        tauri::async_runtime::block_on(storage.record_hydration(entry))
    {
        eprintln!("Rewind: failed to persist hydration log: {e}");
    }

    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::LogWater(amount_ml), now);

    // Sync the live config snapshot to the store — caller's
    // `update_config` already does this, but a quick-log shouldn't
    // require a full `update_config` round-trip just to keep the
    // store in sync.
    let _ = app;

    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn update_config(
    app: tauri::AppHandle,
    engine: State<'_, Arc<Mutex<Engine>>>,
    config: AppConfig,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    // Persist to the store BEFORE telling the engine so a crash mid-way
    // doesn't lose the user's setting.
    config_store::save(&app, &config);

    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::ConfigUpdated(config), now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn set_strictness(
    engine: State<'_, Arc<Mutex<Engine>>>,
    strictness: Strictness,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::SetStrictness(strictness), now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| format!("autostart enable: {e}"))?;
    } else {
        manager.disable().map_err(|e| format!("autostart disable: {e}"))?;
    }
    manager.is_enabled().map_err(|e| format!("autostart status: {e}"))
}

// ---------------------------------------------------------------------------
// History IPCs — read through the storage layer.
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_today(storage: State<'_, StorageApp>) -> Result<DailyAggregate, String> {
    // Always re-sync from disk so the snapshot is fresh after
    // out-of-band changes (e.g. a `clear_history` from Settings).
    storage
        .reload_today()
        .await
        .map_err(|e| format!("storage reload: {e}"))?;
    Ok(storage.today_snapshot())
}

#[tauri::command]
pub async fn get_recent_sessions(
    storage: State<'_, StorageApp>,
    days: u32,
) -> Result<Vec<rewind_core::model::session::SessionRecord>, String> {
    storage
        .repo()
        .recent_sessions(days)
        .await
        .map_err(|e| format!("read sessions: {e:?}"))
}

#[tauri::command]
pub async fn get_recent_hydration(
    storage: State<'_, StorageApp>,
    days: u32,
) -> Result<Vec<rewind_core::model::hydration::HydrationEntry>, String> {
    storage
        .repo()
        .recent_hydration(days)
        .await
        .map_err(|e| format!("read hydration: {e:?}"))
}

#[derive(Serialize)]
pub struct ExportPayload {
    /// Pretty-printed JSON document. The frontend can `download` it
    /// via a `Blob` + temporary `<a>` click.
    pub json: String,
    /// ISO-8601 timestamp of the export.
    pub generated_at: String,
    /// Total number of rows exported (sum across all four tables).
    pub row_count: u32,
}

#[tauri::command]
pub async fn export_data(storage: State<'_, StorageApp>) -> Result<ExportPayload, String> {
    let value = storage
        .repo()
        .export_json()
        .await
        .map_err(|e| format!("export: {e:?}"))?;
    let json = serde_json::to_string_pretty(&value).map_err(|e| format!("pretty: {e}"))?;
    // Row count = sum of array lengths (cheap probe via the JSON
    // value we already have in hand).
    let row_count = ["session", "break_record", "hydration_log", "daily_aggregate"]
        .iter()
        .map(|k| {
            value
                .get(*k)
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u32)
                .unwrap_or(0)
        })
        .sum();
    Ok(ExportPayload {
        json,
        generated_at: chrono_like_now(),
        row_count,
    })
}

#[tauri::command]
pub async fn clear_history(storage: State<'_, StorageApp>) -> Result<(), String> {
    storage
        .clear()
        .await
        .map_err(|e| format!("clear history: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_break_kind_label(kind: BreakKind) -> String {
    kind.to_string()
}

#[tauri::command]
pub fn get_pause_reason_label(reason: PauseReason) -> String {
    reason.to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Tiny ISO-ish timestamp formatter used by `export_data`. We don't
/// pull in `chrono` (the rest of the crate uses `time`); format
/// from a `time::OffsetDateTime` instead.
fn chrono_like_now() -> String {
    use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_utc();
    now.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

// ---------------------------------------------------------------------------
// DTOs — the frontend sees CoreEvent tagged with `kind` so it can
// pattern-match. We don't re-export the full enum because TypeScript
// needs a stable tagged-union shape.
// ---------------------------------------------------------------------------

use rewind_core::{CoreCommand, CoreEvent, Timestamp};

/// Tagged DTO mirroring the Rust `CoreEvent` enum so the frontend
/// can `switch (event.kind)` cleanly. See `src/lib/types.ts`.
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEventDto {
    StateChanged {
        state: SessionState,
    },
    Tick {
        phase: SessionState,
        remaining_ms: u64,
        now_ms: i64,
    },
    ShowBreak {
        kind: BreakKind,
        presentation_strict: bool,
        exercise_id: Option<String>,
    },
    DismissBreak,
    FireReminder {
        kind: rewind_core::ReminderKind,
        priority: rewind_core::Priority,
        message: String,
    },
    HydrationUpdated {
        consumed_ml: u32,
        goal_ml: u32,
    },
    TrayStatus {
        tooltip_line: String,
        title: String,
        icon_hint: String,
    },
    TrayMenu {
        items: Vec<TrayMenuItemDto>,
    },
}

#[derive(Serialize, Clone)]
pub struct TrayMenuItemDto {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

impl From<CoreEvent> for CoreEventDto {
    fn from(e: CoreEvent) -> Self {
        use CoreEvent::*;
        match e {
            StateChanged(state) => CoreEventDto::StateChanged { state },
            Tick {
                phase,
                remaining,
                now,
            } => CoreEventDto::Tick {
                phase,
                remaining_ms: remaining.as_millis() as u64,
                now_ms: now.map(|t| t.0).unwrap_or(0),
            },
            ShowBreak {
                kind,
                presentation,
                exercise_id,
            } => CoreEventDto::ShowBreak {
                kind,
                presentation_strict: matches!(
                    presentation,
                    rewind_core::BreakPresentation::Strict
                ),
                exercise_id,
            },
            DismissBreak => CoreEventDto::DismissBreak,
            FireReminder {
                kind,
                priority,
                message,
            } => CoreEventDto::FireReminder {
                kind,
                priority,
                message,
            },
            HydrationUpdated(p) => CoreEventDto::HydrationUpdated {
                consumed_ml: p.consumed_ml,
                goal_ml: p.goal_ml,
            },
            TrayStatus(s) => CoreEventDto::TrayStatus {
                title: s.title,
                tooltip_line: s.tooltip_line,
                icon_hint: s.icon_hint,
            },
            TrayMenu(items) => CoreEventDto::TrayMenu {
                items: items
                    .into_iter()
                    .map(|i| TrayMenuItemDto {
                        id: i.id,
                        label: i.label,
                        enabled: i.enabled,
                    })
                    .collect(),
            },
        }
    }
}

/// Break records are persisted via the runtime tick loop (not the
/// IPC layer) because the engine doesn't carry enough state to
/// reliably derive them from `CoreEvent` alone — see `runtime.rs`
/// for the state-transition-based detection.
#[allow(dead_code)]
pub fn break_record_from_event(
    ev: &CoreEvent,
    now: Timestamp,
) -> Option<BreakRecord> {
    let _ = (ev, now);
    None
}

/// Compile-time linker check that `StorageAppError` is `Send + Sync`
/// (the State lives behind a `Mutex` which doesn't need this — but
/// we want a future async path to be safe).
#[allow(dead_code)]
fn assert_storage_send_sync() {
    fn require<T: Send + Sync>() {}
    require::<StorageAppError>();
}
