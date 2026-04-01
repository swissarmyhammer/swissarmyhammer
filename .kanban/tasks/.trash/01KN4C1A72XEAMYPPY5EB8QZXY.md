---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8280'
title: Update attachments reference field and watcher for file-backed attachments
---
## What

With attachments no longer being entities, the `attachments` field definition on tasks and the file watcher need updating.

### Field definition change
`builtin/fields/definitions/attachments.yaml` currently declares `kind: reference, entity: attachment`. Since there's no attachment entity anymore, this should become `kind: list` (or similar) — a plain list of ULID strings. The display (`badge-list`) and editor (`multi-select`) may need adjustment since there's no entity to look up for display names.

### Watcher change
`kanban-app/src/watcher.rs` watches the `attachments` subdirectory. This should still work since we're still writing files there — but verify it doesn't depend on `.yaml` extension filtering.

### Command registry
`src/commands/entity_commands.rs` has `AttachmentDeleteCmd` — verify it still works or update to use the new non-entity delete path.

### Files to modify
- `swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml` — change from entity reference to plain list
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — update `AttachmentDeleteCmd` if needed
- `kanban-app/src/watcher.rs` — verify/update extension handling

## Acceptance Criteria
- [ ] `attachments` field definition no longer references the `attachment` entity type
- [ ] File watcher still detects changes in the attachments directory
- [ ] `AttachmentDeleteCmd` works with file-backed attachments
- [ ] No runtime errors from orphaned entity type references

## Tests
- [ ] Test: task with attachments serializes/deserializes correctly
- [ ] Test: file watcher picks up new attachment files (not just `.yaml`)
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] Run: `cargo test -p kanban-app` — all pass