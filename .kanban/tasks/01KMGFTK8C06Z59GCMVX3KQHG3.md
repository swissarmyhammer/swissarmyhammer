---
assignees:
- claude-code
depends_on:
- 01KMGFT3FY71766HMMVBTQ8DEB
position_column: todo
position_ordinal: 7c8180
title: Visual focus highlight on board cursor position
---
## What

Make the board cursor visually apparent. When navigating with keyboard, the focused column header or card must show a highlight so the user knows where they are.

### Approach

Use the existing `FocusHighlight` component (`kanban-app/ui/src/components/ui/focus-highlight.tsx`) which sets `data-focused` and auto-scrolls into view. This is the same primitive used by the inspector.

### Changes to `kanban-app/ui/src/components/column-view.tsx`

1. Add new props to `ColumnViewProps`:
   - `focusedCardIndex?: number | null` — which card in this column has keyboard focus (-1 = column header, null = not focused)
2. Wrap the column header in `<FocusHighlight focused={focusedCardIndex === -1}>` 
3. Wrap each `DraggableTaskCard` in `<FocusHighlight focused={focusedCardIndex === i}>`
4. FocusHighlight handles scroll-into-view automatically

### Changes to `kanban-app/ui/src/components/board-view.tsx`

1. Pass `focusedCardIndex` prop to each `ColumnView`:
   - If `cursor.col === colIndex`, pass `cursor.card`
   - Otherwise pass `null`
2. The focused column should also scroll into view horizontally — add a ref to the column wrapper and call `scrollIntoView({ inline: 'nearest' })` when `cursor.col` changes

### Status bar

Add a status bar below the board (like grid-view has) showing:
- Mode (NORMAL)
- Current position: `Col {n} Card {m}` or `Col {n} (header)`

### Files to modify

- **Modify**: `kanban-app/ui/src/components/column-view.tsx` — add `focusedCardIndex` prop, wrap items in FocusHighlight
- **Modify**: `kanban-app/ui/src/components/board-view.tsx` — pass focus props, add horizontal scroll-into-view, add status bar

## Acceptance Criteria

- [ ] Focused card shows `data-focused` attribute (brightness filter visible)
- [ ] Focused column header shows `data-focused` when card index is -1
- [ ] Focused card auto-scrolls into view vertically within column
- [ ] Focused column auto-scrolls into view horizontally
- [ ] Status bar shows current mode and cursor position
- [ ] Click on a card updates the board cursor to that card's position

## Tests

- [ ] `kanban-app/ui/src/components/__tests__/column-view.test.tsx` — test that FocusHighlight renders with `data-focused` when `focusedCardIndex` matches
- [ ] `pnpm vitest run` passes