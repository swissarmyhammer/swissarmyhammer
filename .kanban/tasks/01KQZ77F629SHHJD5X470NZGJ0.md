---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffbe80
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

## Review Findings (2026-05-09 12:30)

### Nits
- [x] `kanban-app/ui/src/components/data-table.tsx` — `isLeafEditor` was computed via a hardcoded string check (`col.field.editor === "color-palette"`). For one editor today this is acceptable, but the underlying concept ("clicking the visible cell IS the edit gesture; double-click drills in instead of inspecting") is general. If a second leaf editor lands (e.g. `icon-palette`, `boolean-toggle`), the discriminator should move onto the schema as a `FieldDef.editor` registry flag (e.g. `editorKind: "leaf"` or `editors[name].leaf === true`) so the cell's behavior is metadata-driven, matching JS_TS_REVIEW.md's "no hardcoded field logic in components". No refactor expected for this task — flagged for the next leaf-editor that lands.

  Resolved: extracted a small `isLeafEditorField(field: FieldDef): boolean` helper near `DataBodyCell` (body: `field.editor === "color-palette"`), with a docstring explaining what "leaf editor" means and noting the schema-flag migration path. The single call site at the `isLeafEditor` discriminator now routes through the helper, so promoting this to a schema-driven check (when a second leaf editor lands) is a one-line change inside the helper.

## Notes on the implementer's deviation from the task spec

The task said only "do NOT add the outer click-handling wrapper around editing-mode children" + "onDoubleClick should open the color picker instead of dispatching `ui.inspect`". The implementer correctly identified that the spec under-specified the bubble-vs-capture interaction:

- `<Field>` in display mode wraps content in `<Inspectable>` (`components/inspectable.tsx`), whose `onDoubleClick` calls `e.stopPropagation()` on the bubble phase. A bubble-phase handler on the cell never sees the gesture — Inspectable swallows it first and dispatches `ui.inspect` against the FIELD moniker (not the row).
- The implementer moved leaf-editor handlers to the **capture phase** (`onClickCapture` / `onDoubleClickCapture`) so they fire top-down BEFORE descendant Inspectable handlers, then calls `e.stopPropagation()` to prevent both the field-level `<Inspectable>` and the row-level `useInspectOnDoubleClick` from firing.
- Since `<PopoverContent>` uses `<PopoverPrimitive.Portal>` (`ui/popover.tsx`), the popover content is NOT a descendant of the cell — capture-phase handlers on the cell do not intercept clicks on the popover. Verified.
- Single-click also enters edit mode (the editor's `useState(true)` opens the popover on mount). This is required because the popover only mounts when the cell flips to edit mode — without single-click → enter-edit, the user would have to dblclick first.

The deviation is sound and well-reasoned. Capture phase + portal isolation makes the fix robust.

The new test (`data-table.color-cell-click.spatial.test.tsx`) mounts the **real** `<DataTable>`, real `<Field>`, real `<Inspectable>`, real `<ColorPaletteEditor>`, real `<Popover>`. No stubs of the click path. Both tests pin the fix end-to-end.