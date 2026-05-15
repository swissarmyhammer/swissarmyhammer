---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb380
title: 'Review finding: Race between UndoProvider entity-changed listener and undo_stack read'
---
**Severity**: Low (cosmetic flicker)\n**File**: `kanban-app/ui/src/lib/undo-context.tsx` lines 75-83, 85-93\n\nThe `UndoProvider` refreshes undo state in two places:\n1. After executing undo/redo (`await refreshState()` on line 87/92)\n2. On every `entity-changed` event (line 77)\n\nBoth paths call `fetchUndoState()` which invokes `get_undo_state`. This creates a benign race: after `app.undo` dispatches, the explicit `refreshState()` fires AND the entity-changed event listener also fires `refreshState()`. This results in two sequential `get_undo_state` Tauri IPC calls for a single undo action.\n\nThis is harmless (both will return the same result) but wasteful. A debounce on `refreshState` or deduplication via a pending-flag would eliminate the duplicate call.\n\nNote: this is only relevant once the flush_and_emit bug (other card) is fixed. Currently entity-changed events don't fire after undo/redo, so the explicit refresh is the only path that works. #review-finding