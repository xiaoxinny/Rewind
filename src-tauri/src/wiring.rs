//! `wiring.rs` — the shell's composition root (per implementation
//! plan §9). The `setup` closure calls `Wiring::build(...)` once at
//! boot, which:
//!
//! 1. Picks the platform-appropriate `IdleSource` (X11, Wayland,
//!    fallback to degraded — same picker the `rewind-adapters` crate
//!    exposes).
//! 2. Builds the bundled `OverlayController` (TauriOverlay) and the
//!    storage handle (SQLite via `rewind-storage`).
//! 3. Loads `AppConfig` from the JSON store (or seeds `default()`
//!    the first run), constructs the `Engine`, and hands every
//!    component back to the caller.
//!
//! On a real platform mismatch (e.g. X11-only `user_idle` binary on
//! a GNOME Wayland session), the picker returns a `DegradedIdleSource`
//! and `IdleSource::reliability()` reads `Unreliable` — the engine
//! already downgrades to timer-only mode on its own (§7f).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rewind_adapters::idle::pick as pick_idle_source;
use rewind_core::ports::IdleSource;
use rewind_core::{AppConfig, Clock, Engine, RealClock};
use tauri::{AppHandle, Manager};

use crate::overlay_adapter::TauriOverlay;
use crate::storage_app::StorageApp;

/// Result of building the composition root.
pub struct Wiring {
    pub clock: Arc<dyn Clock>,
    pub engine: Arc<Mutex<Engine>>,
    pub overlay: TauriOverlay,
    pub storage: StorageApp,
    pub idle: Arc<dyn IdleSource>,
}

impl Wiring {
    /// Build the composition root. `app_data_dir` is the standard
    /// Tauri `app_data_dir()` — the DB lives one level below it as
    /// `rewind.db`.
    pub async fn build(
        app: &AppHandle,
        app_data_dir: PathBuf,
        initial_config: AppConfig,
    ) -> Self {
        let clock: Arc<dyn Clock> = Arc::new(RealClock);
        let now = clock.now();
        let engine = Arc::new(Mutex::new(Engine::new(now, initial_config)));

        let overlay = TauriOverlay::new(app.clone());
        let idle = Arc::from(pick_idle_source());

        // Storage: open at `app_data_dir/rewind.db`.
        let db_path = app_data_dir.join("rewind.db");
        let storage = match StorageApp::open(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Rewind: failed to open history DB at {db_path:?}; starting with in-memory only. {e}"
                );
                // Fall back to an in-memory pool — better than crashing.
                let tmp = std::env::temp_dir().join(format!("rewind-{}.db", std::process::id()));
                StorageApp::open(tmp).await.expect("fallback storage")
            }
        };

        Self {
            clock,
            engine,
            overlay,
            storage,
            idle,
        }
    }
}
