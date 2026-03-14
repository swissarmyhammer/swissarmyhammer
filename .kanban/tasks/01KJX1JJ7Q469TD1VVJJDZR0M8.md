---
position_column: done
position_ordinal: b6
title: Client-side undo stack with Command objects (do/undo/redo)
---
Phase 1 deliverable from app-architecture.md.

The client-side undo stack holds Command objects — not just IDs. Each command has do/undo/redo methods. Some commands are pure client-side (view switch), some dispatch to the Tauri backend and hold the operation ULID for backend undo/redo.

## What to build

### Command interface
```typescript
interface Command {
  do(): Promise<void>     // execute, may call backend, captures result state
  undo(): Promise<void>   // reverse — may call backend undo(ulid) or flip UI state
  redo(): Promise<void>   // re-apply — may call backend redo(ulid) or flip UI state
  label: string           // for display: "Set Status → Done"
}
```

### Undo stack
- Bounded array (~100 entries), old entries fall off bottom
- Pointer separates undo side from redo side
- `do()` pushes command, clears redo side
- `undo()` pops from undo side, moves to redo side
- `redo()` pops from redo side, moves to undo side

### Backend-bound commands
```typescript
class SetFieldCommand implements Command {
  private operationId?: string  // filled after do()
  async do() { result = await invoke("update_entity_field", ...); this.operationId = result.operation_id }
  async undo() { await invoke("undo_operation", { id: this.operationId }) }
  async redo() { await invoke("redo_operation", { id: this.operationId }) }
}
```

### Client-only commands (future — view switching etc)
```typescript
class SwitchViewCommand implements Command {
  async do() { this.setActiveView(this.to) }
  async undo() { this.setActiveView(this.from) }
  async redo() { this.setActiveView(this.to) }
}
```

### React integration
- `useUndoStack()` hook — exposes undo(), redo(), canUndo, canRedo
- app.undo and app.redo global commands dispatch through this hook

## Files
- `ui/src/lib/undo-stack.ts` — UndoStack class, Command interface
- `ui/src/lib/commands/set-field-command.ts` — backend-bound command
- `ui/src/lib/undo-context.tsx` — React context + useUndoStack hook
- Tests

## Checklist
- [ ] Command interface (do/undo/redo/label)
- [ ] UndoStack class (bounded, pointer-based)
- [ ] SetFieldCommand (backend-bound, stores operation_id)
- [ ] useUndoStack() React hook
- [ ] Wire app.undo and app.redo global commands to the stack
- [ ] Tests for stack operations (push, undo, redo, clear redo on new do, bounded size)
- [ ] Run test suite