---
title: Implement Tauri commands for board operations
position:
  column: done
  ordinal: a3C
---
Create src/commands.rs with Tauri commands that delegate to KanbanOperationProcessor.

Commands:
- get_board(path: Option<String>) → active board if None, specific if given. Uses GetBoard::default() via processor, caches result in BoardHandle.
- list_tasks(path: Option<String>) → same pattern, uses ListTasks via processor
- open_board(path: String) → resolve_kanban_path, create BoardHandle, set as active, update MRU. Returns board JSON.
- list_open_boards() → returns vec of {path, name, is_active} for all open boards
- set_active_board(path: String) → switch active board pointer
- get_recent_boards() → returns MRU list from config (for boards not currently open)

All commands return Result<serde_json::Value, String>. Error is KanbanError.to_string().
Processor auto-init creates "Untitled Board" if board.json missing, so open always succeeds.

Depends on: state management card.
Verify: cargo check passes with all commands registered.