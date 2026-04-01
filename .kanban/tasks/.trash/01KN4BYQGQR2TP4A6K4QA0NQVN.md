---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Replace attachment entity with direct file storage
---
## What

Remove the attachment entity (YAML metadata sidecar) entirely. Attachments become plain files stored directly in `.kanban/attachments/`. The file IS the attachment — metadata is derived from the filesystem.

### Storage layout
- `.kanban/attachments/{ulid}-{sanitized_original_name}` (e.g., `01abc123-screenshot.png`)
- ULID prefix for uniqueness, original filename preserved for readability
- No `.yaml` metadata file — name, size, MIME type all derived from the file on read

### Task reference
- Task's `attachments` field stores ULIDs (unchanged)
- To resolve a ULID → file, glob `.kanban/attachments/{ulid}-*`

### Approach
1. Rewrite `AddAttachment::execute()`:
   - Validate source file exists
   - Sanitize original filename (strip path separators, limit length)
   - Copy source file to `.kanban/attachments/{ulid}-{sanitized_name}`
   - Store ULID in task's `attachments` list
   - Return derived metadata in response JSON (no entity write)

2. Rewrite `GetAttachment::execute()` / `ListAttachments::execute()`:
   - Glob for `{ulid}-*` to find the file
   - Derive name (after first `-`), size (`fs::metadata`), MIME (extension)
   - Return same JSON shape as today for API compatibility

3. Rewrite `DeleteAttachment::execute()`:
   - Glob for `{ulid}-*`, remove the file (or move to `.trash`)
   - Remove ULID from task's `attachments` list

4. Update `UpdateAttachment` — rename support only (move/rename the file)

5. Remove attachment entity definition and field definitions:
   - Delete `builtin/fields/entities/attachment.yaml`
   - Delete `builtin/fields/definitions/attachment_name.yaml`
   - Delete `builtin/fields/definitions/attachment_path.yaml`
   - Delete `builtin/fields/definitions/attachment_mime_type.yaml`
   - Delete `builtin/fields/definitions/attachment_size.yaml`
   - Remove `"attachment"` from `KNOWN_ENTITY_TYPES` in `defaults.rs`
   - Remove compute stubs for `attachment-mime-type` and `attachment-file-size`

### Files to modify
- `swissarmyhammer-kanban/src/attachment/add.rs` — file copy instead of entity write
- `swissarmyhammer-kanban/src/attachment/get.rs` — glob + derive instead of entity read
- `swissarmyhammer-kanban/src/attachment/list.rs` — glob + derive instead of entity list
- `swissarmyhammer-kanban/src/attachment/delete.rs` — file delete instead of entity delete
- `swissarmyhammer-kanban/src/attachment/update.rs` — rename file
- `swissarmyhammer-kanban/src/defaults.rs` — remove attachment from entity types and compute stubs
- `swissarmyhammer-kanban/src/task/delete.rs` — cascade delete uses file ops
- `swissarmyhammer-kanban/src/task/cut.rs` — cascade delete uses file ops

### Files to delete
- `swissarmyhammer-kanban/builtin/fields/entities/attachment.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_name.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_path.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_mime_type.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_size.yaml`

## Acceptance Criteria
- [ ] `add attachment` copies source file to `.kanban/attachments/{ulid}-{name}`
- [ ] No YAML metadata files are created for attachments
- [ ] `get attachment` / `list attachments` derive metadata from filesystem
- [ ] `delete attachment` removes the stored file
- [ ] Task delete/cut cascade removes stored attachment files
- [ ] API response JSON shape is unchanged (id, name, path, mime_type, size)
- [ ] Source file not found → clear error, no partial state

## Tests
- [ ] Test: add attachment with real temp file → file copied, metadata derived correctly
- [ ] Test: add attachment with nonexistent source → error
- [ ] Test: get attachment → returns derived name, size, MIME type
- [ ] Test: list attachments → returns all with derived metadata
- [ ] Test: delete attachment → file removed from store
- [ ] Test: delete task → cascade removes attachment files
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass