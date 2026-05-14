---
assignees:
- claude-code
position_column: todo
position_ordinal: ff80
title: 'Bug: scrollIntoView fights user scroll after drag-and-drop in a column'
---
## What

After dragging and dropping a task card in a column, the user can briefly scroll down inside that column, but the app then auto-scrolls back up to the focused (dropped) card — fighting the user. The "follow the focus bar" scroll behavior is firing repeatedly instead of exactly once at the moment focus is set.

Two scrollIntoView sites are involved; one or both is misbehaving:

1. **`kanban-app/ui/src/components/focus-scope.tsx:405-409`** — `SpatialFocusScopeBody` runs `useEffect(() => { if (isDirectFocus && ref.current?.scrollIntoView) ref.current.scrollIntoView({ block: "nearest" }); }, [isDirectFocus]);`. `isDirectFocus = showFocus && isFocused` (line 225) where `isFocused = useOptionalIsDirectFocus(fq)`. Suspected mechanism: virtualized columns unmount/remount the focused card row as the user scrolls, so the focused-scope's `useEffect` mounts fresh with `isDirectFocus=true` and re-fires `scrollIntoView`. Alternatively, a scroll-driven re-dispatch is causing `isDirectFocus` to flip false→true, retriggering the effect.

2. **`kanban-app/ui/src/components/board-view.tsx:791-803`** — `useScrollFocusedIntoView` runs `scrollIntoView` on `[scrollContainerRef, focusedFq]`. If any code path re-sets `focusedFq` to the same logical FQM during/after scroll (or returns a new identity), this effect re-fires.

The fix is to make scrollIntoView fire only on focus *transitions* (the moment focus moves to this scope) — not on mount-while-focused, not on re-renders, not on scroll events. Plausible implementations:

- Track the previous `isDirectFocus` (and the previous focused FQM at the board level) via `useRef`, and only call `scrollIntoView` when the value transitions from `false → true` (or when `focusedFq` actually changes). A remount while still focused must NOT re-scroll.
- Alternatively, gate the scroll on a one-shot "just-focused" signal from the entity-focus store (a focus-change event the store emits exactly once per transition) rather than on derived boolean state.

Files in scope:
- `kanban-app/ui/src/components/focus-scope.tsx` — fix the effect at lines 405-409 so it only fires on a real focus transition, not on mount-while-already-focused.
- `kanban-app/ui/src/components/board-view.tsx` — fix `useScrollFocusedIntoView` (lines 791-803) so it only fires when `focusedFq` actually changes, comparing against the prior value, not on every dep-change including stable re-renders.

Investigate whether the column virtualizer is unmounting the focused card during user scroll (likely the root cause for case 1) — if so, the transition-guard fix is sufficient and no virtualizer change is needed.

## Acceptance Criteria
- [ ] After drag-and-drop, scrolling within a column does not auto-scroll back to the focused card.
- [ ] When focus *transitions* to a new card (via drop, click, or keyboard nav), the card is still scrolled into view exactly once.
- [ ] Re-rendering a `SpatialFocusScopeBody` while `isDirectFocus` remains `true` does NOT call `scrollIntoView` again.
- [ ] Unmounting and remounting a focused scope (e.g., virtualizer recycling) does NOT call `scrollIntoView` on remount.
- [ ] Re-firing the board-level `useScrollFocusedIntoView` with the same `focusedFq` value (no actual focus change) does NOT call `scrollIntoView`.

## Tests

Use `/tdd` — write each test below as a failing regression test first, then implement the transition-guard fix.

- [ ] **Unit (focus-scope)** — `kanban-app/ui/src/components/focus-scope.scroll-transition.test.tsx` (new): render a `SpatialFocusScopeBody` wired into a fake `SpatialFocusProvider`, with `Element.prototype.scrollIntoView` spied. Assert:
  - Mounting while `isDirectFocus=true` calls `scrollIntoView` once.
  - A re-render with `isDirectFocus` still `true` does NOT call `scrollIntoView` again.
  - Toggling `isDirectFocus` true→false→true calls `scrollIntoView` exactly once more (on the second `true`).
  - Unmounting and remounting the scope while still focused calls `scrollIntoView` at most once on remount AND only if focus had moved away in between (i.e., remount-while-still-the-focused-scope path does not re-scroll the user).
- [ ] **Unit (board-view)** — `kanban-app/ui/src/components/board-view.scroll-focused.test.tsx` (new or extend existing): mount `useScrollFocusedIntoView` in a test host with a fake container holding multiple `[data-moniker=...]` elements. Spy `scrollIntoView`. Assert:
  - First non-null `focusedFq` → one call.
  - Same `focusedFq` value passed again (referential or value-equal) → no additional call.
  - Different `focusedFq` → one additional call.
- [ ] **Browser regression** — `kanban-app/ui/src/components/column-view.drag-drop-scroll.browser.test.tsx` (new): in a virtualized column with > viewport rows, simulate a drag-drop that focuses a card mid-list, then dispatch a programmatic scroll on the column scroller. Assert the scroller's `scrollTop` after the user scroll equals the user-set value (it is NOT yanked back to the focused card's position). Use the same browser-test harness as `column-view.scroll-rects.browser.test.tsx` and `column-view.virtualized-nav.browser.test.tsx`.
- [ ] Run `cd kanban-app/ui && pnpm test focus-scope.scroll-transition board-view.scroll-focused column-view.drag-drop-scroll` — all three pass.
- [ ] Run the full UI suite (`cd kanban-app/ui && pnpm test`) — no regressions.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.