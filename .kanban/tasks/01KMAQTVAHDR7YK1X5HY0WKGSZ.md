---
assignees:
- claude-code
depends_on:
- 01KMAQTDF4HJJE6Y532EKXM4JK
position_column: todo
position_ordinal: '8380'
title: Inspector focus transfer — auto-focus on open, return focus on close
---
## What

When an inspector panel opens, the inspector should automatically receive keyboard focus (i.e., its command scope becomes the focused scope in the entity-focus system). When it closes, focus should return to the previous scope (typically the grid or board view).

Currently `InspectorPanel` in `App.tsx` renders `EntityInspector` inside `SlidePanel` but never claims focus. The grid's `GridFocusBridge` calls `setFocus(moniker)` — the inspector needs the same behavior.

Changes needed:

1. **InspectorFocusBridge** (from card 3) should call `setFocus()` on mount and save/restore the previous focus moniker on unmount.
2. **Focus stack**: Save `focusedMoniker` before claiming focus; restore it in the cleanup effect. This ensures the grid (or whatever was focused before) regains focus when the inspector closes.
3. **Auto-focus first field**: On mount, `useInspectorNav` should default `focusedIndex` to 0, so the first field is highlighted immediately.

### Files to modify
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — add focus claiming/restoring
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — may need a `getFocusedMoniker()` accessor if not already exposed

## Acceptance Criteria
- [ ] Opening an inspector panel automatically focuses it for keyboard navigation
- [ ] First field is highlighted when panel opens
- [ ] Closing the panel returns focus to the previously focused scope
- [ ] Multiple stacked panels: closing the top panel focuses the one below it

## Tests
- [ ] Test that opening a panel sets focus to the inspector scope
- [ ] Test that closing restores previous focus
- [ ] Run: `cd kanban-app && npx vitest run src/components/inspector-focus-bridge.test.tsx`