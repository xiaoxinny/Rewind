//! In-app uninstall orchestration.
//!
//! Removes user data dirs, autostart entries, and (where applicable) the
//! binary itself. The frontend invokes `uninstall_and_exit` IPC after the
//! user confirms; this module does the actual work synchronously, then asks
//! the OS to terminate the process.
//!
//! Platform behaviour:
//!   - **macOS**: `rm -rf /Applications/Rewind.app` + user data dirs + LaunchAgent plist.
//!   - **Linux (dpkg)**: `pkexec dpkg --purge rewind` (or sudo fallback) + XDG dirs.
//!   - **Linux (AppImage/tarball)**: `rm -rf` the binary's parent directory + XDG dirs.
//!   - **Windows**: spawn `Uninstall.exe /S` (NSIS silent) + `%APPDATA%` cleanup.
//!
//! All removals are best-effort: missing paths are silently ignored, and
//! errors are logged to stderr without panicking. The process always exits
//! with code 0 after cleanup completes.

use std::path::PathBuf;
use std::process::Command;

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

/// Bundle identifier used as the directory name under platform data roots.
const BUNDLE_ID: &str = "com.rewind.app";

/// Debian package name (must match tauri.conf.json's deb metadata).
const DEB_PACKAGE_NAME: &str = "rewind";

/// Windows NSIS uninstaller path (Tauri 2 default for perUser/perMachine installs).
#[cfg(target_os = "windows")]
const NSIS_UNINSTALL_EXE: &str = r"C:\Program Files\Rewind\Uninstall.exe";

/// Best-effort recursive directory removal. Logs to stderr on error but
/// never returns Err — a missing path is treated as success (already cleaned).
fn try_remove_dir(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(path) {
        eprintln!("Rewind uninstall: could not remove {}: {e}", path.display());
    }
}

/// Best-effort single file removal. Same error policy as `try_remove_dir`.
fn try_remove_file(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    if let Err(e) = std::fs::remove_file(path) {
        eprintln!("Rewind uninstall: could not remove {}: {e}", path.display());
    }
}

/// Collect every user-data directory that should be removed, using Tauri 2
/// path APIs where available and falling back to `home_dir()` for
/// non-standard locations (autostart files, XDG cache).
fn collect_user_data_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    // Standard Tauri dirs — these resolve per-platform.
    if let Ok(d) = app.path().app_data_dir() {
        dirs.push(d);
    }
    if let Ok(d) = app.path().app_config_dir() {
        dirs.push(d);
    }
    if let Ok(d) = app.path().app_log_dir() {
        dirs.push(d);
    }
    if let Ok(d) = app.path().app_cache_dir() {
        dirs.push(d);
    }

    // Platform-specific extras not covered by Tauri's standard dirs.
    if let Ok(home) = app.path().home_dir() {
        #[cfg(target_os = "macos")]
        {
            dirs.push(home.join("Library/Application Support").join(BUNDLE_ID));
            dirs.push(home.join("Library/Preferences").join(format!("{BUNDLE_ID}.plist")));
            dirs.push(home.join("Library/Caches").join(BUNDLE_ID));
        }
        #[cfg(target_os = "linux")]
        {
            // XDG dirs that Tauri may not expose via path API on all distros.
            dirs.push(home.join(".local/share").join(BUNDLE_ID));
            dirs.push(home.join(".config").join(BUNDLE_ID));
            dirs.push(home.join(".cache").join(BUNDLE_ID));
            // Autostart .desktop file (registered by tauri-plugin-autostart).
            dirs.push(home.join(".config/autostart/rewind.desktop"));
        }
        #[cfg(target_os = "windows")]
        {
            // %APPDATA%\com.rewind.app — not removed by NSIS uninstaller.
            dirs.push(home.join("AppData/Roaming").join(BUNDLE_ID));
            dirs.push(home.join("AppData/Local").join(BUNDLE_ID));
        }
    }

    dirs
}

/// Remove all user-data directories and files.
fn purge_user_data(app: &AppHandle) {
    let dirs = collect_user_data_dirs(app);
    for d in &dirs {
        if d.is_dir() {
            try_remove_dir(d);
        } else if d.is_file() {
            try_remove_file(d);
        }
    }
}

/// Disable the autostart entry via the plugin, then also remove the
/// autostart file directly (belt-and-suspenders for broken plugin state).
fn remove_autostart(app: &AppHandle) {
    // Plugin-level disable — works on all platforms.
    let _ = app.autolaunch().disable();

    // File-level cleanup on Linux (the plugin writes a .desktop file).
    #[cfg(target_os = "linux")]
    if let Ok(home) = app.path().home_dir() {
        let autostart = home.join(".config/autostart/rewind.desktop");
        try_remove_file(&autostart);
    }

    // macOS LaunchAgent plist (if the plugin created one).
    #[cfg(target_os = "macos")]
    if let Ok(home) = app.path().home_dir() {
        let plist = home.join("Library/LaunchAgents").join(format!("{BUNDLE_ID}.plist"));
        try_remove_file(&plist);
    }
}

/// Platform-specific binary removal.
fn uninstall_binary(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        // Remove the .app bundle from /Applications.
        let app_bundle = PathBuf::from("/Applications/Rewind.app");
        try_remove_dir(&app_bundle);
        // Also try the user's local Applications folder.
        if let Ok(home) = app.path().home_dir() {
            let local_apps = home.join("Applications/Rewind.app");
            try_remove_dir(&local_apps);
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Detect dpkg installation vs AppImage/tarball.
        let dpkg_check = Command::new("dpkg")
            .args(["-s", DEB_PACKAGE_NAME])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        let is_dpkg_installed = dpkg_check.map(|s| s.success()).unwrap_or(false);

        if is_dpkg_installed {
            // Try pkexec first (graphical prompt), then sudo -n (passwordless).
            let pkexec_result = Command::new("pkexec")
                .args(["dpkg", "--purge", DEB_PACKAGE_NAME])
                .status();

            if pkexec_result.is_err() {
                // pkexec not available — try sudo -n.
                let _ = Command::new("sudo")
                    .args(["-n", "dpkg", "--purge", DEB_PACKAGE_NAME])
                    .status();
            }
            // If both fail (user cancelled, no sudo), fall through to
            // the rmdir below — the binary may still be running from
            // the installed path.
        }

        // AppImage / tarball: remove the binary's parent directory if it
        // looks like a self-contained install (not /usr/bin or system dir).
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let p = parent.to_path_buf();
                // Only remove if it's under the user's home or /opt/rewind
                // or a directory that contains "rewind" in its name.
                let is_user_path = p.starts_with("/home/")
                    || p.starts_with("/opt/rewind")
                    || p.to_string_lossy().to_lowercase().contains("rewind");
                if is_user_path && !p.starts_with("/usr") {
                    try_remove_dir(&p);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Spawn the NSIS uninstaller in silent mode.
        let uninstaller = PathBuf::from(NSIS_UNINSTALL_EXE);
        if uninstaller.exists() {
            let _ = Command::new(&uninstaller)
                .arg("/S")
                .status();
        }
        // The NSIS uninstaller removes Program Files\Rewind. The
        // %APPDATA% cleanup is handled by purge_user_data above.
    }
}

/// IPC entry point. Called from the frontend after the user confirms the
/// uninstall modal. Performs all cleanup synchronously, then exits the
/// process. Never returns.
#[tauri::command]
pub fn uninstall_and_exit(app: AppHandle) -> Result<(), String> {
    eprintln!("Rewind: uninstall initiated by user");

    // 1. Disable autostart first so the OS doesn't relaunch us mid-cleanup.
    let _ = app.autolaunch().disable();

    // 2. Remove user data (SQLite, store JSON, window-state JSON, logs, cache).
    purge_user_data(&app);

    // 3. Remove autostart files (post-uninstall dead entries).
    remove_autostart(&app);

    // 4. Trigger the platform-specific binary uninstaller.
    uninstall_binary(&app);

    // 5. Exit synchronously — std::process::exit is immediate, unlike
    //    app.exit(0) which defers to the next event loop iteration.
    eprintln!("Rewind: uninstall complete, exiting");
    std::process::exit(0);
}
