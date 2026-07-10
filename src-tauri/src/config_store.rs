//! `ConfigStore` — bridges `rewind_core::AppConfig` ↔
//! `tauri-plugin-store`'s JSON-on-disk store.
//!
//! The store file is named `config.json` inside the app's data
//! directory. The first read seeds `AppConfig::default()` if the
//! file is missing or empty.

use rewind_core::AppConfig;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "config.json";
const CONFIG_KEY: &str = "app_config";

/// Load the saved `AppConfig` from the JSON store, or fall back to
/// `AppConfig::default()` (which is also written back so the next
/// boot finds a valid file).
pub fn load_or_default(app: &AppHandle) -> AppConfig {
    let store = match app.store(STORE_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Rewind: failed to open store {STORE_PATH}: {e}; using defaults");
            return AppConfig::default();
        }
    };

    if let Some(value) = store.get(CONFIG_KEY) {
        match serde_json::from_value::<AppConfig>(value) {
            Ok(cfg) => return cfg,
            Err(e) => {
                eprintln!(
                    "Rewind: failed to deserialize saved AppConfig: {e}; falling back to defaults"
                );
            }
        }
    }

    let cfg = AppConfig::default();
    save(app, &cfg);
    cfg
}

/// Persist the current `AppConfig` to the store. Idempotent — safe
/// to call after every settings change.
pub fn save(app: &AppHandle, cfg: &AppConfig) {
    let store = match app.store(STORE_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Rewind: failed to open store {STORE_PATH}: {e}");
            return;
        }
    };
    let json = match serde_json::to_value(cfg) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Rewind: failed to serialize AppConfig: {e}");
            return;
        }
    };
    store.set(CONFIG_KEY, json);
    if let Err(e) = store.save() {
        eprintln!("Rewind: failed to save AppConfig to disk: {e}");
    }
}
