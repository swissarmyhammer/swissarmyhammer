---
assignees:
- claude-code
depends_on:
- 01KMX2PFZ0SYDJT8E3ZCEH1YY1
position_column: done
position_ordinal: ffffffffffffffeb80
title: CopyTask, CutTask, PasteTask entity kernel operations
---
## What

Implement cut/copy/paste as proper entity kernel operations with `#[operation]` macro, using ClipboardProvider for system clipboard I/O.

### Files to create/modify
- `swissarmyhammer-kanban/src/task/copy.rs` — new: CopyTask operation
- `swissarmyhammer-kanban/src/task/cut.rs` — new: CutTask operation
- `swissarmyhammer-kanban/src/task/paste.rs` — new: PasteTask operation
- `swissarmyhammer-kanban/src/task/mod.rs` — register new modules
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — rewrite CopyCmd/CutCmd/PasteCmd to delegate to operations via `run_op()`

### CopyTask
- `#[operation(verb = \"copy\", noun = \"task\")]`
- Fields: `id: TaskId`
- Execute: read entity via `ectx.read()`, serialize with swissarmyhammer_clipboard wrapper, write to system clipboard via ClipboardProvider, set `has_clipboard` flag on UIState
- Returns `ExecutionResult::Unlogged` (not a data mutation)

### CutTask
- `#[operation(verb = \"cut\", noun = \"task\")]`
- Fields: `id: TaskId`
- Execute: read entity, serialize to clipboard (mode: \"cut\"), delete via `ectx.delete()`, set `has_clipboard` flag
- Undoable (delete is transactional)
- Returns `ExecutionResult::Logged`

### PasteTask
- `#[operation(verb = \"paste\", noun = \"task\")]`
- Fields: `column: ColumnId, after_id: Option<TaskId>`
- Execute: read system clipboard via ClipboardProvider, validate JSON has swissarmyhammer_clipboard marker, deserialize fields, create new Entity with `TaskId::new()`, copy ALL fields except position/id, set position_column + compute ordinal via `compute_ordinal_for_neighbors`, validate entity, `ectx.write()`
- Undoable
- Fields to copy: title, body (with #tags), assignees, depends_on, swimlane, custom fields
- Fields to NOT copy: id, position_column, position_ordinal (set by paste logic)

### Command layer (clipboard_commands.rs)
- CopyCmd delegates to `run_op(&CopyTask { id }, &kanban)`
- CutCmd delegates to `run_op(&CutTask { id }, &kanban)`
- PasteCmd delegates to `run_op(&PasteTask { column, after_id }, &kanban)`
- Availability unchanged: copy/cut need task in scope, paste needs clipboard + column/board

## Acceptance Criteria
- [ ] Copy writes entity snapshot to system clipboard as JSON
- [ ] Cut writes to clipboard then deletes (undoable)
- [ ] Paste reads clipboard, validates, creates new task with new ULID
- [ ] Paste copies all relevant fields (title, body, tags, assignees)
- [ ] Paste computes correct ordinal (after focused task or first position)
- [ ] Clipboard persists after paste (multiple paste works)
- [ ] Operations go through KanbanOperationProcessor for transaction/logging

## Tests
- [ ] CopyTask unit test with InMemoryClipboard
- [ ] CutTask unit test: clipboard written + entity deleted
- [ ] PasteTask unit test: new entity created from clipboard with new ID
- [ ] Paste invalid clipboard returns error gracefully
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes"
<parameter name="assignees">[]