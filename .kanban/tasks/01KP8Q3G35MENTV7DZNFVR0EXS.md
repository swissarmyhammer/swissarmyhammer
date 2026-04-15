---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
title: Fix column width bounds — raise to 24em–48em and stop cards overflowing columns
---
## What

Follow-up to the scroll-containment fix in task `01KP3PKES13ATRFMSX16WPWKTN`. Two bugs surfaced once the outer `overflow` chain was correct:

### Bug 1 — columns can shrink below their stated minimum

`kanban-app/ui/src/components/column-view.tsx:423` sets the column wrapper to:

```
flex flex-col min-h-0 min-w-[20em] max-w-[40em] flex-1
```

When the window is narrow, columns visibly shrink smaller than `20em` and bunch up. The root cause is flex's default `min-width: auto` behavior combined with the downstream `min-w-0` chain (recently added in `BoardView`'s `scrollContainerRef` div at `board-view.tsx`) — once any ancestor has `min-w-0`, the column's own `min-w-[20em]` is *allowed* to be violated because flex will shrink flex items below their min-content when the parent shrinks. The fix is to add `shrink-0` (or `flex-shrink-0`) to the column so it refuses to shrink below `min-w`; the board's horizontal scroll container is the correct surface to absorb the overflow (it already has `overflow-x-auto` and `min-w-0`).

Additionally the desired bounds are stricter than the current values:

- `min-w-[20em]` → `min-w-[24em]`
- `max-w-[40em]` → `max-w-[48em]`

### Bug 2 — cards overflow their column

`kanban-app/ui/src/components/entity-card.tsx` (the card body used inside `SortableTaskCard`) has `flex-1 min-w-0` at its top-level flex rows (lines 159, 212). But if the column's flex chain above doesn't break min-width-auto correctly at the card list level, long unbroken content (URLs, tags, long titles without spaces) lets the card grow wider than the column. The card must always size to its column.

Check the whole chain from `column-view.tsx:425` (`flex flex-col min-h-0 min-w-0 flex-1`) down through the task-list container and through `SortableTaskCard` / `EntityCard`. Every flex descendant must have `min-w-0` so the card respects the column's width. Any element that currently has intrinsic width (e.g., from unwrapping text) must get `break-words` / `overflow-wrap: anywhere` / `min-w-0`.

### Fix

Files to change (both in `kanban-app/ui/src/components/`):

1. **`column-view.tsx:423`** — update the column `FocusScope` className from
   `flex flex-col min-h-0 min-w-[20em] max-w-[40em] flex-1`
   to
   `flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0`.
   Adding `shrink-0` stops the column from collapsing below `min-w-[24em]` when the viewport is narrower than the cumulative column strip width; the horizontal scroll container absorbs the overflow.

2. **`entity-card.tsx`** — audit every flex descendant inside the card and ensure each has `min-w-0` so no long content can push the card wider than its column. Anywhere text can be long (titles, URLs, tags), also ensure `break-words` (Tailwind: `break-words`) or `overflow-wrap: anywhere`.

3. **`sortable-task-card.tsx`** — verify the sortable wrapper does not introduce an intrinsic-width container. It currently lacks `min-w-0` — add it if anything in the component uses `flex` that could propagate min-content width upward.

4. If any task-list wrapper inside `column-view.tsx` (below line 425) wraps cards in a `flex` row/column without `min-w-0`, add it.

### Non-goals

- Do NOT change the per-column vertical `overflow-y-auto` at `column-view.tsx:545`.
- Do NOT touch `App.tsx`'s `overflow-hidden`, `views-container.tsx`'s `min-w-0`, `perspectives-container.tsx`'s `min-w-0`, or the `scrollContainerRef` div's `min-w-0 overflow-x-auto` in `board-view.tsx` — those are from the just-completed scroll-containment fix and must remain intact.
- Do not change card visuals beyond min-width-auto chain + break-words.

## Subtasks

- [ ] Write failing tests first (see Tests section).
- [ ] Update `column-view.tsx:423` className to `flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0`.
- [ ] Audit `entity-card.tsx` and `sortable-task-card.tsx` flex chains and add `min-w-0` / `break-words` wherever an intrinsic-width path exists from card content up to the column's `min-w-0` boundary.
- [ ] Verify in a narrow viewport: columns remain at ≥24em, do not bunch, card content wraps to column width, no horizontal scroll appears inside a column.
- [ ] Verify the scroll-containment fix still works — at narrow widths the column strip scrolls horizontally within the board scroll container, nothing else moves.

## Acceptance Criteria

- [ ] In `column-view.tsx:423`, the column wrapper className is exactly `flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0`.
- [ ] With the viewport narrower than `N × 24em` (where N is the number of columns), every rendered column's bounding-rect width is ≥ `24em` (in px: `24 * parseFloat(getComputedStyle(document.documentElement).fontSize)`), and the board's `scrollContainerRef` div has `scrollWidth > clientWidth` (column strip scrolls instead of columns bunching).
- [ ] At any viewport width, every `EntityCard` inside a column has `getBoundingClientRect().width <= column.getBoundingClientRect().width` — cards never exceed their column.
- [ ] A card with a long unbreakable string (e.g. a 60-char URL) renders the string wrapped/broken within the card; no card ever reports `scrollWidth > clientWidth`.
- [ ] `cd kanban-app/ui && npm run typecheck` passes.
- [ ] All existing tests in `app-layout.test.tsx`, `board-view.test.tsx`, `column-view.test.tsx` (if it exists), `entity-card.test.tsx` still pass unchanged.

## Tests

- [ ] Extend `kanban-app/ui/src/components/app-layout.test.tsx` — add a test that mounts `<App />` with 6 columns inside a 800px-wide probe (800px ≪ 6 × 24em). Assert:
  - Each column's `getBoundingClientRect().width` is ≥ `24em` in px.
  - The board's `scrollContainerRef` div has `scrollWidth > clientWidth`.
- [ ] New test file: `kanban-app/ui/src/components/card-column-fit.test.tsx` — render a single column containing a card with a long unbreakable string (a 60-char URL). Assert `card.scrollWidth <= card.clientWidth` (the card never has internal horizontal overflow).
- [ ] Test command: `cd kanban-app/ui && npm test -- app-layout card-column-fit column-view board-view` — all green.
- [ ] Full check: `cd kanban-app/ui && npm run typecheck && npm test` — all green.
- [ ] Manual verification: `cd kanban-app && cargo tauri dev`, resize the window narrow with several columns present — columns should stay at ≥24em and the column strip should scroll horizontally; cards should always fit their column regardless of content.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.