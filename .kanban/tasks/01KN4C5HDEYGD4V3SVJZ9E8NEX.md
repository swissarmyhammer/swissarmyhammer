---
assignees:
- claude-code
depends_on:
- 01KN4C4R94N21004BP3YNEAJSK
position_column: done
position_ordinal: ffffffffffffffffffa580
title: 'Migrate kanban attachments to use kind: attachment field type'
---
## What

With `kind: attachment` working at the entity layer, migrate the kanban attachment system to use it. This replaces the current multi-entity pattern (separate attachment entity + reference field on tasks) with a single attachment field on the task entity.

### Field definition change
Replace `builtin/fields/definitions/attachments.yaml`:
```yaml
# Before: kind: reference, entity: attachment, multiple: true
# After:
name: attachments
type:
  kind: attachment
  multiple: true
  max_bytes: 104857600
```

### Remove attachment entity and its field definitions
- Delete `builtin/fields/entities/attachment.yaml`
- Delete `builtin/fields/definitions/attachment_name.yaml`
- Delete `builtin/fields/definitions/attachment_path.yaml`
- Delete `builtin/fields/definitions/attachment_mime_type.yaml`
- Delete `builtin/fields/definitions/attachment_size.yaml`

### Simplify kanban attachment commands
The 5 operations in `src/attachment/` become thin wrappers around entity write/read:
- `AddAttachment` — set the attachment field value to the source path, let entity layer copy the file
- `GetAttachment` — read task, extract one entry from the attachment field's metadata
- `ListAttachments` — read task, return the attachment field's metadata array
- `DeleteAttachment` — read task, remove filename from the field, write task (entity layer trashes the file)
- `UpdateAttachment` — may be removable entirely (rename = delete + re-add)

### Remove stale registrations
- Remove `"attachment"` from `KNOWN_ENTITY_TYPES` in `defaults.rs`
- Remove compute stubs for `attachment-mime-type` and `attachment-file-size` in `defaults.rs`

### Cascade delete
`task/delete.rs` and `task/cut.rs` currently iterate attachment IDs and call `ectx.delete("attachment", id)`. Remove this manual cascade — entity layer now handles trashing attachment files when an entity is deleted (card #2).

### Watcher: add `is_attachment` check
The watcher's `is_entity_file()` currently filters by `.yaml`/`.yml`/`.md` extensions, so binary attachment files (`.png`, `.pdf`, etc.) are ignored. Add a parallel `is_attachment()` check that recognizes files in `.attachments/` subdirectories (any extension). These should emit events so the frontend knows when attachments change (e.g., for thumbnail previews, badge counts).

### Files to modify
- `swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml` — change to `kind: attachment`
- `swissarmyhammer-kanban/src/attachment/add.rs` — simplify to entity write
- `swissarmyhammer-kanban/src/attachment/get.rs` — simplify to entity read
- `swissarmyhammer-kanban/src/attachment/list.rs` — simplify to entity read
- `swissarmyhammer-kanban/src/attachment/delete.rs` — simplify to field update
- `swissarmyhammer-kanban/src/attachment/update.rs` — simplify or remove
- `swissarmyhammer-kanban/src/defaults.rs` — remove attachment entity type and compute stubs
- `swissarmyhammer-kanban/src/task/delete.rs` — remove manual cascade
- `swissarmyhammer-kanban/src/task/cut.rs` — remove manual cascade
- `kanban-app/src/watcher.rs` — add `is_attachment()` check, emit events for attachment file changes

### Files to delete
- `swissarmyhammer-kanban/builtin/fields/entities/attachment.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_name.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_path.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_mime_type.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_size.yaml`

## Acceptance Criteria
- [ ] `attachments` field on tasks uses `kind: attachment` instead of `kind: reference`
- [ ] No attachment entity type exists
- [ ] `add attachment` copies file via entity layer (no manual file I/O in kanban)
- [ ] `get/list attachments` derives metadata via entity layer
- [ ] `delete attachment` removes file via entity layer (trashed, not hard-deleted)
- [ ] Task delete/cut cascades attachment file cleanup through entity layer
- [ ] Watcher emits events for attachment file changes (not just `.yaml` files)
- [ ] API response JSON shape is backward-compatible

## Tests
- [ ] All existing attachment tests updated and passing
- [ ] Test: add attachment → file stored in `.attachments/`, metadata returned
- [ ] Test: delete task → attachment files trashed automatically
- [ ] Test: watcher detects attachment file creation/deletion
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] Run: `cargo test -p kanban-app` — all pass