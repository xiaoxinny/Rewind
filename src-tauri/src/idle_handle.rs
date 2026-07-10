//! Wrapped `IdleSource` kept in Tauri's state. The Settings page
//! uses `reliable()` to decide whether to grey out the idle
//! thresholds (per §13 — "auto-greyed with explanation on GNOME
//! Wayland").

use rewind_core::ports::{IdleReliability, IdleSource};

/// Lightweight wrapper that exposes the idle-`reliability()` to IPC
/// without re-cloning the inner `Arc<dyn IdleSource>`.
#[derive(Clone)]
pub struct IdleHandle {
    inner: std::sync::Arc<dyn IdleSource>,
}

impl IdleHandle {
    pub fn new(inner: std::sync::Arc<dyn IdleSource>) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &std::sync::Arc<dyn IdleSource> {
        &self.inner
    }

    pub fn reliable(&self) -> bool {
        self.inner.reliability() == IdleReliability::Reliable
    }
}
