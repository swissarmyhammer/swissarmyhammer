//! Application state management with multi-board support and MRU persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor};
use tokio::sync::RwLock;

const MAX_RECENT_BOARDS: usize = 20;
const CONFIG_DIR_NAME: &str = "swissarmyhammer-kanban";
const CONFIG_FILE_NAME: &str = "config.json";

/// A handle to a single open kanban board.
pub struct BoardHandle {
    pub ctx: KanbanContext,
    pub processor: KanbanOperationProcessor,
}

impl BoardHandle {
    pub fn new(kanban_path: PathBuf) -> Self {
        Self {
            ctx: KanbanContext::new(kanban_path),
            processor: KanbanOperationProcessor::new(),
        }
    }
}

/// A recently opened board entry for MRU persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentBoard {
    pub path: PathBuf,
    pub name: String,
    pub last_opened: DateTime<Utc>,
}

/// Persisted app configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub recent_boards: Vec<RecentBoard>,
    #[serde(default = "default_keymap_mode")]
    pub keymap_mode: String,
}

fn default_keymap_mode() -> String {
    "cua".to_string()
}

impl AppConfig {
    /// Load config from disk, returning default if not found.
    pub fn load() -> Self {
        let path = config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save config to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)
    }

    /// Add or update a board in the MRU list.
    pub fn touch_recent(&mut self, path: &Path, name: &str) {
        // Remove existing entry for this path
        self.recent_boards.retain(|r| r.path != path);

        // Insert at front
        self.recent_boards.insert(
            0,
            RecentBoard {
                path: path.to_path_buf(),
                name: name.to_string(),
                last_opened: Utc::now(),
            },
        );

        // Truncate to max
        self.recent_boards.truncate(MAX_RECENT_BOARDS);
    }
}

/// The shared application state, managed by Tauri.
pub struct AppState {
    pub boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>,
    pub active_board: RwLock<Option<PathBuf>>,
    pub config: RwLock<AppConfig>,
    /// Tag context menu state: (tag_id, optional task_id) set before popup, read in menu event
    pub context_tag: RwLock<Option<(String, Option<String>)>>,
}

impl AppState {
    /// Create a new AppState, loading config from disk.
    pub fn new() -> Self {
        Self {
            boards: RwLock::new(HashMap::new()),
            active_board: RwLock::new(None),
            config: RwLock::new(AppConfig::load()),
            context_tag: RwLock::new(None),
        }
    }

    /// Open a board at the given path, resolving to its .kanban directory.
    /// Returns the canonical path used as the map key.
    pub async fn open_board(&self, path: &Path) -> Result<PathBuf, String> {
        tracing::info!("Opening board at {}", path.display());
        let kanban_path = resolve_kanban_path(path).map_err(|e| e.to_string())?;

        let canonical = kanban_path
            .canonicalize()
            .unwrap_or_else(|_| kanban_path.clone());

        // Check if already open
        {
            let boards = self.boards.read().await;
            if boards.contains_key(&canonical) {
                // Already open — just update active
                *self.active_board.write().await = Some(canonical.clone());
                return Ok(canonical);
            }
        }

        let handle = Arc::new(BoardHandle::new(kanban_path));

        // Read board name for MRU
        let board_name = if handle.ctx.is_initialized() {
            match handle.ctx.entity_context().await {
                Ok(ectx) => match ectx.read("board", "board").await {
                    Ok(entity) => entity.get_str("name").unwrap_or("").to_string(),
                    Err(_) => canonical.display().to_string(),
                },
                Err(_) => canonical.display().to_string(),
            }
        } else {
            canonical.display().to_string()
        };

        {
            let mut boards = self.boards.write().await;
            boards.insert(canonical.clone(), handle);
        }

        // Update MRU
        {
            let mut config = self.config.write().await;
            config.touch_recent(&canonical, &board_name);
            let _ = config.save();
        }

        *self.active_board.write().await = Some(canonical.clone());
        Ok(canonical)
    }

    /// Auto-open a board at startup by walking up from CWD looking for a `.kanban` directory.
    ///
    /// If no `.kanban` directory is found in any ancestor, the app starts without
    /// a board (the frontend shows the "No board loaded" prompt).
    pub async fn auto_open_board(&self) {
        let cwd = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                tracing::warn!("Cannot determine current directory: {e}");
                return;
            }
        };
        tracing::info!(cwd = %cwd.display(), "auto_open_board: starting discovery");

        // Strategy 1: walk up from CWD
        let mut board_dir = discover_board(&cwd);
        if let Some(ref dir) = board_dir {
            tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via CWD walk");
        } else {
            tracing::info!("auto_open_board: no .kanban found walking up from CWD");
        }

        // Strategy 2: if CWD walk didn't pass through home, check home as backstop
        if board_dir.is_none() {
            if let Some(home) = dirs::home_dir() {
                let walked_through_home = cwd.starts_with(&home);
                tracing::info!(
                    home = %home.display(),
                    walked_through_home,
                    "auto_open_board: checking home dir backstop"
                );
                if !walked_through_home {
                    board_dir = discover_board(&home);
                    if let Some(ref dir) = board_dir {
                        tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via home backstop");
                    }
                }
            }
        }

        // Strategy 3: fall back to MRU — the most recently opened board
        if board_dir.is_none() {
            let config = self.config.read().await;
            if let Some(recent) = config.recent_boards.first() {
                let path = &recent.path;
                tracing::info!(
                    path = %path.display(),
                    name = %recent.name,
                    "auto_open_board: falling back to MRU board"
                );
                // MRU stores the canonical .kanban path — check its parent exists
                if path.is_dir() {
                    board_dir = Some(path.clone());
                } else {
                    tracing::warn!(
                        path = %path.display(),
                        "auto_open_board: MRU path no longer exists"
                    );
                }
            } else {
                tracing::info!("auto_open_board: no MRU boards in config");
            }
        }

        match board_dir {
            Some(ref dir) => {
                tracing::info!(path = %dir.display(), "auto_open_board: opening board");
                if let Err(e) = self.open_board(dir).await {
                    tracing::warn!(
                        path = %dir.display(),
                        error = %e,
                        "auto_open_board: failed to open board"
                    );
                }
            }
            None => {
                tracing::info!("auto_open_board: no board found, starting without one");
            }
        }
    }

    /// Get the handle for the active board.
    pub async fn active_handle(&self) -> Option<Arc<BoardHandle>> {
        let active = self.active_board.read().await;
        let path = active.as_ref()?;
        let boards = self.boards.read().await;
        boards.get(path).cloned()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Walk up from `start_dir` looking for a `.kanban` subdirectory.
///
/// Returns the parent directory containing `.kanban` (not the `.kanban` dir itself),
/// or `None` if no ancestor contains one.
pub fn discover_board(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        let candidate = current.join(".kanban");
        tracing::debug!("Checking for board at {}", candidate.display());
        if candidate.is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Resolve a user-provided path to a .kanban directory path.
///
/// Rules:
/// - If path ends in `.kanban` and is a directory, use it directly
/// - If path is a directory containing `.kanban/`, use `path/.kanban`
/// - If we're already inside a `.kanban` dir, use it (don't nest)
/// - Otherwise, assume `path/.kanban`
pub fn resolve_kanban_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    // Already a .kanban directory
    if path.file_name().and_then(|n| n.to_str()) == Some(".kanban") && path.is_dir() {
        return Ok(path);
    }

    // Check if we're inside a .kanban directory (e.g. path is /foo/.kanban/tasks)
    for ancestor in path.ancestors() {
        if ancestor.file_name().and_then(|n| n.to_str()) == Some(".kanban") && ancestor.is_dir() {
            return Ok(ancestor.to_path_buf());
        }
    }

    // Directory that contains .kanban/
    let child = path.join(".kanban");
    if child.is_dir() {
        return Ok(child);
    }

    // Default: will be created at path/.kanban
    Ok(child)
}

/// Get the path to the app config file.
fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_existing_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing the .kanban dir directly
        let result = resolve_kanban_path(&kanban_dir).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_parent_containing_kanban() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing the parent directory
        let result = resolve_kanban_path(tmp.path()).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_inside_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        let tasks_dir = kanban_dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        // Passing a path inside .kanban
        let result = resolve_kanban_path(&tasks_dir).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_no_kanban_yet() {
        let tmp = TempDir::new().unwrap();

        // No .kanban exists — should return path/.kanban
        let result = resolve_kanban_path(tmp.path()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap().join(".kanban"));
    }

    #[test]
    fn test_resolve_never_nests_kanban() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing .kanban itself should NOT create .kanban/.kanban
        let result = resolve_kanban_path(&kanban_dir).unwrap();
        assert!(
            !result.ends_with(".kanban/.kanban"),
            "Should never nest .kanban: {:?}",
            result
        );
    }

    #[test]
    fn test_mru_config_touch_and_truncate() {
        let mut config = AppConfig::default();

        for i in 0..25 {
            config.touch_recent(
                &PathBuf::from(format!("/board/{}", i)),
                &format!("Board {}", i),
            );
        }

        assert_eq!(config.recent_boards.len(), MAX_RECENT_BOARDS);
        // Most recent should be first
        assert_eq!(config.recent_boards[0].name, "Board 24");
    }

    #[test]
    fn test_discover_board_found_in_cwd() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let result = discover_board(tmp.path());
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn test_discover_board_found_in_ancestor() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let nested = tmp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();

        let result = discover_board(&nested);
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn test_discover_board_not_found() {
        let tmp = TempDir::new().unwrap();
        // No .kanban anywhere
        let result = discover_board(tmp.path());
        assert_eq!(result, None);
    }

    #[test]
    fn test_mru_deduplicates() {
        let mut config = AppConfig::default();
        let path = PathBuf::from("/board/a");

        config.touch_recent(&path, "Board A");
        config.touch_recent(&PathBuf::from("/board/b"), "Board B");
        config.touch_recent(&path, "Board A Updated");

        assert_eq!(config.recent_boards.len(), 2);
        assert_eq!(config.recent_boards[0].name, "Board A Updated");
    }

    // =========================================================================
    // Integration tests for auto_open_board
    // =========================================================================

    /// Helper: create a minimal .kanban board structure that the entity system
    /// can load. This means .kanban/boards/board.yaml must exist (the entity
    /// location, not just the legacy root-level board.yaml).
    fn create_board_at(root: &Path, name: &str) {
        let kanban_dir = root.join(".kanban");
        let boards_dir = kanban_dir.join("boards");
        std::fs::create_dir_all(&boards_dir).unwrap();
        std::fs::write(boards_dir.join("board.yaml"), format!("name: {}\n", name)).unwrap();
        // Also create columns dir so the processor doesn't try to auto-init
        std::fs::create_dir_all(kanban_dir.join("columns")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("tasks")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("tags")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("actors")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("swimlanes")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("activity")).unwrap();
    }

    #[tokio::test]
    async fn test_auto_open_board_from_cwd() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "Test Board");

        // Simulate CWD being inside the project
        let subdir = tmp.path().join("src").join("components");
        std::fs::create_dir_all(&subdir).unwrap();

        let state = AppState::new();
        // Manually run discovery from the subdir (can't change real CWD in tests)
        let board_dir = discover_board(&subdir);
        assert_eq!(board_dir, Some(tmp.path().to_path_buf()));

        // Open it
        let result = state.open_board(board_dir.as_ref().unwrap()).await;
        assert!(result.is_ok(), "open_board failed: {:?}", result.err());

        // Verify active board is set
        let handle = state.active_handle().await;
        assert!(handle.is_some(), "active_handle should be Some after open");
    }

    #[tokio::test]
    async fn test_auto_open_board_no_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        // No .kanban anywhere

        let result = discover_board(tmp.path());
        assert_eq!(result, None);

        let state = AppState::new();
        // No board opened — active_handle should be None
        let handle = state.active_handle().await;
        assert!(handle.is_none());
    }

    #[tokio::test]
    async fn test_open_board_sets_active_and_appears_in_boards() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "My Board");

        let state = AppState::new();
        let result = state.open_board(tmp.path()).await;
        assert!(result.is_ok());

        let canonical = result.unwrap();

        // active_board should be set
        let active = state.active_board.read().await;
        assert_eq!(*active, Some(canonical.clone()));

        // boards map should contain the handle
        let boards = state.boards.read().await;
        assert!(boards.contains_key(&canonical));
    }
}
