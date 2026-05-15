---
position_column: done
position_ordinal: ffff8880
title: '[BLOCKER] undo_entry for Create writes compensating Delete with previous: None -- redo path broken'
---
In swissarmyhammer-views/src/changelog.rs line 117-118, when undoing a Create operation, the compensating Delete entry is logged with `previous: None, current: None`. This means if you later try to undo the undo (which would be a redo of the original create), the Delete entry has no `previous` snapshot to restore from, and the operation fails with `NothingToUndo`. The compensating entry should store the created view def as `previous` so it can be re-created on redo. #blocker