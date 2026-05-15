---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe880
title: trash_file overwrites existing trashed file without warning
---
**swissarmyhammer-store/src/trash.rs:14-22**\n\n`trash_file()` does `std::fs::rename(src, dst)`. If a file with the same item_id was previously trashed and not yet restored, the old trashed file is silently overwritten. This means:\n1. Create item A, delete it (trashed). Create item A again, delete it again -- the first trashed version is lost.\n2. The undo of the first delete can never be completed because the trashed content is gone.\n\nThis is a data loss scenario when the same item ID is created-deleted-created-deleted.\n\n**Severity: blocker**\n\n**Suggestion:** Either:\n1. Include the undo entry ID in the trash filename (e.g., `{item_id}.{entry_id}.{ext}`) so multiple trash versions coexist.\n2. Check if the destination exists and return an error.\n\n**Subtasks:**\n- [ ] Decide: versioned trash filenames or error on overwrite\n- [ ] Implement chosen strategy\n- [ ] Update restore_file to match the new naming scheme\n- [ ] Add test: double-delete same item ID preserves both trashed versions\n- [ ] Verify undo/redo still works end-to-end" #review-finding