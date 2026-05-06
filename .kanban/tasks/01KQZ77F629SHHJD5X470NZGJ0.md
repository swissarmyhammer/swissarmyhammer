---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: 'Fix tag color cell: single-click should open picker, double-click should not inspect'
---
## Bugs

**Bug 1: single click doesn't open the color picker.**
Root cause: in `kanban-app/ui/src/components/data-table.tsx::GridCellFocusable`, the cell wraps editing-mode children in an outer `<div className={innerClassName} onClick={...} onDoubleClick={...}>`. That extra click handler interferes with Radix `PopoverTrigger` inside `<ColorPaletteEditor>` — the swatch click bubbles to the cell's `handleCellClick`, runs `stopPropagation()` / refocus logic, and the popover never opens (or immediately closes).

**Bug 2: double-click on a color cell tries to inspect the entity instead of opening the color picker.**
Color is a leaf editor. Double-click on a color field should open the color editor (drill in), not route to `ui.inspect`. The cell's `onDoubleClick` is firing the inspect path.

## Fix

1. In `data-table.tsx::GridCellFocusable`, special-case color-palette fields:
   - When the field def's editor is `"color-palette"`, do NOT add the outer click-handling wrapper around editing-mode children.
   - When the field def's editor is `"color-palette"`, the cell's `onDoubleClick` should also open the color picker (start editing) instead of dispatching `ui.inspect`.

2. **Add tests** that pin both behaviors:
   - **Click test**: click on a color cell → color picker (Radix popover) opens.
   - **Double-click test**: double-click on a color cell → opens the color picker, does NOT dispatch `ui.inspect`.

## Acceptance Criteria
- Single click on color cell opens the Radix popover with color palette.
- Double click on color cell opens the picker and does NOT dispatch `ui.inspect`.
- New tests pass.
- `pnpm -C kanban-app/ui exec tsc --noEmit` is clean.
- Existing data-table tests stay green.

## Tests
- `kanban-app/ui/src/components/data-table.color-cell-click.spatial.test.tsx` (new)