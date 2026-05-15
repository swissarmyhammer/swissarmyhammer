---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffa580
title: Fix dual-focus and drag ghost focus indicator bugs
---
## What

Two focus indicator bugs stem from having **two independent focus state sources** that are not coordinated:

1. **`FocusHighlight` in `column-view.tsx:260`** — driven by `focusedCardIndex` from `BoardNavContext`
2. **`FocusScope` → `FocusHighlight` in `entity-card.tsx:81`** — driven by `focusedMoniker` from `EntityFocusContext`, renders `[data-focused]::before` (the black sidebar via `index.css:137`)

### Bug 1: Multiple focus indicators
These two systems can show focus on different cards simultaneously because they track state independently. Clicking a card sets `focusedMoniker` via `FocusScope.handleClick`, but `focusedCardIndex` may point to a different card from keyboard nav. Result: two cards show focus bars.

**Fix**: Unify to a single source of truth. When `focusedCardIndex` changes (keyboard nav), sync it to `setFocus(moniker)`. When `focusedMoniker` changes (click), sync it back to the nav cursor. Ensure only one card ever has `data-focused` at a time.

**Files to modify**:
- `kanban-app/ui/src/components/column-view.tsx` — the `FocusHighlight` wrapper around `DraggableTaskCard` (line 260)
- `kanban-app/ui/src/components/board-view.tsx` — board-level focus/nav coordination

### Bug 2: Ghost focus bar on drag image
When dragging a focused card, `sortable-task-card.tsx:43` clones the DOM via `cloneNode(true)`. The clone inherits `data-focused`, so the `::before` pseudo-element (black sidebar) appears in the OS drag ghost image.

**Fix**: In `sortable-task-card.tsx` `handleDragStart`, remove `data-focused` from the clone before calling `setDragImage`. One line: `clone.removeAttribute("data-focused")`.

**File to modify**:
- `kanban-app/ui/src/components/sortable-task-card.tsx` — `handleDragStart` around line 43

## Acceptance Criteria
- [ ] Only one card ever shows the focus indicator (black sidebar) at a time, regardless of whether focus was set via click or keyboard navigation
- [ ] When a focused card is dragged, the OS drag ghost image does not show the focus indicator bar
- [ ] Keyboard navigation and click-to-focus remain functional and visually consistent

## Tests
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — add test: clicking card A then keyboard-navigating to card B shows focus on B only (no dual focus)
- [ ] `kanban-app/ui/src/components/sortable-task-card.test.tsx` (new) — add test: drag clone does not have `data-focused` attribute
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass