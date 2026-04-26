---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffec80
title: 'StoreContext.undo/redo: TOCTOU race between stack read and store dispatch'
---
swissarmyhammer-store/src/context.rs -- undo() and redo() methods\n\nThe undo/redo flow reads the stack target under one lock scope, releases the lock, scans stores for the matching entry, releases the stores lock, performs the undo/redo, then re-acquires the stack lock to record the pointer movement. Between the initial stack read and the final pointer update, another concurrent caller could also read the same undo target and attempt to undo the same entry.\n\nThis is not a data-corruption bug (the store-level undo is idempotent on its own), but it could produce confusing behavior: two concurrent callers both think they undid the same operation, the pointer moves back by 2 instead of 1, etc.\n\nSuggestion: Hold the stack write lock for the entire undo operation (read target + dispatch + record_undo), or use a Mutex to serialize undo/redo calls. Since undo/redo is user-initiated and infrequent, contention is not a concern. #review-finding