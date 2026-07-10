//! `Adapters` — bundles the concrete adapter instances that the
//! tick loop dispatches to. Lives in the shell because most of the
//! real implementations (overlay, tray, store, autostart) need
//! Tauri types. The platform-only `IdleSource` lives in
//! `rewind-adapters` so the engine ↔ OS edge is shared with
//! non-Tauri use cases (e.g. a future CLI test harness).

use rewind_core::ports::IdleSource;
use std::sync::Arc;

use crate::overlay_adapter::TauriOverlay;
use crate::storage_app::StorageApp;

/// Bundle the tick loop needs. Cheap to clone — each field is
/// itself cheap.
#[derive(Clone)]
pub struct Adapters {
    pub idle: Arc<dyn IdleSource>,
    pub overlay: TauriOverlay,
    pub storage: StorageApp,
}

impl Adapters {
    /// Build the default bundle. Called from `setup` after the
    /// composition root has assembled the inputs.
    pub fn build(
        app: &tauri::AppHandle,
        idle: Arc<dyn IdleSource>,
        overlay: TauriOverlay,
        storage: StorageApp,
    ) -> Self {
        let _ = app; // future-proof: tray/icon access will live here
        Self {
            idle,
            overlay,
            storage,
        }
    }
}
