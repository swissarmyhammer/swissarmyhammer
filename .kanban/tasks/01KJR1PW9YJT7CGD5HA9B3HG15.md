---
title: Delete changelog entry is immediately destroyed by delete_entity_files
position:
  column: todo
  ordinal: b8
---
**File**: `swissarmyhammer-entity/src/context.rs`, lines 132-161

**What**: `EntityContext::delete()` writes a "delete" changelog entry to the `.jsonl` log file, then immediately calls `io::delete_entity_files()` which removes BOTH the data file AND the `.jsonl` log file. The delete changelog entry is lost the moment it is written.

**Why**: This is a correctness issue for auditability. The doc comment says "Logs a delete changelog entry... before removing the data file and log file" but the log is destroyed in the same operation. The KanbanContext test `test_delete_entity_creates_changelog` has a long comment acknowledging this but never actually asserts anything -- it is effectively a no-op test.

**Suggestion**: Either:
1. (Preferred) Stop deleting the `.jsonl` file in `delete()` -- let the log survive as a tombstone. Modify `io::delete_entity_files()` to accept an option, or have `delete()` call `fs::remove_file` on just the data file.
2. (Alternative) Remove the dead changelog-writing code from `delete()` since it has no observable effect. Document that deletion is not auditable at the entity level.

Checklist:
- [ ] Decide on approach (tombstone log vs. remove dead code)
- [ ] Implement the fix in `EntityContext::delete()` or `io::delete_entity_files()`
- [ ] Update `test_delete_entity_creates_changelog` to actually assert the changelog survives (if going with approach 1)
- [ ] Verify with `cargo nextest run --package swissarmyhammer-entity` #warning