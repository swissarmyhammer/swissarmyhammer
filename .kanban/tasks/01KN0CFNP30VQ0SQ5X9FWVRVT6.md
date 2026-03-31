---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9780
title: '[Low] UndoProvider listens for wrong event name'
---
**File**: `kanban-app/ui/src/lib/undo-context.tsx` line 81\n\n**Issue**: The `UndoProvider` listens for `\"entity-changed\"` events to refresh undo state. However, the backend emits `\"entity-created\"`, `\"entity-removed\"`, and `\"entity-field-changed\"` — there is no `\"entity-changed\"` event. This means the undo state is never refreshed after entity mutations, only on mount and after explicit undo/redo calls.\n\nThe `can_undo`/`can_redo` flags will be stale until the next undo/redo action manually calls `refreshState()`.\n\n**Severity**: Low (the undo/redo buttons still work, but their enabled state may lag)\n**Layer**: Functionality/Correctness\n\n**Fix**: Listen for all three entity events, or better, emit a dedicated `\"undo-state-changed\"` event from the backend after any push to the undo stack."