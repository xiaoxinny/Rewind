//! Rewind shell — composition root.
//!
//! Ties the engine, adapters, and storage together behind Tauri v2.
//! M0 is intentionally minimal: registers all required plugins, brings
//! up an empty window, and shows a tray icon with a Quit item. No IPC
//! commands, no real engine, no real idle yet — those land in their
//! respective milestones.

use rewind_core::{Clock, RealClock, Timestamp};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Prove the workspace linkage works: read the wall clock once at
    // startup. This intentionally exercises the `rewind-core` dep from
    // inside the Tauri process. The binding is replaced in M1 with a
    // real 1 Hz tick loop.
    let clock = RealClock;
    let _start: Timestamp = clock.now();

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
            build_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the system tray icon: a single Quit menu item for M0. The
/// real tray (countdown tooltip, status icons, dynamic menu) lands in
/// M1 per the milestone plan.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    // Load the tray icon from the bundled icon asset. Tauri's tray
    // icon must be small enough for the platform status bar; on
    // macOS this is the template image and must be a 1-bit alpha
    // (the existing icon is RGBA which is fine on Linux/Windows for
    // M0).
    let icon = tauri::image::Image::from_path("icons/32x32.png")?;

    TrayIconBuilder::with_id("rewind-tray")
        .icon(icon)
        .tooltip("Rewind")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
