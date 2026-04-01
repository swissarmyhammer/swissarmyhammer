---
assignees:
- claude-code
depends_on:
- 01KN4C401J0HY3FB0GBJSMD3MJ
position_column: done
position_ordinal: ffffffffffffffffb180
title: 'Entity layer: file storage for attachment fields'
---
## What

Teach `EntityContext::write()`, `EntityContext::read()`, and `EntityContext::delete()` to handle `FieldType::Attachment` fields by copying files into the entity store, deriving metadata on read, and trashing attachment files on entity delete.

### Storage layout
When an entity has a field of `kind: attachment`, the field value in the YAML is the filename within `.attachments/`:
```yaml
# single
avatar: 01abc123-photo.jpg

# multiple
attachments:
  - 01abc123-screenshot.png
  - 01def456-spec.pdf
```

The actual files live at:
```
.kanban/{entity_type}s/.attachments/{ulid}-{sanitized_original_name}
```
e.g., `.kanban/tasks/.attachments/01abc123-screenshot.png`

The `.attachments/` subdirectory lives inside the entity type's directory, prefixed with `.` to distinguish from entity files.

### Write path (`apply_validation()`)
New infrastructure — `apply_validation` currently only branches on `Computed`. This adds a second type-specific branch for `Attachment`:

For each attachment field:
1. If the value is a **path string** pointing to a file that exists on the filesystem, treat it as a source file to attach:
   - Validate the source file exists
   - Check file size against `max_bytes` from the field definition → error if exceeded
   - Generate a ULID for this attachment
   - Sanitize the original filename
   - Copy the file to `.attachments/{ulid}-{sanitized_name}` (atomic: temp + rename)
   - Replace the field value with the filename (`{ulid}-{sanitized_name}`)
2. If the value is a string that matches a file already in `.attachments/`, leave it alone (existing attachment). **Detection**: check if `.attachments/{value}` exists — don't rely on pattern matching the ULID format.
3. For `multiple: true`, the value is an array — process each element
4. When updating: diff old vs new attachment lists. Any filenames present in old but absent in new → move the attachment file to `.attachments/.trash/`

### Read path (`apply_compute()`)
New infrastructure — `apply_compute` currently only handles `Computed`. This adds a branch for `Attachment`:

For each attachment field:
1. Resolve the stored filename to an absolute path: `{entity_dir}/.attachments/{filename}`
2. Stat the file for size via `fs::metadata`
3. Derive name (everything after first `-`), MIME type (from extension)
4. Return a rich JSON object with the resolved path for content access:
   ```json
   {
     "id": "01abc123",
     "name": "screenshot.png",
     "size": 12345,
     "mime_type": "image/png",
     "path": "/absolute/path/to/.kanban/tasks/.attachments/01abc123-screenshot.png"
   }
   ```
5. For `multiple: true`, return an array of these objects
6. The `path` field gives callers the resolved filesystem location — they read/stream/serve the bytes however they want. No base64 encoding in the JSON.

### Delete path (`EntityContext::delete()`)
New infrastructure — `delete()` currently just trashes the YAML + changelog. Add attachment awareness:

When deleting an entity, check its fields for `FieldType::Attachment`. For each attachment filename found, move the file to `.attachments/.trash/` (not hard-delete — matches entity trash semantics). This ensures entity delete cascades to attachment file cleanup without leaking blobs.

### Files to modify
- `swissarmyhammer-entity/src/context.rs` — attachment handling in `apply_validation()`, metadata derivation in `apply_compute()`, cascade trash in `delete()`
- `swissarmyhammer-entity/src/io.rs` — helper functions for attachment file I/O (copy, trash, stat, derive metadata)

## Acceptance Criteria
- [ ] Writing a filesystem path to an attachment field copies the file and stores the filename
- [ ] Writing a filename that already exists in `.attachments/` leaves it alone
- [ ] Reading an attachment field returns metadata object with absolute `path` for content access
- [ ] File size exceeding `max_bytes` → validation error before copy
- [ ] Removing a filename from the field moves the stored file to `.attachments/.trash/`
- [ ] Deleting an entity with attachment fields moves all attachment files to `.attachments/.trash/`
- [ ] Atomic file copy (temp + rename)
- [ ] `multiple: true` fields handle arrays of attachments
- [ ] Source file not found → clear error

## Tests
- [ ] Test: write entity with attachment path → file copied to `.attachments/`, field becomes `{ulid}-{name}`
- [ ] Test: write entity with existing attachment filename → file untouched, no re-copy
- [ ] Test: read entity with attachment → returns metadata object including absolute `path`
- [ ] Test: read the file at the returned `path` → content matches original source
- [ ] Test: write entity with file exceeding max_bytes → validation error
- [ ] Test: update entity removing an attachment → stored file moved to `.attachments/.trash/`
- [ ] Test: delete entity with attachments → attachment files moved to `.attachments/.trash/`
- [ ] Test: multiple attachments — add, read, remove one
- [ ] Run: `cargo test -p swissarmyhammer-entity` — all pass