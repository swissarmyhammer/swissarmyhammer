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
}

impl AppState {
    /// Create a new AppState, loading config from disk.
    pub fn new() -> Self {
        Self {
            boards: RwLock::new(HashMap::new()),
            active_board: RwLock::new(None),
            config: RwLock::new(AppConfig::load()),
        }
    }

    /// Open a board at the given path, resolving to its .kanban directory.
    /// Returns the canonical path used as the map key.
    pub async fn open_board(&self, path: &Path) -> Result<PathBuf, String> {
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
            match handle.ctx.read_board().await {
                Ok(board) => board.name.clone(),
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

    /// Auto-open a board at startup by walking up from cwd, falling back to home dir.
    pub async fn auto_open_board(&self) {
        // Walk up from cwd looking for a .kanban/ directory
        let target = std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                let mut current = cwd;
                loop {
                    if current.join(".kanban").is_dir() {
                        return Some(current);
                    }
                    if !current.pop() {
                        return None;
                    }
                }
            })
            .or_else(dirs::home_dir);

        if let Some(path) = target {
            if let Err(e) = self.open_board(&path).await {
                tracing::warn!("Failed to auto-open board at {}: {}", path.display(), e);
            }
        }
    }

    /// Close a board, removing it from the open set.
    pub async fn close_board(&self, path: &Path) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut boards = self.boards.write().await;
        boards.remove(&canonical);

        // If this was the active board, clear active or switch to another
        let mut active = self.active_board.write().await;
        if active.as_deref() == Some(&canonical) {
            *active = boards.keys().next().cloned();
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
    fn test_mru_deduplicates() {
        let mut config = AppConfig::default();
        let path = PathBuf::from("/board/a");

        config.touch_recent(&path, "Board A");
        config.touch_recent(&PathBuf::from("/board/b"), "Board B");
        config.touch_recent(&path, "Board A Updated");

        assert_eq!(config.recent_boards.len(), 2);
        assert_eq!(config.recent_boards[0].name, "Board A Updated");
    }
}
