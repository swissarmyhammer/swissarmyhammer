---
position_column: todo
position_ordinal: b7
title: Extract shared field display components into fields/ directory
---
Move display components out of cells/ into fields/ so both grid and inspector can import them. Each display component gets a `mode: "compact" | "full"` prop.

What exists today:
- `cells/text-cell.tsx` — grid only, truncated
- `cells/badge-cell.tsx` — grid only
- `cells/date-cell.tsx` — grid only
- `cells/color-swatch-cell.tsx` — grid only
- `cells/number-cell.tsx` — grid only
- TagPill — already shared
- Inspector inlines: EditableMarkdown (markdown), ColorField (color), SubtaskProgress (progress), FieldPlaceholder (fallback)

Refactor:
- [ ] Move cell display components into `fields/displays/` with a mode param (compact=grid, full=inspector)
- [ ] `text-display.tsx` — compact: truncated single-line; full: full text
- [ ] `markdown-display.tsx` — compact: truncated plain text; full: ReactMarkdown+GFM (reuse EditableMarkdown's display mode)
- [ ] `badge-display.tsx` — compact: small badge; full: same (already fine)
- [ ] `badge-list-display.tsx` — TagPill list, already works both ways
- [ ] `color-swatch-display.tsx` — compact: small circle+hex; full: larger swatch
- [ ] `date-display.tsx` — same both modes, already fine
- [ ] `number-display.tsx` — same both modes, already fine
- [ ] Refactor CellDispatch to dispatch on `field.display` using new components with mode="compact"
- [ ] Refactor inspector FieldDispatch read paths to use same components with mode="full"
- [ ] Delete old cells/ display files that are now superseded
- [ ] Run tests