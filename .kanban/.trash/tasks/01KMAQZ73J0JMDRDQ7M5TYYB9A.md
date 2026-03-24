---
assignees:
- claude-code
depends_on:
- 01KMAQTVAHDR7YK1X5HY0WKGSZ
position_column: todo
position_ordinal: '8480'
title: Guard grid keydown handler against inspector focus
---
## What

The grid view (`kanban-app/ui/src/components/grid-view.tsx` lines 100-183) registers a direct `window.addEventListener("keydown")` handler that processes j/k/h/l/arrow navigation independently of the command scope system. When an inspector panel is open and focused, both the inspector's command-scope-based navigation AND the grid's direct handler will fire on the same keypress, causing the grid cursor to move while the user navigates the inspector.

**Fix**: Add a guard to the grid's keydown handler that checks whether the grid is the currently focused scope. If focus has moved to an inspector (or any other scope), the grid handler should bail out early.

Two approaches (pick simplest):

1. **Check `focusedMoniker`**: Import `useEntityFocus()` in `GridView`, read `focusedMoniker`, and in the handler check if it matches the grid's own moniker. Bail if not.
2. **Check for open inspector**: Check if `panelStack.length > 0` — but this requires threading panel state down, which is less clean.

Option 1 is preferred since the entity-focus system already tracks this.

### Files to modify
- `kanban-app/ui/src/components/grid-view.tsx` — add focused-scope guard to keydown handler

## Acceptance Criteria
- [ ] Grid keydown handler does NOT process navigation keys when an inspector is focused
- [ ] Grid keydown handler DOES process navigation when the grid is focused (no regression)
- [ ] Modifier combos (Cmd+Z etc.) still work regardless of focus (they go through the global handler)

## Tests
- [ ] Verify grid navigation still works when no inspector is open
- [ ] Verify grid does not move cursor when inspector panel is open
- [ ] Run: `cd kanban-app && npx vitest run src/components/grid-view`