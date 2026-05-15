---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa880
title: Entity write path accepts enriched attachment objects on round-trip
---
## What

The attachment write path (`resolve_attachment_value` in `swissarmyhammer-entity/src/context.rs`) currently only handles string values. It needs to also accept enriched metadata objects so that read → modify → write round-trips work without the caller having to de-enrich.

### Current problem
```rust
let task = ectx.read("task", id).await?;
// attachments = [{ "id": "01abc", "name": "screenshot.png", "size": 123, ... }]

// To add a second attachment, caller must reverse-engineer the stored filename
// using extract_stored_filenames() — fragile, wrong layer
task.set("attachments", json!(["01abc-screenshot.png", "/path/to/new.pdf"]));
```

### Target behavior
The write path should accept any of these value shapes per element and figure it out:

1. **Enriched metadata object** `{ "id": "01abc", "name": "screenshot.png", ... }` → extract `{id}-{name}`, verify exists in `.attachments/`, keep it
2. **Stored filename** `"01abc-screenshot.png"` → verify exists in `.attachments/`, keep it (already works)
3. **Filesystem path** `"/Users/me/file.pdf"` → copy into `.attachments/`, replace with stored filename (already works)

Mixed arrays are fine — each element resolved independently.

### Approach
In `process_attachment_field` / `resolve_attachment_value`:
- When the value is a `Value::Object`, check for `id` and `name` fields, reconstruct `{id}-{name}`, verify it exists in `.attachments/`, return the filename
- When the value is a `Value::String`, existing logic applies (check `.attachments/` first, then treat as source path)
- This makes the round-trip seamless: `read() → set() → write()` just works

### Cleanup
Once this works, `extract_stored_filenames` in `swissarmyhammer-kanban/src/attachment/add.rs` becomes unnecessary — `AddAttachment` can read the task, grab the enriched array, append the new path, and write it back directly. Remove `extract_stored_filenames` and `stored_filename_from_meta`.

### Files to modify
- `swissarmyhammer-entity/src/context.rs` — `process_attachment_field` handles `Value::Object`, `resolve_attachment_value` signature/logic
- `swissarmyhammer-kanban/src/attachment/add.rs` — remove `extract_stored_filenames`, simplify `execute()` to pass enriched objects through
- `swissarmyhammer-kanban/src/attachment/mod.rs` — remove `extract_stored_filenames` export
- `swissarmyhammer-kanban/src/attachment/delete.rs` — simplify if it uses `extract_stored_filenames`

## Acceptance Criteria
- [ ] Writing an enriched metadata object back to an attachment field preserves the attachment (no re-copy)
- [ ] Writing a mix of enriched objects and new paths works correctly
- [ ] `extract_stored_filenames` removed — no longer needed
- [ ] Read → append path → write round-trip works without any format translation by the caller

## Tests
- [ ] Test: read entity with attachment, write it back unchanged → file untouched, no error
- [ ] Test: read entity, append new path to enriched array, write → existing kept, new copied
- [ ] Test: mixed array of enriched objects + raw paths + stored filenames → all resolve correctly
- [ ] Run: `cargo test -p swissarmyhammer-entity` — all pass
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass