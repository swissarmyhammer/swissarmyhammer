---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd580
title: 'BLOCKER: app.undo and app.redo missing menu: field in app.yaml'
---
Plan section 6 requires app.undo to have `menu: { path: [Edit], group: 0, order: 0 }` and app.redo to have `menu: { path: [Edit], group: 0, order: 1 }`. Neither command has a `menu:` field in `swissarmyhammer-commands/builtin/commands/app.yaml` (lines 18-30). Without these, Undo and Redo will NOT appear in the Edit menu. The Edit menu currently only contains `app.search` (Find). This is a critical gap -- the Edit menu is incomplete.\n\nFix: Add `menu:` blocks to app.undo and app.redo in app.yaml. Note that app.search currently occupies group 0 order 0 in Edit -- undo/redo should be group 0 order 0/1, and app.search should be bumped to a higher group (e.g., group 2) so a separator appears between undo/redo and Find." #review-finding