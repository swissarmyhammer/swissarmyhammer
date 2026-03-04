---
position_column: done
position_ordinal: i6
title: Handle partial transaction undo failure gracefully
---
**Review finding: B1 (blocker)**

`swissarmyhammer-entity/src/context.rs` — `undo_transaction()`

When a transaction undo fails partway through (e.g., 3rd of 5 ops fails due to stale TextDiff), the first 2 ops are already undone. The transaction is left half-reverted with no indication of which entries were already undone.

Example: tag rename touching 3 tasks. If the 3rd task body undo fails, the tag name and 2 task bodies revert but the 3rd task is stuck with the new tag name.

## Fix approach
Return the partial-failure state in the error so callers can handle it. Include which entry ULIDs were successfully undone vs which failed. This lets the client remove the partially-undone entries from its undo stack or attempt cleanup.

- [ ] Change undo_transaction error to include list of successfully undone ULIDs
- [ ] Consider attempting re-redo of already-undone entries on failure (rollback)
- [ ] Add test for partial transaction undo failure
- [ ] Verify fix