---
assignees:
- claude-code
depends_on:
- 01KMTJ0RPCMGC0TYASH3861JGZ
position_column: done
position_ordinal: ffffffffffffffff9980
title: 'Frontend: remove UndoStack entirely, dispatch to backend'
---
## What

Remove ALL client-side undo state. The frontend becomes a pure passthrough — it sends `app.undo`/`app.redo` commands to the backend and queries `get_undo_state` for can_undo/can_redo. Zero undo logic in TypeScript.

**Delete entirely:**
- `kanban-app/ui/src/lib/undo-stack.ts` — the UndoStack class
- `kanban-app/ui/src/lib/undo-stack.test.ts` — its tests

**Rewrite `kanban-app/ui/src/lib/undo-context.tsx`:**
- Remove `UndoStack` import and ref
- Remove `push()` from the context interface — nothing pushes from the frontend
- `undo()` → `invoke('dispatch_command', { cmd: 'app.undo' })`
- `redo()` → `invoke('dispatch_command', { cmd: 'app.redo' })`
- `canUndo`/`canRedo` → fetched from `invoke('get_undo_state')`, refreshed on every `entity-changed` Tauri event
- Export `useUndoState()` hook (renamed from `useUndoStack`) with `{ undo, redo, canUndo, canRedo }`

**Update consumers:**
- `kanban-app/ui/src/App.tsx` — update provider (may simplify, no more UndoStackProvider wrapping)
- `kanban-app/ui/src/components/app-shell.test.tsx` — remove UndoStackProvider from test wrappers
- Any other file importing from `undo-stack` or `undo-context`

## Acceptance Criteria
- [ ] Zero undo logic in TypeScript — no UndoStack class, no pointer, no entries array
- [ ] `undo-stack.ts` and `undo-stack.test.ts` deleted
- [ ] Frontend undo/redo calls dispatch to Rust backend
- [ ] `canUndo`/`canRedo` reflect backend state
- [ ] All undo/redo is fully testable from Rust without UI

## Tests
- [ ] `pnpm test` passes with no undo-stack references
- [ ] Grep for `UndoStack` in `ui/src/` returns zero hits (only `useUndoState` remains)