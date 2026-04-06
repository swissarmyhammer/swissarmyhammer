---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc380
title: Add tests for EntityContext undo/redo cycle
---
swissarmyhammer-entity/src/context.rs\n\nCoverage: 56.8% (386/679 lines)\n\nUncovered functions (largest gaps):\n- undo / undo_single / undo_transaction / undo_update / undo_create / undo_delete (lines 492-765)\n- redo / redo_single / redo_transaction / redo_update / redo_create / redo_delete (lines 982-1256)\n- undo_archive / undo_unarchive / redo_archive / redo_unarchive (lines 782-959)\n- archive / unarchive / list_archived / read_archived (lines 1326-1493)\n- write / delete (lines 316-467)\n\nWhat to test: Create a temp EntityContext, perform write/delete/archive operations, then verify undo and redo restore previous state correctly. Test transaction grouping, archive round-trips, and changelog entries. #coverage-gap