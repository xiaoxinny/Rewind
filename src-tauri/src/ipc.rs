//! IPC commands — the thin Rust layer that translates Tauri
//! `#[tauri::command]` calls into `CoreCommand`s on the engine and
//! returns CoreEvents to the frontend.
//!
//! Each command is a small function that grabs the shared `Engine`
//! from Tauri's state, calls `engine.handle(...)` with the appropriate
//! `CoreCommand`, and returns the resulting events (or a sanitized
//! response payload).

use std::sync::{Arc, Mutex};

use rewind_core::{
    AppConfig, BreakKind, Engine, HydrationProgress, PauseReason, SessionState,
    Strictness,
};
use serde::Serialize;
use tauri::State;

/// Commands that return state-shapes get a serializable wrapper so
/// the JSON shape matches `src/lib/types.ts`. The raw engine events
/// are returned only via the event channel (`emit("core-event", …)`)
/// — these `*_state` / `*_config` commands are for explicit queries.
#[derive(Serialize)]
pub struct EngineSnapshot {
    pub state: SessionState,
    pub config: AppConfig,
    pub hydration: HydrationProgress,
}

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
    engine: State<'_, Arc<Mutex<Engine>>>,
    amount_ml: u32,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let events = engine.handle(CoreCommand::LogWater(amount_ml), now);
    Ok(events.into_iter().map(CoreEventDto::from).collect())
}

#[tauri::command]
pub fn update_config(
    engine: State<'_, Arc<Mutex<Engine>>>,
    config: AppConfig,
    now: Timestamp,
) -> Result<Vec<CoreEventDto>, String> {
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
pub fn get_engine_snapshot(
    engine: State<'_, Arc<Mutex<Engine>>>,
) -> Result<EngineSnapshot, String> {
    let engine = engine.lock().map_err(|e| e.to_string())?;
    Ok(EngineSnapshot {
        state: engine.state(),
        config: engine.config().clone(),
        hydration: HydrationProgress::new(0, engine.config().hydration.goal_ml),
    })
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
// DTOs — the frontend sees CoreEvent tagged with `kind` so it can
// pattern-match. We don't re-export the full enum because TypeScript
// needs a stable tagged-union shape.
// ---------------------------------------------------------------------------

use rewind_core::{CoreCommand, CoreEvent, Timestamp};

/// Tagged DTO mirroring the Rust `CoreEvent` enum so the frontend
/// can `switch (event.kind)` cleanly. See `src/lib/types.ts`.
///
/// The serde tag is `type` (not `kind`) so the inner field of
/// variants like `ShowBreak { kind, ... }` doesn't clash with the
/// tag. The frontend reads `event.type` for the variant and
/// `event.kind` for the inner `BreakKind`.
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