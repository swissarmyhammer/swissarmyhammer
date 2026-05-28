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
//! `NewBoard` / `OpenBoard`) live alongside the window / file-action surface as
//! additional trait methods. Two of them — `NewBoard` and `OpenBoard` — show an
//! OS file dialog; that dialog goes through an injectable picker shim
//! ([`PickFolderFn`]) so tests can drive the "user chose folder X" / "user
//! cancelled" paths without standing up a native dialog. The board open / close
//! / init side effects thread through `AppState` (which this crate cannot
//! depend on), so — exactly as `open_new_window` does with [`OpenWindowFn`] —
//! they are supplied as injected callbacks the app-shell bootstrap wires up.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// A board created by [`WindowShell::new_board`].
///
/// Mirrors the result of the original `new_board_dialog` path: the user picked
/// a folder, a board was initialized at its `.kanban` directory (if not already
/// present), and the board was opened. Carries the resolved board path and the
/// board name derived from the chosen folder.
///
/// Named to parallel [`NewWindow`] (the result of opening a window) rather than
/// the `NewBoard` operation struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedBoard {
    /// The resolved `.kanban` board path the new board lives at.
    pub path: String,
    /// The board name, derived from the chosen folder's file name.
    pub name: String,
}

/// A board opened by [`WindowShell::open_board`].
///
/// Mirrors the result of the original `open_board_dialog` path: the user picked
/// an existing folder via the OS file-open dialog and the board there was
/// opened. Carries the resolved board path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenedBoard {
    /// The resolved `.kanban` board path that was opened.
    pub path: String,
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

    /// Switch the active board to the one at the given path.
    ///
    /// Backs `file.switchBoard`. Wraps `AppState::open_board` (which resolves
    /// the `.kanban` directory, opens / touches the board, and updates MRU
    /// tracking) without behavior change.
    fn switch_board(&self, path: &str) -> Result<(), String>;

    /// Close the board at the given path, removing it from the open set.
    ///
    /// Backs `file.closeBoard`. Wraps `AppState::close_board` (which removes the
    /// board, re-points MRU if needed, and stops any running agent) without
    /// behavior change.
    fn close_board(&self, path: &str) -> Result<(), String>;

    /// Create a new board via the OS folder picker.
    ///
    /// Backs `file.newBoard`. Ports `new_board_dialog`: shows the folder picker,
    /// derives the board name from the chosen folder, initializes a board at its
    /// `.kanban` directory (if not already a board), and opens it. Returns the
    /// resulting [`CreatedBoard`], or an error string on failure.
    ///
    /// The picker shim returning "cancelled" is surfaced as an error so the op
    /// can report it; this is the only board op that always produces a board on
    /// success (the dialog path has no "opened nothing" success state for new).
    fn new_board(&self) -> Result<CreatedBoard, String>;

    /// Open an existing board via the OS file-open dialog.
    ///
    /// Backs `file.openBoard`. Ports `open_board_dialog`: shows the folder
    /// picker and opens the chosen board. Returns `Some(OpenedBoard)` for the
    /// chosen board, or `None` when the user cancelled the dialog.
    fn open_board(&self) -> Result<Option<OpenedBoard>, String>;
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

/// The injectable OS folder-picker shim for the board dialogs.
///
/// `NewBoard` and `OpenBoard` show a native folder picker; production wires
/// this to Tauri's `dialog().file().pick_folder(...)`, while tests inject a
/// closure returning a fixed path (or `None` to model the user cancelling).
/// Returning `None` means the dialog was dismissed without a choice.
pub type PickFolderFn = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;

/// A callback that initializes a board at a resolved `.kanban` path.
///
/// Board initialization runs the kanban `InitBoard` operation, which lives in a
/// crate this library does not — and should not — depend on. The bootstrap site
/// supplies this closure wired to the real init processor; it is a no-op when
/// the path is already an initialized board.
pub type InitBoardFn = Arc<dyn Fn(&Path, &str) -> Result<(), String> + Send + Sync>;

/// A callback that opens / switches the active board to the one at the given
/// path, threading through `AppState`.
///
/// `AppState::open_board` (board resolution, watcher start, MRU tracking) lives
/// in the app bin crate this library cannot reach, mirroring why
/// [`OpenWindowFn`] is injected. The bootstrap supplies this wired to the real
/// `AppState`; it backs `switch_board`, `new_board`, and `open_board`.
pub type SwitchBoardFn<R> =
    Arc<dyn Fn(&AppHandle<R>, &Path) -> Result<(), String> + Send + Sync>;

/// A callback that closes the board at the given path via `AppState`.
///
/// Wraps `AppState::close_board` for the same reason [`SwitchBoardFn`] wraps
/// `open_board`. Backs `close_board`.
pub type CloseBoardFn<R> = Arc<dyn Fn(&AppHandle<R>, &Path) -> Result<(), String> + Send + Sync>;

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
    pick_folder: PickFolderFn,
    init_board: InitBoardFn,
    switch_board: SwitchBoardFn<R>,
    close_board: CloseBoardFn<R>,
}

impl<R: Runtime> TauriWindowShell<R> {
    /// Wrap a `tauri::AppHandle` plus the injected callbacks as a
    /// [`WindowShell`].
    ///
    /// `open_window` backs new-window creation; `pick_folder` is the OS
    /// folder-picker shim the board dialogs drive; `init_board` initializes a
    /// board at a resolved path; `switch_board` / `close_board` thread board
    /// open / close through `AppState`. Each touches state this crate cannot
    /// own, so all are supplied by the app-shell bootstrap.
    pub fn new(
        app: AppHandle<R>,
        open_window: OpenWindowFn<R>,
        pick_folder: PickFolderFn,
        init_board: InitBoardFn,
        switch_board: SwitchBoardFn<R>,
        close_board: CloseBoardFn<R>,
    ) -> Self {
        Self {
            app,
            open_window,
            pick_folder,
            init_board,
            switch_board,
            close_board,
        }
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

    fn switch_board(&self, path: &str) -> Result<(), String> {
        (self.switch_board)(&self.app, Path::new(path))
    }

    fn close_board(&self, path: &str) -> Result<(), String> {
        (self.close_board)(&self.app, Path::new(path))
    }

    /// Port of `new_board_dialog` / `handle_new_board`, threaded through the
    /// injected `AppState` board-open callback.
    fn new_board(&self) -> Result<CreatedBoard, String> {
        run_new_board(&self.pick_folder, |kanban_path, name| {
            (self.init_board)(kanban_path, name)
        })
        .and_then(|created| match created {
            Some((created, folder)) => {
                (self.switch_board)(&self.app, &folder)?;
                Ok(created)
            }
            None => Err("new board cancelled".to_string()),
        })
    }

    /// Port of `open_board_dialog` / `handle_open_board`, threaded through the
    /// injected `AppState` board-open callback.
    fn open_board(&self) -> Result<Option<OpenedBoard>, String> {
        match run_open_board(&self.pick_folder)? {
            Some((opened, folder)) => {
                (self.switch_board)(&self.app, &folder)?;
                Ok(Some(opened))
            }
            None => Ok(None),
        }
    }
}

/// The ported `new_board_dialog` / `handle_new_board` file/IO logic, decoupled
/// from `AppState` and the tauri runtime so it is unit-testable on disk.
///
/// Shows the folder picker, derives the board name, resolves its `.kanban`
/// path, and initializes the board there via `init_board`. Returns the
/// [`CreatedBoard`] paired with the originally-chosen folder (which the caller
/// passes to the `AppState` board-open seam), or `None` when the picker was
/// cancelled.
///
/// `init_board` is the kanban `InitBoard` step, supplied by the caller because
/// it lives in a crate this one cannot depend on; it is a no-op when the path is
/// already an initialized board.
pub fn run_new_board(
    pick_folder: &PickFolderFn,
    init_board: impl FnOnce(&Path, &str) -> Result<(), String>,
) -> Result<Option<(CreatedBoard, PathBuf)>, String> {
    let Some(folder) = pick_folder() else {
        return Ok(None);
    };
    let name = board_name_from_folder(&folder);
    let kanban_path = resolve_kanban_path(&folder);
    init_board(&kanban_path, &name)?;
    let created = CreatedBoard {
        path: kanban_path.display().to_string(),
        name,
    };
    Ok(Some((created, folder)))
}

/// The ported `open_board_dialog` / `handle_open_board` file/IO logic, decoupled
/// from `AppState` and the tauri runtime so it is unit-testable.
///
/// Shows the folder picker and resolves the chosen folder to its `.kanban`
/// path. Returns the [`OpenedBoard`] paired with the originally-chosen folder
/// (which the caller passes to the `AppState` board-open seam), or `None` when
/// the picker was cancelled.
pub fn run_open_board(
    pick_folder: &PickFolderFn,
) -> Result<Option<(OpenedBoard, PathBuf)>, String> {
    let Some(folder) = pick_folder() else {
        return Ok(None);
    };
    let kanban_path = resolve_kanban_path(&folder);
    let opened = OpenedBoard {
        path: kanban_path.display().to_string(),
    };
    Ok(Some((opened, folder)))
}

/// Derive a board name from a chosen folder, ported from `handle_new_board`.
///
/// Uses the folder's file name, falling back to `"New Board"` when the path has
/// no usable final component (e.g. a root path).
fn board_name_from_folder(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("New Board")
        .to_string()
}

/// Resolve a chosen folder to its `.kanban` board directory.
///
/// Behavior-preserving port of `AppState::resolve_kanban_path`
/// (`apps/kanban-app/src/state.rs`): the path itself may be a `.kanban`
/// directory, may live inside one, or may be a folder that contains (or will
/// contain) a `.kanban` child. Unlike the original this never returns an error
/// — the original's only error channel was unreachable in practice — so callers
/// get a plain `PathBuf`.
fn resolve_kanban_path(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    // Already a .kanban directory.
    if path.file_name().and_then(|n| n.to_str()) == Some(".kanban") && path.is_dir() {
        return path;
    }

    // Inside a .kanban directory (e.g. /foo/.kanban/tasks).
    for ancestor in path.ancestors() {
        if ancestor.file_name().and_then(|n| n.to_str()) == Some(".kanban") && ancestor.is_dir() {
            return ancestor.to_path_buf();
        }
    }

    // A folder that contains (or will contain) a .kanban child.
    path.join(".kanban")
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
