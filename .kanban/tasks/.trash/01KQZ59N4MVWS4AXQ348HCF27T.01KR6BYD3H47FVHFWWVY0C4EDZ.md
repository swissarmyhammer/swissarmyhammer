---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: 'Fix grid view: Field cells not focusable due to nested FocusScope'
---
**Bug**: In kanban-app grid view, users cannot focus into field cells (only row-header cells work).

**Root cause**: `kanban-app/ui/src/components/data-table.tsx::GridCellFocusable` wraps each cell in `<FocusScope moniker={fmtCell(...)}>`, then renders `<Field>` inside it. `<Field>` ALSO registers its own `<FocusScope>`, nesting scope inside scope. The kernel drops the cell registration as `scope-not-leaf`, so cells never appear in the spatial registry.

**Fix**: Add a `register={false}` (or similar) prop on `<Field>` that suppresses its own `<FocusScope>` registration. Pass `false` from `GridCellFocusable` so the outer `grid_cell:R:K` scope is the only registered scope in the subtree.

**Acceptance**:
- Click a grid field cell → focus moves there (data-focused flips on the cell)
- Arrow nav between cells works
- Inspector and card field rendering of `<Field>` is unchanged (defaults preserved)
- `pnpm -C kanban-app/ui exec tsc --noEmit` clean
- Targeted tests for grid keyboard nav and grid-view spatial-nav stay green #test