//! Adapters — bundles the concrete implementations the shell hands
//! to the engine. Lives in `src-tauri/` because the implementations
//! are tauri-bound (the plan §4 actually puts these in
//! `rewind-adapters`, but the overlay and tray adapters need tauri
//! types — so the tauri-bound ones live here, and the platform-only
//! `idle` adapter lives in `rewind-adapters`).

use std::sync::Arc;

use rewind_adapters::idle::pick;
use rewind_core::ports::IdleSource;

use crate::overlay_adapter::TauriOverlay;

/// All the OS / UI adapters the engine owns. Cheap to clone — each
/// field is itself cheap (Arc or AppHandle).
#[derive(Clone)]
pub struct Adapters {
    pub idle: Arc<dyn IdleSource>,
    pub overlay: TauriOverlay,
}

impl Adapters {
    /// Build the default adapter bundle. Call this from
    /// `setup(|app| …)` after the engine is constructed.
    pub fn build(app: &tauri::AppHandle) -> Self {
        Self {
            idle: Arc::from(pick()),
            overlay: TauriOverlay::new(app.clone()),
        }
    }
}