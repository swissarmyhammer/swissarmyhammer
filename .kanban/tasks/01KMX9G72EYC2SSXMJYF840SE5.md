---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: CopyTag, CutTag, PasteTag kernel operations
---
## What

Add tag clipboard operations as proper kernel operations.

### Files to create/modify
- `swissarmyhammer-kanban/src/tag/copy.rs` — CopyTag operation (read tag, serialize to clipboard JSON)
- `swissarmyhammer-kanban/src/tag/cut.rs` — CutTag operation (read tag, serialize, untag from task)
- `swissarmyhammer-kanban/src/tag/paste.rs` — PasteTag operation (deserialize clipboard, tag the task)
- `swissarmyhammer-kanban/src/tag/mod.rs` — register new modules

### CopyTag
- `#[operation(verb = \"copy\", noun = \"tag\")]`
- Fields: `id: String` (tag entity ID)
- Read tag entity, serialize with entity_type: \"tag\"
- Returns ExecutionResult::Unlogged

### CutTag
- `#[operation(verb = \"cut\", noun = \"tag\")]`
- Fields: `task_id: TaskId, tag: String`
- Read tag entity for snapshot, serialize to clipboard, remove #tag from task body
- Undoable

### PasteTag
- `#[operation(verb = \"paste\", noun = \"tag\")]`
- Fields: `task_id: TaskId, clipboard_json: String`
- Validate entity_type == \"tag\", extract tag_name, append #tag to task body, auto-create tag if missing
- No-op if already tagged
- Undoable

## Acceptance Criteria
- [ ] CopyTag reads tag and returns clipboard JSON
- [ ] CutTag untags from source task + returns clipboard JSON
- [ ] PasteTag tags target task from clipboard
- [ ] PasteTag is no-op if already tagged
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Tests
- [ ] Unit tests for each operation"
<parameter name="assignees">[]