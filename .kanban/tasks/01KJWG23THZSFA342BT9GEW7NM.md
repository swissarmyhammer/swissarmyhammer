---
position_column: done
position_ordinal: i5
title: Expose undo/redo as Tauri commands, return operation ULIDs from mutations
---
The frontend needs two things from the backend:
1. Every mutation command returns an operation/transaction ULID so the client can track it in its undo stack
2. `undo_operation` and `redo_operation` Tauri commands that accept a ULID

## Return format
Add `operation_id` as a top-level field on the existing result JSON for all mutation commands. No envelope wrapper — just mix it into the domain result object.

## Changes

### All mutation Tauri commands return ULID
Add `operation_id: "<ulid>"` to every mutation result JSON.

### New Tauri commands
```rust
#[tauri::command]
async fn undo_operation(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]  
async fn redo_operation(state: State<'_, AppState>, id: String) -> Result<Value, String>
```

These delegate to EntityContext::undo/redo which handles both single-entity ops and transactions.

### Remove convenience wrapper commands
Remove `update_task_title`, `update_task_description`, `rename_column` — everything routes through `update_entity_field` which returns the operation ULID.

## Files
- `swissarmyhammer-kanban-app/src/commands.rs` — add undo/redo commands, update mutation commands to return ULIDs, remove convenience wrappers
- `swissarmyhammer-kanban-app/ui/src/App.tsx` — update invoke calls to use `update_entity_field` instead of removed wrappers

## Checklist
- [ ] Add `undo_operation` Tauri command
- [ ] Add `redo_operation` Tauri command
- [ ] Update all mutation commands to include operation_id in result JSON
- [ ] Remove `update_task_title` command (use `update_entity_field`)
- [ ] Remove `update_task_description` command (use `update_entity_field`)
- [ ] Remove `rename_column` command (use `update_entity_field`)
- [ ] Update frontend invoke calls for removed commands
- [ ] Test undo/redo round-trip through Tauri IPC
- [ ] Run full test suite