---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffe580
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
- [x] After drag-and-drop, scrolling within a column does not auto-scroll back to the focused card.
- [x] When focus *transitions* to a new card (via drop, click, or keyboard nav), the card is still scrolled into view exactly once.
- [x] Re-rendering a `SpatialFocusScopeBody` while `isDirectFocus` remains `true` does NOT call `scrollIntoView` again.
- [x] Unmounting and remounting a focused scope (e.g., virtualizer recycling) does NOT call `scrollIntoView` on remount.
- [x] Re-firing the board-level `useScrollFocusedIntoView` with the same `focusedFq` value (no actual focus change) does NOT call `scrollIntoView`.

## Tests

Use `/tdd` — write each test below as a failing regression test first, then implement the transition-guard fix.

- [x] **Unit (focus-scope)** — `kanban-app/ui/src/components/focus-scope.scroll-transition.test.tsx` (new): render a `SpatialFocusScopeBody` wired into a fake `SpatialFocusProvider`, with `Element.prototype.scrollIntoView` spied. Assert:
  - Mounting while `isDirectFocus=true` calls `scrollIntoView` once.
  - A re-render with `isDirectFocus` still `true` does NOT call `scrollIntoView` again.
  - Toggling `isDirectFocus` true→false→true calls `scrollIntoView` exactly once more (on the second `true`).
  - Unmounting and remounting the scope while still focused calls `scrollIntoView` at most once on remount AND only if focus had moved away in between (i.e., remount-while-still-the-focused-scope path does not re-scroll the user).
- [x] **Unit (board-view)** — `kanban-app/ui/src/components/board-view.scroll-focused.test.tsx` (new or extend existing): mount `useScrollFocusedIntoView` in a test host with a fake container holding multiple `[data-moniker=...]` elements. Spy `scrollIntoView`. Assert:
  - First non-null `focusedFq` → one call.
  - Same `focusedFq` value passed again (referential or value-equal) → no additional call.
  - Different `focusedFq` → one additional call.
- [x] **Browser regression** — `kanban-app/ui/src/components/column-view.drag-drop-scroll.browser.test.tsx` (new): in a virtualized column with > viewport rows, simulate a drag-drop that focuses a card mid-list, then dispatch a programmatic scroll on the column scroller. Assert the scroller's `scrollTop` after the user scroll equals the user-set value (it is NOT yanked back to the focused card's position). Use the same browser-test harness as `column-view.scroll-rects.browser.test.tsx` and `column-view.virtualized-nav.browser.test.tsx`.
- [x] Run `cd kanban-app/ui && pnpm test focus-scope.scroll-transition board-view.scroll-focused column-view.drag-drop-scroll` — all three pass.
- [x] Run the full UI suite (`cd kanban-app/ui && pnpm test`) — no regressions.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-05-14 18:30)

### Warnings
- [x] `kanban-app/ui/src/components/focus-scope.tsx:429-437` — Every visible `<FocusScope>` mounts its own `focusStore.subscribeAll` listener whose only job is to clear a module-level latch (`lastScrolledFq`). With N visible scopes, every focus move fans out into N broad-listener callbacks just to perform one `current !== lastScrolledFq` comparison. The same invariant can be enforced with a single app-level subscription, or — better — by moving the latch into `FocusStore` itself (e.g. a private `lastScrolled` slot plus a `consumeScrollLatch(fq)` API the scope effect calls) so the store owns its own invalidation. Today the cost is bounded by virtualized scope counts (~50-100) so it's not a hot-path emergency, but the listener-per-scope wiring is wasteful and duplicates state across instances that should live once.

### Nits
- [x] `kanban-app/ui/src/components/focus-scope.tsx:523-532` — The module-level `lastScrolledFq` + `__resetFocusScrollTransitionStateForTests` pair is the familiar "global mutable + test-only reset" smell. The latch is a legitimate concept ("which FQM did we last auto-scroll to?") and belongs as state on the existing `FocusStore` class in `entity-focus-context.tsx`, alongside the focus state it depends on. Putting it there removes the test-only export, removes the cross-module reset, and gives the focus-store's existing test fixtures one more thing to reset implicitly via "fresh store per provider".
- [x] `kanban-app/ui/src/components/focus-scope.tsx:422-428` — The comment "focus moved away while the scope was unmounted, then focus came back" implies the new mount in that strict-ordering case would correctly re-scroll. With the current design the latch is only cleared while a listener is alive, so unmount → focus-away → focus-back → remount leaves `lastScrolledFq` stale and the remount skips the scroll. That behavior is consistent with AC4 ("remount does not re-scroll") but the comment overstates the guarantee. Tighten the wording to describe what actually happens — the latch is cleared whenever *any* mounted scope observes focus moving off it; if no scope is mounted to observe, the latch sticks.

## Review Resolution (2026-05-14)

Consolidated all three findings via a single refactor: the focus-scroll latch now lives on `FocusStore` (private `lastScrolledFq` slot + `consumeScrollLatch(fq)` API), and the store invalidates the latch in its own `set()` path whenever focus moves to a different FQM.

**Changes:**
- `kanban-app/ui/src/lib/entity-focus-context.tsx`: added `private lastScrolledFq: string | null` field on `FocusStore`; added `consumeScrollLatch(fq): boolean` method (returns `true` and pins the latch when the caller should scroll, `false` when the latch is already pinned to `fq`); `set()` now invalidates the latch whenever the focused FQM changes.
- `kanban-app/ui/src/components/focus-scope.tsx`: deleted the module-level `lastScrolledFq` variable, the `__resetFocusScrollTransitionStateForTests` export, and the per-scope `focusStore.subscribeAll` listener. The scroll effect is now a one-shot `consumeScrollLatch(fq)` call against the store, with the per-instance `prevIsDirectFocusRef` transition guard kept for the in-component re-render case. Rewrote the surrounding comment block to accurately describe the two-layer guard (per-instance ref + store-owned latch).
- `kanban-app/ui/src/components/focus-scope.scroll-transition.test.tsx`: dropped the `__resetFocusScrollTransitionStateForTests` import and `afterEach`/`beforeEach` reset calls (fresh store per provider handles reset implicitly). Tests 4 and 5 (virtualizer-recycle and round-trip-while-unmounted) were restructured to toggle the inner `<FocusScope>` via `rerender(buildToggleTree(mounted))` while keeping a single provider stack — accurately modelling production where the virtualizer recycles rows inside one app-level `<EntityFocusProvider>`.
- `kanban-app/ui/src/components/column-view.drag-drop-scroll.browser.test.tsx`: dropped the `__resetFocusScrollTransitionStateForTests` import and `beforeEach`/`afterEach` reset calls; the fresh provider per test handles reset implicitly.

**Verification:**
- Targeted tests: `focus-scope.scroll-transition` 5/5 pass, `board-view.scroll-focused` 3/3 pass, `column-view.drag-drop-scroll.browser` 1/1 passes.
- Full UI suite: 234 test files, 2172 tests, all green (same baseline as before refactor).

## Second Root Cause — Drag-Edge rAF Loop (2026-05-14 user re-report)

After the scrollIntoView transition-guard fix shipped, the user reported the same symptom still happening. Investigation surfaced a SECOND, distinct bug in the same area:

`useColumnDragScroll` in `kanban-app/ui/src/components/column-view.tsx` runs a `requestAnimationFrame` auto-scroll loop. When the drag pointer enters the top/bottom `SCROLL_ZONE` of a column, `handleDragOver` calls `start(-1)` or `start(1)`, which sets `dirRef.current` and schedules `scrollBy({ top: dirRef.current * SCROLL_SPEED })` every frame.

**The bug**: `stop()` was only called from (1) inside `handleDragOver` when the pointer is in the middle of the column, and (2) the component unmount cleanup. When the user drops a card with the pointer still near a column edge, `dragover` stops firing — but `dirRef.current` is still `-1` or `1` and the rAF loop keeps running, calling `scrollBy` every frame and yanking the user's scroll back in whichever direction the pointer was last near.

**Fix**: `useColumnDragScroll` now subscribes to the existing global `drag-ended` event (already emitted by `useTaskDragHandlers` in `board-view.tsx:516` after every task drag completes) and calls `stop()` on receipt. It also exposes a `handleDragLeave` callback that the column scroller wires to `onDragLeave` for the case where the pointer leaves the column without a drop.

**Changes:**
- `kanban-app/ui/src/components/column-view.tsx`:
  - Added `listen` import from `@tauri-apps/api/event`.
  - Extended the `ColumnDragScroll` return type with `handleDragLeave: () => void`, plus documentation describing the two-signal guard (per-instance `dragleave` + global `drag-ended`).
  - Added a `useEffect` inside `useColumnDragScroll` that subscribes to `drag-ended` and calls `stop()` on receipt. The unlisten function is captured asynchronously and cancelled by a `cancelled` flag if the effect tears down before the promise resolves.
  - Added `handleDragLeave` as a `useCallback` that calls `stop()`.
  - Propagated `onDragLeave` through `VirtualizedCardListProps` and `VirtualColumnProps`, wiring it onto the scroll container in `EmptyColumn`, `SmallCardList`, and `VirtualColumn`.

**New test** — `kanban-app/ui/src/components/column-view.drag-edge-scroll.browser.test.tsx`:
- Spies `Element.prototype.scrollBy` to capture every call regardless of CSS layout.
- Phase 1: dispatches `dragover` events with `clientY=10` (inside `SCROLL_ZONE=40`) to start the rAF loop, asserts `scrollBy` IS called at least once after several frames.
- Phase 2: emits `drag-ended` (the production "drag complete" signal that `useTaskDragHandlers` fires after every drop).
- Phase 3: lets several rAF frames pass and asserts the `scrollBy` call count does NOT continue to grow after the drag-ended signal.

This complements the existing `column-view.drag-drop-scroll.browser.test.tsx` (which exercises the `scrollIntoView` transition-guard for the focus path) — the two tests cover the two independent root causes of the same user-visible symptom.

**Verification:**
- New test: `column-view.drag-edge-scroll.browser` 1/1 pass.
- Verified the test fails without the fix (`expected 9 to be 4`: 9 `scrollBy` calls after settle vs the 4 captured right after emit — proving the rAF loop kept firing).
- Verified the test passes with the fix (calls plateau immediately after `drag-ended`).
- Full UI suite: 235 test files, 2173 tests, all green (baseline 2172 + new test = 2173).