---
position_column: done
position_ordinal: ffec80
title: 'Shared display components: deduplicate grid CellDispatch and inspector FieldDispatch'
---
The problem: The grid view's CellDispatch and the inspector's FieldDispatch both render the same field display types but with separate, duplicated components. Both should dispatch on `field.display` and call the same underlying display components.

Current duplication:
- **badge-list**: both render TagPill lists independently (nearly identical code in CellDispatch and FieldDispatch)
- **color**: grid has ColorSwatchCell, inspector has ColorField's display portion
- **markdown**: grid falls through to TextCell (shows plain text, wrong!), inspector uses EditableMarkdown display mode
- **text/number/date/badge**: grid has dedicated cells in cells/, inspector falls through to FieldPlaceholder

Fix: Create shared display components in `fields/displays/` that both CellDispatch and FieldDispatch import. Each accepts a `mode: "compact" | "full"` prop for size differences.

Files to modify:
- `cells/cell-dispatch.tsx` — import shared displays, pass mode="compact"
- `entity-inspector.tsx` FieldDispatch read paths — import shared displays, pass mode="full"

Files to create in `fields/displays/`:
- `badge-list-display.tsx` — TagPill list (extract from both)
- `markdown-display.tsx` — compact: truncated plain text; full: EditableMarkdown display mode
- `badge-display.tsx` — colored badge from SelectOption (move from cells/badge-cell.tsx)
- `color-swatch-display.tsx` — move from cells/color-swatch-cell.tsx, add mode
- `text-display.tsx` — move from cells/text-cell.tsx
- `date-display.tsx` — move from cells/date-cell.tsx
- `number-display.tsx` — move from cells/number-cell.tsx

- [ ] Create fields/displays/ directory with shared display components
- [ ] Refactor CellDispatch to import shared displays with mode="compact"
- [ ] Refactor inspector FieldDispatch read paths to use shared displays with mode="full"
- [ ] Delete superseded cells/ display files
- [ ] Run tests