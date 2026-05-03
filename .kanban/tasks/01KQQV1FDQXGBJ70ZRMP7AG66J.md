---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
- 01KQQTXDHP3XBHZ8G40AC4FG4D
position_column: todo
position_ordinal: d380
project: spatial-nav
title: 'Spatial-nav #5: scroll-on-edge for virtualized regions'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting, especially the "Virtualization" section.

**This component owns:** the scroll-on-edge fall-through that lets cardinal nav cross the boundary of a virtualized scroll container.

**Why it's needed:** the app uses *essential* virtualization. Off-viewport rows do not register `<FocusScope>`, so the kernel cannot find them via `geometric_pick` (component #1). When the user is on the last visible row of a virtualized list and presses Down, the kernel returns stay-put. Without this component, the user is stuck.

**Contract (restated from design):**

> When the kernel returns stay-put (`result === focusedFq`) AND the focused scope is at the edge of a scrollable ancestor in direction D AND that ancestor can scroll further in D, scroll the ancestor by one item-height in D, wait for the virtualizer to mount the next row, then re-run nav.

This rule lives in **React glue, not the Rust kernel.** The kernel doesn't know about scroll containers — those are DOM-only. The kernel returns stay-put; the React side decides what to do next.

## What

### Files to modify

- `kanban-app/ui/src/components/app-shell.tsx`:
  - In `buildNavCommands` (the four cardinal command builders that dispatch `spatial_navigate`), wrap the result handling so that when the kernel returns the focused FQM (stay-put):
    1. Find the focused scope's nearest scrollable ancestor in direction D (walk DOM ancestors, check `overflow-y` for vertical / `overflow-x` for horizontal, check `scrollHeight > clientHeight` etc.).
    2. If that ancestor can scroll further in D (compare scroll position to scroll size), scroll it by one item-height (or some sensible step — `Math.max(focused-rect-height, 64px)` is a reasonable default).
    3. Wait for the next animation frame (so the virtualizer has a chance to mount the freshly-revealed row), then re-dispatch the same nav command. This time geometric pick should find a candidate.
    4. Cap the retry depth at 1 to avoid infinite loops on weird layouts.

- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts`:
  - No code changes needed (it already updates rects on scroll), but verify the rect refresh runs synchronously enough that the re-dispatched nav sees the new rects.

- New file `kanban-app/ui/src/lib/scroll-on-edge.ts` (or similar):
  - Extract the "find scrollable ancestor in direction" + "can scroll further?" logic into a small helper. Pure function over a DOM element + Direction; testable in isolation. ~50 lines.

- `swissarmyhammer-focus/README.md`:
  - Add a "## Scrolling" section describing the rule, where it lives (React glue), and noting that the kernel itself remains scroll-unaware.

### Tests

- **Unit test in `kanban-app/ui/src/lib/scroll-on-edge.test.ts`** for the helper:
  - Given a DOM element inside a `overflow-y: auto` ancestor with content larger than viewport, `scrollableAncestorInDirection(el, "Down")` returns the ancestor.
  - When scroll position is at max, `canScrollFurther(ancestor, "Down")` returns false.
  - When scroll position is < max, returns true.
  - Walks past `overflow: visible` ancestors.
- **End-to-end browser test in `kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx`** (new file):
  - Mount a column with enough cards that virtualization kicks in (~50 cards).
  - Drive focus to the last visible card via simulated focus event.
  - Fire keydown ArrowDown.
  - Assert (a) the column scrolled (scroll position increased), (b) after one animation frame, focus moved to a card that was previously off-viewport, (c) `data-focused="true"` is on the new card.
- **End-to-end browser test for horizontal**: similar but for the column strip — Right from the rightmost card in the rightmost visible column triggers a horizontal scroll of the strip.
- **Negative test**: when the ancestor is fully scrolled to the end, the scroll-on-edge fallback does NOT fire (focus stays put genuinely). No infinite loop.
- Run `pnpm -C kanban-app/ui test scroll-on-edge column-view.virtualized-nav` and confirm green.

## Acceptance Criteria

- [ ] Pressing ArrowDown from the last visible card in a virtualized column scrolls the column to reveal the next card AND moves focus to it (one keypress, one user-visible action).
- [ ] Pressing ArrowRight at the right edge of the visible column strip scrolls the strip horizontally AND moves focus to a card in the newly-visible column.
- [ ] When the scrollable ancestor is fully scrolled in direction D, the fallback does NOT fire — focus stays put as it should at a true visual edge.
- [ ] No infinite loop: retry depth is capped at 1.
- [ ] The helper in `scroll-on-edge.ts` has unit tests covering the four cases above.
- [ ] README "## Scrolling" section documents the rule and notes the kernel remains scroll-unaware.
- [ ] `pnpm -C kanban-app/ui test` passes.

## Workflow

- Depends on **#1 (geometric cardinal pick)** because the fallback fires on `result === focusedFq`, which only the geometric algorithm produces reliably (the current cascade also produces stay-put but for different reasons that the fallback would mishandle).
- Use `/tdd`. Write the helper unit tests first, then the column-view end-to-end test (RED), then implement the helper and wire it into `buildNavCommands`.
#spatial-nav-redesign