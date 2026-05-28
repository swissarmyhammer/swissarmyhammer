//! The `WindowShell` seam between the `window` operation tool and the OS / GUI.
//!
//! Window operations (open / activate / position / monitors / close) and the
//! OS-level file actions (open in default app, reveal in file manager) are all
//! inherently tauri- and OS-coupled. To keep the
//! [`WindowService`](crate::WindowService) testable without standing up a live
//! GUI or poking the real Finder, every such action goes through the
//! [`WindowShell`] trait rather than touching a `tauri::AppHandle` or spawning
//! an OS process directly.
//!
//! - The production impl, [`TauriWindowShell`], backs the trait with a real
//!   `tauri::AppHandle` (for window ops) and the OS opener / file-manager
//!   commands (for the file actions).
//! - Tests inject a spy that records calls and returns canned data, then assert
//!   the service drove the right shell method with the right arguments.
//!
//! The board-file lifecycle operations (`SwitchBoard` / `CloseBoard` /
//! `NewBoard` / `OpenBoard`) are a separate follow-up task on this same crate.
//! They are deliberately absent here, but the seam is shaped so they can be
//! added as additional trait methods without disturbing the window / file-action
//! surface.

use serde::{Deserialize, Serialize};

/// A window's top-left position in logical pixels relative to the primary
/// display's top-left origin.
///
/// Matches the coordinate space Tauri's `Window::outer_position()` reports and
/// `Window::set_position()` consumes, so the production shell can round-trip a
/// position without conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPosition {
    /// Logical-pixel x coordinate of the window's top-left corner.
    pub x: i32,
    /// Logical-pixel y coordinate of the window's top-left corner.
    pub y: i32,
}

/// Description of a connected monitor, surfaced by [`WindowShell::get_monitors`].
///
/// Carries the geometry a caller needs to place windows across displays without
/// a second round-trip into the windowing system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Human-readable monitor name, when the platform reports one.
    pub name: Option<String>,
    /// Logical-pixel x coordinate of the monitor's top-left corner.
    pub x: i32,
    /// Logical-pixel y coordinate of the monitor's top-left corner.
    pub y: i32,
    /// Monitor width in logical pixels.
    pub width: u32,
    /// Monitor height in logical pixels.
    pub height: u32,
    /// The monitor's scale factor (e.g. `2.0` for a Retina display).
    pub scale_factor: f64,
}

/// The label assigned to a newly created window, returned by
/// [`WindowShell::open_new_window`].
///
/// Mirrors the `{ label, board_path }` shape the original `create_window` Tauri
/// command returned so the relocation is behavior-preserving.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewWindow {
    /// The window label (a freshly generated ULID-based label, or a reused one).
    pub label: String,
    /// The board path the window was opened against, if any was resolved.
    pub board_path: Option<String>,
}

/// The seam between `window` operations and the OS / GUI.
///
/// Implementors perform the actual side effects: opening / activating / closing
/// windows, reading and writing window positions, enumerating monitors, opening
/// a file in the OS default app, and revealing a file in the OS file manager.
/// The [`WindowService`](crate::WindowService) holds an `Arc<dyn WindowShell>`
/// and calls through this trait so the operation-dispatch path can be exercised
/// with a spy in tests.
///
/// All methods are fallible: window ops can fail when a label is unknown, and
/// the file actions can fail when the OS opener / file-manager command errors.
/// Errors are returned as human-readable strings, which the service maps onto
/// rmcp `internal_error`s.
pub trait WindowShell: Send + Sync {
    /// Open a new application window, optionally pointed at a board path.
    ///
    /// Ports the original `create_window` Tauri command: resolves the board to
    /// display, builds the webview window, shows and focuses it, and returns its
    /// label and resolved board path.
    fn open_new_window(&self, board_path: Option<String>) -> Result<NewWindow, String>;

    /// Bring the window with the given label to the front and focus it.
    fn activate_window(&self, label: &str) -> Result<(), String>;

    /// Move the window with the given label to the given logical-pixel position.
    fn set_window_position(&self, label: &str, position: WindowPosition) -> Result<(), String>;

    /// Read the current logical-pixel position of the window with the given
    /// label.
    fn get_window_position(&self, label: &str) -> Result<WindowPosition, String>;

    /// Enumerate the connected monitors.
    fn get_monitors(&self) -> Result<Vec<MonitorInfo>, String>;

    /// Close the window with the given label.
    fn close_window(&self, label: &str) -> Result<(), String>;

    /// Open a file in the OS default application.
    ///
    /// Backs `attachment.open`. Ports the `AttachmentOpenCmd` behavior
    /// (`open::that`) out of the kanban command path into the shell seam.
    fn open_path(&self, path: &str) -> Result<(), String>;

    /// Reveal a file in the OS file manager (Finder / Explorer / file browser).
    ///
    /// Backs `attachment.reveal`. Ports the `AttachmentRevealCmd` behavior — the
    /// platform-specific "reveal" command (`open -R` / `xdg-open` parent /
    /// `explorer /select,`) — out of the kanban command path into the shell
    /// seam.
    fn reveal_path(&self, path: &str) -> Result<(), String>;
}

use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

/// A callback that opens a new window given the live `AppHandle` and the
/// requested board path, returning the new window's label and resolved board.
///
/// Window creation in the production app is async and threads through
/// `AppState` (board resolution, geometry restore, UIState persistence), none
/// of which this crate can — or should — reimplement. The bootstrap site
/// (the app-shell cut-over) supplies this closure wired to `create_window_impl`;
/// the shape here is the minimal contract the seam needs.
pub type OpenWindowFn<R> =
    Arc<dyn Fn(&AppHandle<R>, Option<String>) -> Result<NewWindow, String> + Send + Sync>;

/// Production [`WindowShell`] backed by a live `tauri::AppHandle`.
///
/// Generic over the tauri [`Runtime`] so it works against both the real `Wry`
/// runtime and tauri's mock test runtime. Window enumeration, positioning,
/// activation, and closing go through the `AppHandle`'s window manager; the
/// file actions go through the OS opener and the platform file-manager command.
/// New-window creation is delegated to the injected [`OpenWindowFn`] because it
/// requires app state this crate does not own.
#[derive(Clone)]
pub struct TauriWindowShell<R: Runtime> {
    app: AppHandle<R>,
    open_window: OpenWindowFn<R>,
}

impl<R: Runtime> TauriWindowShell<R> {
    /// Wrap a `tauri::AppHandle` plus a new-window callback as a [`WindowShell`].
    pub fn new(app: AppHandle<R>, open_window: OpenWindowFn<R>) -> Self {
        Self { app, open_window }
    }
}

impl<R: Runtime> WindowShell for TauriWindowShell<R> {
    fn open_new_window(&self, board_path: Option<String>) -> Result<NewWindow, String> {
        (self.open_window)(&self.app, board_path)
    }

    fn activate_window(&self, label: &str) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("no window with label {label:?}"))?;
        window
            .set_focus()
            .map_err(|e| format!("failed to focus window {label:?}: {e}"))
    }

    fn set_window_position(&self, label: &str, position: WindowPosition) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("no window with label {label:?}"))?;
        window
            .set_position(tauri::PhysicalPosition::new(position.x, position.y))
            .map_err(|e| format!("failed to set position of window {label:?}: {e}"))
    }

    fn get_window_position(&self, label: &str) -> Result<WindowPosition, String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("no window with label {label:?}"))?;
        let pos = window
            .outer_position()
            .map_err(|e| format!("failed to read position of window {label:?}: {e}"))?;
        Ok(WindowPosition { x: pos.x, y: pos.y })
    }

    fn get_monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        let monitors = self
            .app
            .available_monitors()
            .map_err(|e| format!("failed to enumerate monitors: {e}"))?;
        Ok(monitors
            .into_iter()
            .map(|m| {
                let pos = m.position();
                let size = m.size();
                MonitorInfo {
                    name: m.name().map(|n| n.to_string()),
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                    scale_factor: m.scale_factor(),
                }
            })
            .collect())
    }

    fn close_window(&self, label: &str) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("no window with label {label:?}"))?;
        window
            .close()
            .map_err(|e| format!("failed to close window {label:?}: {e}"))
    }

    /// Open a file in the OS default application via the `open` crate — the
    /// ported `AttachmentOpenCmd` behavior.
    fn open_path(&self, path: &str) -> Result<(), String> {
        open::that(path).map_err(|e| format!("failed to open {path}: {e}"))
    }

    /// Reveal a file in the OS file manager — the ported `AttachmentRevealCmd`
    /// behavior, branching per platform.
    fn reveal_path(&self, path: &str) -> Result<(), String> {
        reveal_in_file_manager(path).map_err(|e| format!("failed to reveal {path}: {e}"))?;
        Ok(())
    }
}

/// Spawn the platform-specific "reveal in file manager" command.
///
/// Returns the exit status of the spawned process. Each platform uses a
/// different binary and argument convention, so we branch at compile time with
/// `#[cfg(target_os)]`. Ported verbatim (behavior-preserving) from
/// `AttachmentRevealCmd` in `swissarmyhammer-kanban`:
/// - macOS: `open -R <path>` (selects the file in Finder)
/// - Linux: `xdg-open <parent>` (opens the parent directory)
/// - Windows: `explorer /select,<path>` (selects the file in Explorer)
fn reveal_in_file_manager(path: &str) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
    }
    #[cfg(target_os = "linux")]
    {
        // xdg-open cannot select a specific file, so open the parent directory.
        let parent = std::path::Path::new(path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        std::process::Command::new("xdg-open").arg(parent).status()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .status()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "reveal-in-file-manager is not supported on this platform".to_string(),
        ))
    }
}
