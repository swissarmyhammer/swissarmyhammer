---
title: Implement Rust state management with multi-board support
position:
  column: done
  ordinal: a3^
---
Create src/state.rs with AppState that supports multiple open boards and MRU persistence.

Hello

Key types:
- BoardHandle: KanbanContext + KanbanOperationProcessor + RwLock<Option<serde_json::Value>> cache
- AppState: RwLock<HashMap<PathBuf, BoardHandle>> + active_board: Option<PathBuf>
- RecentBoard: path, name, last_opened timestamp
- App config at dirs::config_dir()/swissarmyhammer-kanban/config.json

Key functions:
- resolve_kanban_path(path) — smart resolution:
  - path ends in .kanban and is dir → use directly
  - path is dir containing .kanban/ → use path/.kanban
  - otherwise → use path/.kanban (will be created)
  - never creates .kanban/.kanban (detect if already inside .kanban)
  - canonicalize paths for stable HashMap keys
- open_board(path) → resolve, create BoardHandle, insert, update MRU
- close_board(path) → remove from map
- On startup: auto-open cwd board (always a board available)

MRU config: ~20 entries max, sorted by last_opened desc. Load on startup.

Depends on: crate scaffold existing.
Verify: unit tests for resolve_kanban_path edge cases.