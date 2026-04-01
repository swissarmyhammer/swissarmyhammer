---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffff280
title: app.search (Find) occupies Edit group 0 order 0 -- will collide with undo/redo
---
In `swissarmyhammer-commands/builtin/commands/app.yaml`, `app.search` (Find) has `menu: { path: [Edit], group: 0, order: 0 }`. When undo/redo are added per the plan at group 0 order 0/1, app.search will sort alongside them with no separator. Standard macOS convention: Undo/Redo in group 0, then separator, then Cut/Copy/Paste in group 1, then separator, then Find/Select All in group 2.\n\nFix: Change app.search to `group: 2, order: 0` so it falls into its own separator group below the clipboard commands." #review-finding