//! Tauri-based init/deinit progress reporter.
//!
//! Bridges the `InitReporter` trait from `swissarmyhammer-common` to the Tauri
//! event system, so the frontend can display init progress via toast
//! notifications.

use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use tauri::{AppHandle, Emitter};

/// Reports init/deinit progress via Tauri `init-progress` events.
///
/// Each `InitEvent` is serialized as a tagged-enum JSON payload and emitted
/// to all listening webview windows.
pub struct TauriReporter {
    app_handle: AppHandle,
}

impl TauriReporter {
    /// Create a new reporter that emits events through the given app handle.
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl InitReporter for TauriReporter {
    fn emit(&self, event: &InitEvent) {
        let _ = self.app_handle.emit("init-progress", event);
    }
}
