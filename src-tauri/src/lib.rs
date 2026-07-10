//! Rewind shell — composition root.
//!
//! Ties the engine, adapters, and storage together behind Tauri v2.
//! M6 wires in the SQLite-backed history repo (`StorageApp`) +
//! the `tauri-plugin-store` based `ConfigStore` for `AppConfig`.
//!
//! Sub-modules:
//!   * `adapters.rs`        — bundles the OS/UI adapters
//!   * `config_store.rs`    — `tauri-plugin-store` ↔ `AppConfig`
//!   * `ipc.rs`             — `#[tauri::command]`s → `CoreCommand`s
//!   * `overlay_adapter.rs` — Tauri-backed `OverlayController`
//!   * `runtime.rs`         — 1 Hz tick loop spawned from `setup`
//!   * `storage_app.rs`     — `SqliteHistoryRepo` convenience handle
//!   * `storage_helpers.rs` — small helpers shared with the storage crate
//!   * `wiring.rs`          — DI: build Engine + adapters + storage

use std::sync::Arc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

mod adapters;
mod config_store;
mod idle_handle;
mod ipc;
mod overlay_adapter;
mod runtime;
mod storage_app;
mod storage_helpers;
mod wiring;

pub use adapters::Adapters;
pub use config_store::{load_or_default as load_config_or_default, save as save_config};
pub use idle_handle::IdleHandle;
pub use ipc::{
    clear_history, export_data, get_break_kind_label, get_engine_snapshot, get_pause_reason_label,
    get_recent_hydration, get_recent_sessions, get_today, log_water, pause_toggle, postpone_break,
    set_autostart, set_strictness, skip_break, start_focus, update_config, EngineSnapshot,
    ExportPayload,
};
pub use storage_app::{StorageApp, StorageAppError};
pub use wiring::Wiring;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Tauri v2 plugins — see implementation plan §4.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // On a second launch, focus the existing main window.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.show();
                let _ = window.unminimize();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            // Load AppConfig from the JSON store (or seed defaults).
            let initial_config = config_store::load_or_default(&app.handle());

            // Resolve app_data_dir before wiring builds the storage
            // pool (storage needs the directory to exist).
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("com.rewind.app"));

            // Build the wiring (engine, adapters, storage). This
            // must be `block_on`-safe: the setup callback runs
            // synchronously inside Tauri's startup, before the
            // tokio runtime is opened. We create a single-threaded
            // runtime inline so the storage open + migrate can
            // complete before we hand off to Tauri's main loop.
            let wiring = {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("setup runtime");
                rt.block_on(Wiring::build(&app.handle(), app_data_dir, initial_config))
            };

            // Build the tray.
            build_tray(app.handle())?;

            // Spawn the 1 Hz tick loop on Tauri's tokio runtime.
            let engine = wiring.engine.clone();
            let adapters = Adapters::build(
                app.handle(),
                wiring.idle.clone(),
                wiring.overlay.clone(),
                wiring.storage.clone(),
            );
            runtime::start(
                app.handle().clone(),
                engine,
                adapters,
                wiring.clock,
                wiring.storage.clone(),
            );

            // Register the kill-switch global shortcut. It hides
            // the overlay window; safe to bind even before the
            // overlay is visible.
            if let Err(e) = register_kill_switch(app.handle()) {
                eprintln!("Rewind: failed to register kill switch: {e}");
            }

            // Make the engine + storage + idle-handle reachable from
            // IPC commands.
            app.manage(wiring.engine);
            app.manage(wiring.storage);
            app.manage(IdleHandle::new(wiring.idle));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_focus,
            pause_toggle,
            skip_break,
            postpone_break,
            log_water,
            update_config,
            set_strictness,
            set_autostart,
            get_engine_snapshot,
            get_today,
            get_recent_sessions,
            get_recent_hydration,
            export_data,
            clear_history,
            get_break_kind_label,
            get_pause_reason_label,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the system tray icon. Stays alive for the lifetime of the
/// app; `runtime.rs` updates its tooltip on every tick.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open Rewind", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause", "Pause / Resume", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Rewind", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &pause_item, &quit_item])?;

    let icon = tauri::image::Image::from_path("icons/32x32.png")?;

    TrayIconBuilder::with_id("rewind-tray")
        .icon(icon)
        .tooltip("Rewind")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }
            "pause" => {
                use tauri::Emitter;
                let _ = app.emit("tray-pause-toggle", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Register the kill-switch global shortcut (`Ctrl+Alt+Shift+Esc`).
fn register_kill_switch(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let kill = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
        Code::Escape,
    );

    app.global_shortcut().on_shortcut(kill, move |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window("overlay") {
                let _ = window.hide();
                let _ = window.set_fullscreen(false);
            }
        }
    })?;

    app.global_shortcut().register(kill)?;
    Ok(())
}

/// Public re-export of the wiring pipeline. Callers (mostly tests)
/// can ignore this — only the main entry point uses it.
#[allow(dead_code)]
pub(crate) fn handle() {}
