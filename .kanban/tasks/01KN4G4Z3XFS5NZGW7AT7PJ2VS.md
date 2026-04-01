---
assignees:
- claude-code
depends_on:
- 01KN4G3TDD6J4HR9FW33N15SRD
position_column: done
position_ordinal: ffffffffffffffffb480
title: Remove kanban attachment commands — attachments are just entity fields
---
## What

With attachment as a first-class field type and the entity layer handling the full lifecycle (copy on write, enrich on read, trash on delete, round-trip objects), the kanban attachment commands are redundant. Adding an attachment is just setting a field. Getting attachments is just reading the entity. Deleting is just removing from the array and writing.

Remove the entire `src/attachment/` module and its dispatch/schema/operation registrations. The MCP `add attachment` / `get attachment` / etc. operations go away — callers use `update task` to set the `attachments` field directly.

### What to remove
- `swissarmyhammer-kanban/src/attachment/` — entire module (add.rs, get.rs, list.rs, delete.rs, update.rs, mod.rs)
- `swissarmyhammer-kanban/src/dispatch.rs` — remove attachment verb/noun dispatch arms (lines ~370-408)
- `swissarmyhammer-kanban/src/schema.rs` — remove attachment operations from `KANBAN_OPERATIONS`
- `swissarmyhammer-kanban/src/types/operation.rs` — remove `Noun::Attachment` / `Noun::Attachments` variants and their `is_valid_operation` entries
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — remove `AttachmentDeleteCmd`
- `swissarmyhammer-kanban/src/commands/mod.rs` — remove attachment command registration

### What to keep
- The `attachments` field definition (`builtin/fields/definitions/attachments.yaml`) — it's a field on tasks, stays
- Entity layer attachment handling — that's the real implementation now
- The watcher `is_attachment()` — still needed for file change events

### MCP API migration
Callers that previously used:
- `add attachment { task_id, name, path }` → use `update task { id, attachments: [...existing, "/path/to/file"] }`
- `list attachments { task_id }` → use `get task { id }` and read the `attachments` field
- `get attachment { task_id, id }` → use `get task { id }` and find by attachment id
- `delete attachment { task_id, id }` → use `update task { id, attachments: [...without_deleted] }`

### Files to delete
- `swissarmyhammer-kanban/src/attachment/add.rs`
- `swissarmyhammer-kanban/src/attachment/get.rs`
- `swissarmyhammer-kanban/src/attachment/list.rs`
- `swissarmyhammer-kanban/src/attachment/delete.rs`
- `swissarmyhammer-kanban/src/attachment/update.rs`
- `swissarmyhammer-kanban/src/attachment/mod.rs`

### Files to modify
- `swissarmyhammer-kanban/src/lib.rs` — remove `pub mod attachment`
- `swissarmyhammer-kanban/src/dispatch.rs` — remove attachment dispatch arms
- `swissarmyhammer-kanban/src/schema.rs` — remove attachment operations
- `swissarmyhammer-kanban/src/types/operation.rs` — remove Attachment noun variants
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — remove AttachmentDeleteCmd
- `swissarmyhammer-kanban/src/commands/mod.rs` — remove attachment command registration
- `swissarmyhammer-kanban/src/task/delete.rs` — verify no attachment references remain
- `swissarmyhammer-kanban/src/task/cut.rs` — verify no attachment references remain

## Acceptance Criteria
- [ ] `src/attachment/` module deleted entirely
- [ ] No `Noun::Attachment` or `Noun::Attachments` in operation types
- [ ] No attachment operations in MCP schema
- [ ] No attachment dispatch arms
- [ ] Attachments still work via `update task` setting the field
- [ ] Task delete still cascades attachment file cleanup (entity layer handles it)
- [ ] Compiles cleanly with no dead code warnings

## Tests
- [ ] All attachment-related tests removed (they tested the commands, which are gone)
- [ ] Test via entity layer: set attachments field on task, read back enriched metadata
- [ ] Test: delete task with attachments → files trashed by entity layer
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] Run: `cargo test --workspace` — all pass