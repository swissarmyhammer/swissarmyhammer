---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
title: Fix column header focus bar position — align with title text, not column edge
---
## What

When a column header is focused (cursor at card=-1), the focus indicator bar renders at `left: -0.5rem` relative to the `FocusHighlight` wrapper. Because the wrapper has `px-3` padding, the bar appears at the far left edge of the column — visually detached from the title and count badge.

Cards have a CSS override (`.entity-card-focus[data-focused]::before { left: 1.75rem }`) that shifts the bar inward to align with field icons. Column headers need an equivalent override to position the bar next to the title text.

### Root cause

The `FocusHighlight` in `column-view.tsx` (line 271) wraps the header with `className="px-3 py-2 flex items-center gap-2 rounded"`. The global `[data-focused]::before` rule positions the bar at `left: -0.5rem`, which is outside the padding box. No column-header-specific CSS class overrides this.

### Fix

1. **Add a CSS class** for column header focus in `kanban-app/ui/src/index.css`:
   ```css
   .column-header-focus[data-focused]::before {
     left: 0.5rem;
   }
   ```
   This positions the bar just inside the `px-3` (0.75rem) padding, aligned with the start of the title text.

2. **Add the class** to the header `FocusHighlight` in `kanban-app/ui/src/components/column-view.tsx`:
   ```tsx
   <FocusHighlight
     focused={focusedCardIndex === -1}
     className="column-header-focus px-3 py-2 flex items-center gap-2 rounded"
   ```

### Files to modify

- **Modify**: `kanban-app/ui/src/index.css` — add `.column-header-focus[data-focused]::before` rule
- **Modify**: `kanban-app/ui/src/components/column-view.tsx` — add `column-header-focus` class to header FocusHighlight

## Acceptance Criteria

- [ ] Column header focus bar appears aligned with the title text, not at the column edge
- [ ] Card focus bars are unaffected (still use `.entity-card-focus` override)
- [ ] Focus bar still visible on the first column (not clipped by overflow)

## Tests

- [ ] Visual verification — navigate to column header with `k` from card 0, bar should be next to the title
- [ ] `pnpm vitest run` passes (no rendering regressions)