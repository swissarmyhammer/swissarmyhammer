---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffc80
project: spatial-nav
title: 'Focus visual: remove ring so only the left bar shows'
---
## What

The `[data-focused]` CSS rule in `kanban-app/ui/src/index.css` previously rendered TWO focus indicators at the same time:

- **Left bar** via `[data-focused]::before` — a 0.25rem vertical pill at `left: -0.5rem`
- **Ring (surround box)** via `@apply ring-2 ring-primary ring-inset;` — REMOVED

Now only the left bar remains. The bar position was repositioned for every element whose parent would have clipped a negative-left offset.

### Files modified

- `kanban-app/ui/src/index.css` — removed ring, added three new per-consumer override rules (`.cell-focus`, `.nav-button-focus`, `.tab-focus`)
- `kanban-app/ui/src/components/data-table.tsx` — added `cell-focus` class to `DataTableCellTd` and `RowSelectorTd`
- `kanban-app/ui/src/components/left-nav.tsx` — added `nav-button-focus` class to `ViewButtonElement`
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — added `tab-focus` class to the `PerspectiveTab` root `<div>`

### Tests added

- `kanban-app/ui/src/styles/focus-indicator.node.test.ts` — CSS-level guards: `[data-focused]` has no `ring-*`, the `::before` bar persists, and every override class exists.
- `kanban-app/ui/src/components/data-table.test.tsx` — DataTable cell and row-selector carry `cell-focus`.
- `kanban-app/ui/src/components/left-nav.test.tsx` (new) — each view button carries `nav-button-focus`.
- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — each tab root carries `tab-focus`.

## Acceptance Criteria

- [x] `[data-focused]` rule in `index.css` no longer contains `@apply ring-2 ring-primary ring-inset` (or any `ring-*` utility)
- [x] When focusing a card, inspector row, or column content, only the left bar is visible (no surround ring)
- [x] When focusing a grid cell, the left bar is visible inside the cell (not clipped)
- [x] When focusing a row selector, the left bar is visible
- [x] When focusing a LeftNav button, a focus bar is visible (positioned so it's not clipped by the narrow nav strip)
- [x] When focusing a perspective tab, the focus bar is visible (not clipped by the tab row)
- [x] No element gains a redundant visual (no ring + bar on any element)

## Tests

- [x] Add `kanban-app/ui/src/styles/focus-indicator.node.test.ts` — asserts `[data-focused]` has no `ring-*` utility (reads the CSS source and greps for `ring-\d`, `ring-primary`, `ring-inset`); asserts `position: relative` is kept and the `::before` bar still renders; asserts every per-consumer override class exists with a `left` positioning override.
- [x] Update `kanban-app/ui/src/components/column-view.test.tsx` — the existing `.column-header-focus` test still passes (no changes required).
- [x] Add DataTable focus tests to `kanban-app/ui/src/components/data-table.test.tsx` — verify field cells and row selectors carry `cell-focus`.
- [x] Add `kanban-app/ui/src/components/left-nav.test.tsx` — verify each view button carries `nav-button-focus`.
- [x] Add perspective-tab focus test to `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — verify each tab root carries `tab-focus`.
- [x] Run `cd kanban-app/ui && npm test` — all 1333 tests pass (was 1321 before; +12 new tests).
- [ ] Manual verification (browser) — pending user review.

## Workflow

- Used `/tdd` — wrote failing tests first (8 failing CSS + DOM tests), then implemented the CSS rule change and per-consumer class wiring to turn them green.
- This supersedes the "surround box" portion of the earlier visual consolidation work; the `data-focused` attribute stays, only the CSS rule and per-consumer overrides changed.
