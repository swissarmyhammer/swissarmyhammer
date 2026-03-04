---
position_column: done
position_ordinal: i4
title: Wire up delete Tauri commands for all entity types
---
The kanban layer has DeleteTask, DeleteTag, DeleteColumn, DeleteActor, DeleteSwimlane, and DeleteAttachment operations implemented and tested — but none are exposed as Tauri commands. The UI has no way to delete anything.

## Commands to add

All follow the same pattern as existing commands — get the BoardHandle from AppState, run through the processor, return result with operation ULID.

```rust
#[tauri::command]
async fn delete_task(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]
async fn delete_tag(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]
async fn delete_column(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]
async fn delete_actor(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]
async fn delete_swimlane(state: State<'_, AppState>, id: String) -> Result<Value, String>

#[tauri::command]
async fn delete_attachment(state: State<'_, AppState>, task_id: String, id: String) -> Result<Value, String>
```

Each returns the operation/transaction ULID so the frontend can push it onto the undo stack.

## Notes
- DeleteColumn and DeleteSwimlane refuse if non-empty (return error) — the Tauri command should pass that error through cleanly
- DeleteTask cleans up depends_on references and deletes attachments — composite, returns transaction ULID
- DeleteTag removes #tag from all task bodies — composite, returns transaction ULID
- DeleteActor removes from all task assignees — composite, returns transaction ULID
- DeleteAttachment removes from parent task's attachment list — two-phase

## Files
- `swissarmyhammer-kanban-app/src/commands.rs` — add 6 new commands
- `swissarmyhammer-kanban-app/src/main.rs` — register commands in invoke_handler

## Checklist
- [ ] Add delete_task command
- [ ] Add delete_tag command
- [ ] Add delete_column command
- [ ] Add delete_actor command
- [ ] Add delete_swimlane command
- [ ] Add delete_attachment command
- [ ] Register all in invoke_handler
- [ ] Each returns operation ULID in result
- [ ] Test error case: delete non-empty column returns clean error
- [ ] Run full test suite