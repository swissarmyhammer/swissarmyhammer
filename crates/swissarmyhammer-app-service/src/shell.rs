//! The `AppShell` seam between the `app` operation tool and the OS / GUI.
//!
//! App-shell actions (quit, about, help) are inherently tauri-coupled: they
//! talk to the window manager and the running process. To keep the
//! [`AppService`](crate::AppService) testable without standing up a live GUI,
//! every such action goes through the [`AppShell`] trait rather than touching
//! a `tauri::AppHandle` directly.
//!
//! - The production impl, [`TauriAppShell`], backs the trait with a real
//!   `tauri::AppHandle`.
//! - Tests inject a spy that records calls and returns canned data, then
//!   assert the service drove the right shell method.

use serde::{Deserialize, Serialize};

/// Information about the running application, surfaced by
/// [`AppShell::show_about`].
///
/// Carries the human-readable app name and version so the frontend can render
/// an about dialog without a second round-trip to read package metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutInfo {
    /// The app's display name (e.g. `"kanban-app"`).
    pub name: String,
    /// The app's version string (e.g. `"0.13.5"`).
    pub version: String,
}

/// The seam between app-shell operations and the OS / GUI.
///
/// Implementors perform the actual side effects: terminating the process,
/// reading package metadata, and routing the user to help. The
/// [`AppService`](crate::AppService) holds an `Arc<dyn AppShell>` and calls
/// through this trait so the operation-dispatch path can be exercised with a
/// spy in tests.
pub trait AppShell: Send + Sync {
    /// Quit the application.
    ///
    /// The production impl terminates the process with exit code 0, matching
    /// the behavior of the original `quit_app` Tauri command.
    fn quit(&self);

    /// Return information about the running application for an about dialog.
    fn show_about(&self) -> AboutInfo;

    /// Route the user to the application's help / documentation.
    ///
    /// Returns the help target (e.g. a URL or an internal route) so the
    /// caller can render or follow it. The production impl additionally
    /// performs whatever native action is appropriate (e.g. emitting an event
    /// the frontend handles).
    fn show_help(&self) -> String;
}

/// The help target the production shell routes users to.
///
/// Kept as a module constant so the production impl and any test that wants to
/// assert the default agree on one value.
pub const HELP_TARGET: &str = "https://swissarmyhammer.github.io/swissarmyhammer/";

use tauri::{AppHandle, Emitter, Runtime};

/// Production [`AppShell`] backed by a live `tauri::AppHandle`.
///
/// Generic over the tauri [`Runtime`] so it works against both the real `Wry`
/// runtime and tauri's mock test runtime.
#[derive(Clone)]
pub struct TauriAppShell<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriAppShell<R> {
    /// Wrap a `tauri::AppHandle` as an [`AppShell`].
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> AppShell for TauriAppShell<R> {
    /// Terminate the process with exit code 0 — the ported `quit_app`
    /// behavior (`AppHandle::exit(0)`).
    fn quit(&self) {
        self.app.exit(0);
    }

    /// Read the app's name and version from the running process's package
    /// metadata.
    fn show_about(&self) -> AboutInfo {
        let info = self.app.package_info();
        AboutInfo {
            name: info.name.clone(),
            version: info.version.to_string(),
        }
    }

    /// Emit a `app://show-help` event carrying the help target so the frontend
    /// can navigate to it, and return that target.
    fn show_help(&self) -> String {
        if let Err(e) = self.app.emit("app://show-help", HELP_TARGET) {
            tracing::warn!("failed to emit app://show-help event: {e}");
        }
        HELP_TARGET.to_string()
    }
}
