---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: useInspectorNav hook — field cursor and mode management
---
## What

Create a `useInspectorNav` hook in `kanban-app/ui/src/hooks/use-inspector-nav.ts` modeled after the existing `useGrid` hook (`kanban-app/ui/src/hooks/use-grid.ts`).

The inspector is a vertical list of fields (not a 2D grid), so this is a simplified 1D cursor:

- **State**: `focusedIndex: number`, `mode: "normal" | "edit"`, `fieldCount: number`
- **Navigation**: `moveUp(count?)`, `moveDown(count?)`, `moveToFirst()`, `moveToLast()`
- **Mode**: `enterEdit()`, `exitEdit()`
- **Cursor**: `setFocusedIndex(index)`

The hook accepts `{ fieldCount: number }` and returns the state + control functions. Clamp index to `[0, fieldCount-1]`. Copy the clamping and memoization patterns directly from `useGrid`.

### Files to create/modify
- Create: `kanban-app/ui/src/hooks/use-inspector-nav.ts`
- Create: `kanban-app/ui/src/hooks/use-inspector-nav.test.ts`

## Acceptance Criteria
- [ ] Hook manages focusedIndex, mode (normal/edit), and fieldCount
- [ ] moveUp/moveDown clamp to valid range
- [ ] moveToFirst/moveToLast jump to boundaries
- [ ] enterEdit/exitEdit toggle mode correctly
- [ ] Return value is memoized (same pattern as useGrid)

## Tests
- [ ] `kanban-app/ui/src/hooks/use-inspector-nav.test.ts` — unit tests mirroring `use-grid.test.ts` structure
- [ ] Test navigation clamping at boundaries
- [ ] Test mode transitions
- [ ] Run: `cd kanban-app && npx vitest run src/hooks/use-inspector-nav.test.ts`