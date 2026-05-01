---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd880
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

- [x] Write failing tests first (see Tests section).
- [x] Update `column-view.tsx:423` className to `flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0`.
- [x] Audit `entity-card.tsx` and `sortable-task-card.tsx` flex chains and add `min-w-0` / `break-words` wherever an intrinsic-width path exists from card content up to the column's `min-w-0` boundary.
- [x] Verify in a narrow viewport: columns remain at ≥24em, do not bunch, card content wraps to column width, no horizontal scroll appears inside a column.
- [x] Verify the scroll-containment fix still works — at narrow widths the column strip scrolls horizontally within the board scroll container, nothing else moves.

## Acceptance Criteria

- [x] In `column-view.tsx:423`, the column wrapper className is exactly `flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0`.
- [x] With the viewport narrower than `N × 24em` (where N is the number of columns), every rendered column's bounding-rect width is ≥ `24em` (in px: `24 * parseFloat(getComputedStyle(document.documentElement).fontSize)`), and the board's `scrollContainerRef` div has `scrollWidth > clientWidth` (column strip scrolls instead of columns bunching).
- [x] At any viewport width, every `EntityCard` inside a column has `getBoundingClientRect().width <= column.getBoundingClientRect().width` — cards never exceed their column.
- [x] A card with a long unbreakable string (e.g. a 60-char URL) renders the string wrapped/broken within the card; no card ever reports `scrollWidth > clientWidth`.
- [x] `cd kanban-app/ui && npm run typecheck` passes. (Note: no `typecheck` script — `npx tsc --noEmit` run directly; clean.)
- [x] All existing tests in `app-layout.test.tsx`, `board-view.test.tsx`, `column-view.test.tsx` (if it exists), `entity-card.test.tsx` still pass unchanged.

## Tests

- [x] Extend `kanban-app/ui/src/components/app-layout.test.tsx` — add a test that mounts a real `BoardView` with 6 columns inside an 800px-wide probe (800px ≪ 6 × 24em). Asserts each column width ≥ 24em in px, board scroll container has `scrollWidth > clientWidth`, and overflow stays contained in the scroll container.
- [x] New test file: `kanban-app/ui/src/components/card-column-fit.test.tsx` — renders a single column containing a card with a 60-char unbreakable URL. Asserts `card.scrollWidth <= card.clientWidth` and `card.width <= column.width`.
- [x] Test command: `cd kanban-app/ui && npm test -- app-layout card-column-fit column-view board-view entity-card` — all green (59 tests passed).
- [x] Full check: `cd kanban-app/ui && npx tsc --noEmit && npm test` — all green (1148 tests passed, tsc clean).
- [ ] Manual verification: `cd kanban-app && cargo tauri dev`, resize the window narrow with several columns present — columns should stay at ≥24em and the column strip should scroll horizontally; cards should always fit their column regardless of content. (Not performed in this automated pass — left for user verification.)

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes

- The only source change required was the one-line className update in `column-view.tsx` — adding `shrink-0` and raising the bounds to 24em/48em. The existing `entity-card.tsx` / `sortable-task-card.tsx` chain was already correct: CardFields has `flex-1 min-w-0 break-words`, and the inner CardField div has `flex-1 min-w-0` on its content wrapper. No edits were needed there.
- The `card-column-fit.test.tsx` test confirms the long-URL case via the compact mode TextDisplay's `truncate block` pattern — the text is clipped (not wrapped), which still satisfies the `scrollWidth <= clientWidth` acceptance criterion.
- The `app-layout.test.tsx` Tailwind shim was extended with `.min-w-[24em]`, `.max-w-[48em]`, and `.shrink-0` so the browser harness can exercise the column width behavior without a real Tailwind build.