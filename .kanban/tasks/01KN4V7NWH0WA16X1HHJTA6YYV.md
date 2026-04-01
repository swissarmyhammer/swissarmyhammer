---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd680
title: Changelog grows unboundedly with no compaction or rotation
---
**swissarmyhammer-store/src/changelog.rs (entire file)**\n\nThe `UndoStack` has a `max_size` (default 100) that trims old entries, but the `changelog.jsonl` file never trims corresponding old entries. Over time, the changelog grows without bound while the undo stack only references the most recent 100 entries. The rest of the changelog is dead weight that slows `find_entry()` and `read_all()`.\n\n**Severity: warning**\n\n**Suggestion:** Add a `compact()` or `rotate()` method to `Changelog` that removes entries no longer referenced by the undo stack. This could be called after undo stack trimming.\n\n**Subtasks:**\n- [ ] Add `compact(referenced_ids: &HashSet<UndoEntryId>)` to Changelog\n- [ ] Call compact after UndoStack trimming in StoreContext\n- [ ] Add test for compaction\n- [ ] Verify compaction does not break undo/redo for entries still in the stack" #review-finding