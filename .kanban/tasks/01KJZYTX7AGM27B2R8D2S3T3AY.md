---
position_column: done
position_ordinal: fffd80
title: 'BLOCKER: Undo-of-Create logs previous:None -- redo path broken'
---
views/src/changelog.rs:117\n\nUndo-of-Create compensating Delete entry sets previous and current to None. Redoing (undo of that Delete) fails because previous is None.\n\nFix: capture view snapshot as previous before deleting.