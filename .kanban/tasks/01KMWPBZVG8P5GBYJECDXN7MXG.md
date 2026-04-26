---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc080
title: Add tests for UndoProvider and useUndoState hook
---
kanban-app/ui/src/lib/undo-context.tsx:45-105\n\nThe UndoProvider component and useUndoState hook have zero direct tests. They replaced the old UndoStackProvider that had tests.\n\nTest in `kanban-app/ui/src/lib/undo-context.test.tsx` (new file):\n1. undo() calls invoke('dispatch_command', { cmd: 'app.undo' })\n2. redo() calls invoke('dispatch_command', { cmd: 'app.redo' })\n3. canUndo/canRedo default to false\n4. fetchUndoState error fallback returns { can_undo: false, can_redo: false }\n\nMock @tauri-apps/api/core invoke. #coverage-gap