---
position_column: done
position_ordinal: h9
title: Add entity_type and entity_id to ChangeEntry, return ULID from write
---
ChangeEntry already has a ULID `id` field, but it doesn't record WHICH entity it belongs to. Given just a ULID, there's no way to find the right changelog file without scanning all of them. Also, `EntityContext::write()` returns `Result<()>` — the caller never gets the operation ULID back.

## Changes

### changelog.rs
- Add `entity_type: String` and `entity_id: String` fields to `ChangeEntry`
- Update `ChangeEntry::new()` to accept entity_type and entity_id
- All existing tests that construct ChangeEntry need updating

### context.rs
- Change `EntityContext::write()` return type from `Result<()>` to `Result<Option<String>>` — returns the ChangeEntry ULID when changes were logged, None when entity was unchanged
- Change `EntityContext::delete()` return type similarly — returns the ChangeEntry ULID
- Pass entity_type and entity_id when constructing ChangeEntry in both write() and delete()
- Add `RwLock<HashMap<String, (String, String)>>` as in-memory index mapping ChangeEntry ULID → (entity_type, entity_id). Populated during write/delete. Used by undo/redo to locate the right changelog file.

### read_changelog fallback
- `read_changelog()` should check the live path first, then fall back to the trash directory. This way undo of delete can find the changelog without special-casing.

## Checklist
- [ ] Add entity_type and entity_id to ChangeEntry struct
- [ ] Update ChangeEntry::new() signature
- [ ] Add in-memory ULID → (entity_type, entity_id) index to EntityContext
- [ ] Populate index on write() and delete()
- [ ] Update EntityContext::write() to return operation ULID
- [ ] Update EntityContext::delete() to return operation ULID
- [ ] Update read_changelog() to fall back to trash directory
- [ ] Update all existing tests in changelog.rs
- [ ] Update all existing tests in context.rs
- [ ] Verify kanban processor tests still pass (they construct ChangeEntry indirectly)
- [ ] Run full test suite