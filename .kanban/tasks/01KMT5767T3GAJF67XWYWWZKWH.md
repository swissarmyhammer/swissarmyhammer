---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffee80
title: 'Duplicate nav command registrations: app-shell global + board/grid/inspector scopes'
---
**Severity: Low (Design/Clarity)**

Navigation commands (nav.up, nav.down, nav.left, nav.right, nav.first, nav.last) are registered as global commands in `app-shell.tsx` AND as scoped commands in `board-view.tsx` (board.firstColumn, board.lastColumn, board.inspect, board.newTask), `grid-view.tsx` (grid.moveUp/Down/Left/Right), and `inspector-focus-bridge.tsx` (inspector.moveUp/Down).

The scoped commands call `broadcastRef.current("nav.up")` etc., which is the same as the global commands. The scoped versions exist to register keybindings at scope level (e.g., vim `j`/`k` in the inspector scope), but the global versions in app-shell also register the same vim keys.

This means pressing `j` in vim mode will first resolve via the scope (if inspector/grid is focused), which calls broadcastNavCommand, and the scope binding shadows the global. This works correctly because scope bindings take precedence. But the global nav.* commands in app-shell are then unreachable when a scope is active (since the scope shadows the key), and when no scope is active they broadcast to predicates that may not be registered.

**Recommendation:** This is not a bug -- the layered scope system handles it correctly. But the intent would be clearer if the global nav commands only registered keys that don't overlap with scoped commands, or if a comment explained the intended resolution order. #review-finding