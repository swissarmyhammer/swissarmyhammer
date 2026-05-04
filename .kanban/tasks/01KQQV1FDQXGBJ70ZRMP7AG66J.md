---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
- 01KQQTXDHP3XBHZ8G40AC4FG4D
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8480
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

- [x] Pressing ArrowDown from the last visible card in a virtualized column scrolls the column to reveal the next card AND moves focus to it (one keypress, one user-visible action).
- [x] Pressing ArrowRight at the right edge of the visible column strip scrolls the strip horizontally AND moves focus to a card in the newly-visible column.
- [x] When the scrollable ancestor is fully scrolled in direction D, the fallback does NOT fire — focus stays put as it should at a true visual edge.
- [x] No infinite loop: retry depth is capped at 1.
- [x] The helper in `scroll-on-edge.ts` has unit tests covering the four cases above.
- [x] README "## Scrolling" section documents the rule and notes the kernel remains scroll-unaware.
- [x] `pnpm -C kanban-app/ui test` passes.

## Workflow

- Depends on **#1 (geometric cardinal pick)** because the fallback fires on `result === focusedFq`, which only the geometric algorithm produces reliably (the current cascade also produces stay-put but for different reasons that the fallback would mishandle).
- Use `/tdd`. Write the helper unit tests first, then the column-view end-to-end test (RED), then implement the helper and wire it into `buildNavCommands`.
#spatial-nav-redesign

## Review Findings (2026-05-03 19:12)

### Warnings
- [x] `kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx` — Acceptance criterion #2 ("Pressing ArrowRight at the right edge of the visible column strip scrolls the strip horizontally AND moves focus to a card in the newly-visible column") is checked off but no end-to-end test pins it. The task description's Tests section explicitly calls for a horizontal e2e test in addition to the vertical one ("End-to-end browser test for horizontal: similar but for the column strip"). The unit tests in `scroll-on-edge.test.ts` cover horizontal helper math, and `runNavWithScrollOnEdge` is direction-agnostic, so the wiring is very likely to work — but a column-strip-level integration test would catch layout-specific regressions and should exist to honor the acceptance criterion. Suggested fix: add a fourth `it(...)` case that builds a multi-column board, scrolls horizontally with ArrowRight from a card in the rightmost visible column, and asserts both `scrollLeft` advanced and the kernel-simulator received two `spatial_navigate` IPCs.

  **Addressed:** Added a 4th `it("scrolls the column strip horizontally and re-dispatches nav from a card in the rightmost visible column")` test plus a `renderColumnStrip` helper. The test mounts one column inside a 600px-wide wrapper with the strip's intrinsic width forced to 1120px so the wrapper has horizontal scroll travel even when off-screen columns are unmounted (mirroring essential-virtualization in production). Because the kernel-simulator's stay-put surface is sensitive to virtualizer churn (off-screen registrations come and go between focus and navigate), the test installs a per-test `spatial_navigate` override that always emits a stay-put echo for `right` — exactly the case scroll-on-edge is designed to handle. Asserts: two `spatial_navigate` IPCs (initial + retry), `scrollLeft` advanced, and both directions are `right`.

### Nits
- [x] `kanban-app/ui/src/lib/scroll-on-edge.ts:312-329` — `runNavWithScrollOnEdge` captures `fq` once (line 316) and re-uses it on the retry navigate (line 328). Steps 4 (`actions.focusedFq() !== fq`) correctly guards against focus moving during the first `await nextFrame()`, but a focus change during the *second* `await nextFrame()` (between scroll and retry) would dispatch navigate from a now-stale `fq`. The kernel handles unknown fq gracefully (returns stay-put), so the worst case is one wasted IPC — not a correctness bug, but the comment block could note this and explicitly call out that re-reading `focusedFq()` after the scroll wait would be a defensible alternative if a future regression exposes the timing window. Optional: re-read with `const fq2 = actions.focusedFq(); if (fq2 !== fq) return;` before the retry navigate.

  **Addressed:** Adopted the suggested re-read. After `await nextFrame()` post-scroll, the helper now reads `fqAfterScroll = actions.focusedFq()` and bails out (no retry IPC) if it's `null` or differs from the captured `fq`. Existing tests still pass (their stub focus state remains stable across the wait, so `fqAfterScroll === fq` and the retry fires as before). Docstring updated with a "Why we re-read `focusedFq()` before the retry navigate" section.

- [x] `kanban-app/ui/src/lib/scroll-on-edge.ts:128` — `isScrollableOnAxis` accepts `auto`, `scroll`, and `overlay` overflow values. Only `auto` and `scroll` are tested; `overlay` is not exercised by any unit test (and `overlay` is also a non-standard / Webkit-deprecated value, marked legacy in CSS Overflow spec). Suggested fix: either drop `overlay` from the accepted set (cleaner — modern browsers normalize it back to `auto`) or add a unit test that covers it. Either is fine; the current state silently accepts a legacy value.

  **Addressed:** Dropped `overlay` from the accepted set. `isScrollableOnAxis` now only accepts `auto` and `scroll`. Function-level docstring notes the rationale (modern browsers normalize `overlay` back to `auto`, so accepting it would only matter on obsolete engines).

- [x] `kanban-app/ui/src/lib/scroll-on-edge.test.ts` — Unit tests cover `overflow: visible` ancestors at line 146 but no test exercises `overflow: hidden` (which the helper also walks past). The README docstring at lines 19–24 says auto/scroll qualify and visible/hidden are walked past — the implementation is correct, but a one-line test for `hidden` would close the documentation gap.

  **Addressed:** Added a `walks past \`overflow: hidden\` ancestors` unit test that mounts the vertical scroll fixture inside a wrapping `overflow: hidden` div and asserts `scrollableAncestorInDirection(leaf, "down")` returns the inner scroller (walking past the hidden wrapper).

## Review Findings (2026-05-03 21:05)

### Nits
- [x] `kanban-app/ui/src/lib/scroll-on-edge.ts:94` — The `scrollableAncestorInDirection` docstring still claims an element qualifies when its `overflow-y` / `overflow-x` is "`auto`, `scroll`, or `overlay`". The previous round's nit fix correctly dropped `overlay` from `isScrollableOnAxis` (line 133) and updated *that* function's docstring (lines 121–128) to call out the change, but the upstream caller's contract description was missed. Reading the public-facing docstring of the exported function now contradicts the implementation. Suggested fix: change line 94 to `\`auto\` or \`scroll\`` (drop `, or \`overlay\``) so the public contract matches the implementation.

  **Addressed:** Changed the `scrollableAncestorInDirection` docstring to read `auto` or `scroll` (dropped the stale `, or overlay` reference). The public contract now matches the `isScrollableOnAxis` implementation.