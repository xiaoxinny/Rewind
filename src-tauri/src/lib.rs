//! Rewind shell — composition root.
//!
//! Ties the engine, adapters, and storage together behind Tauri v2.
//! M1: registers all plugins, brings up the tray icon, runs the 1 Hz
//! tick loop, dispatches CoreEvents to the frontend, exposes IPC
//! commands for the Svelte UI.
//!
//! Sub-modules:
//!   * `adapters.rs`        — bundles the OS/UI adapters
//!   * `ipc.rs`             — `#[tauri::command]`s → `CoreCommand`s
//!   * `overlay_adapter.rs` — Tauri-backed `OverlayController`
//!   * `runtime.rs`         — 1 Hz tick loop spawned from `setup`

use std::sync::{Arc, Mutex};

use rewind_core::{AppConfig, Clock, Engine, RealClock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

mod adapters;
mod ipc;
mod overlay_adapter;
mod runtime;

pub use adapters::Adapters;
pub use ipc::{
    get_break_kind_label, get_engine_snapshot, get_pause_reason_label, log_water,
    pause_toggle, postpone_break, set_strictness, skip_break, start_focus, update_config,
    EngineSnapshot,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let clock: Arc<dyn Clock> = Arc::new(RealClock);

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
        .setup({
            let clock = clock.clone();
            move |app| {
                // Build the engine with a default config. (M6 will
                // load the saved config from the store; for M1 we
                // ship defaults and let ConfigUpdated overwrite.)
                let initial_config = AppConfig::default();
                let now = clock.now();
                let engine = Arc::new(Mutex::new(Engine::new(now, initial_config)));

                // Build adapters from the app handle.
                let adapter_bundle = Adapters::build(app.handle());

                // Build the tray. The tray stays alive for the
                // lifetime of the app; runtime.rs updates its tooltip.
                build_tray(app.handle())?;

                // Spawn the 1 Hz tick loop.
                runtime::start(app.handle().clone(), engine.clone(), adapter_bundle, clock);

                // Register the kill-switch global shortcut. It hides
                // the overlay window; safe to bind even before the
                // overlay is visible (clicking it with nothing
                // showing is a no-op).
                if let Err(e) = register_kill_switch(app.handle()) {
                    eprintln!("Rewind: failed to register kill switch: {e}");
                }

                // Make the engine reachable from IPC commands.
                app.manage(engine);

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_focus,
            pause_toggle,
            skip_break,
            postpone_break,
            log_water,
            update_config,
            set_strictness,
            get_engine_snapshot,
            get_break_kind_label,
            get_pause_reason_label,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the system tray icon. The tray stays alive for the
/// lifetime of the app; runtime.rs updates its tooltip on every
/// tick so the countdown stays current. The menu is intentionally
/// small in M1 — a richer dynamic menu lands later (M3).
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
                // Emit a CoreCommand to the engine so the state machine
                // pauses. The tick loop will pick it up on the next
                // pass and emit the resulting state change.
                use tauri::Emitter;
                let _ = app.emit("tray-pause-toggle", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Register the kill-switch global shortcut (`Ctrl+Alt+Shift+Esc` on
/// every platform). When the user hits it, hide the overlay window
/// if it's currently visible. This is the only way to dismiss a
/// `Strict` overlay — without it, a bug in the engine could leave
/// the user locked out.
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