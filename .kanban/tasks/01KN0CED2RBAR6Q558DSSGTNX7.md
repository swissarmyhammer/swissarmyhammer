---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8280
title: '[Medium] UndoCmd/RedoCmd available() always returns true — menu items never disabled'
---
**File**: `swissarmyhammer-entity/src/undo_commands.rs` lines 24-26 and 61-63\n\n**Issue**: Both `UndoCmd::available()` and `RedoCmd::available()` unconditionally return `true`. The `update_menu_enabled_state` function checks `available()` to enable/disable menu items, so Undo and Redo are always enabled in the Edit menu even when the stack is empty. The command itself returns `{\"noop\": true}` gracefully, but the UX is misleading — greyed-out menu items are the standard expectation.\n\n**Severity**: Medium (UX)\n**Layer**: Functionality/Correctness\n\n**Fix**: `available()` should check whether the EntityContext extension exists and whether the stack has entries. This requires making `available()` async-aware or caching the undo state, since `UndoStack` is behind a tokio RwLock. A pragmatic fix: store `can_undo`/`can_redo` flags on UIState (already exposed to the frontend) and check them in `available()`."