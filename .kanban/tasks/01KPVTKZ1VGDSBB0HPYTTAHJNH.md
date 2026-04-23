---
assignees:
- claude-code
position_column: todo
position_ordinal: e180
project: spatial-nav
title: 'Board virtualized cards: stale spatial rects — ResizeObserver doesn''t fire on translateY scroll, so Rust has wrong coordinates'
---
## What

On the board, spatial nav reaches some cards but not others, while click-to-focus works on every card. This is because the column's virtualizer (`@tanstack/react-virtual` in `kanban-app/ui/src/components/column-view.tsx`) positions cards via `transform: translateY(${startPx}px)` — and **`ResizeObserver` does not fire on `transform` changes**. The rect each card's `FocusScope` registered with Rust via `spatial_register` is the rect at the moment of first measurement. When the column scrolls, cards visually move (transform updates) but their registered rects in Rust stay stuck at stale coordinates. Click-to-focus works because React reads the current DOM at click time and calls `setFocus(moniker)` — it doesn't need the stored rect. Nav keys use the stored rects exclusively — so they can't reach the right neighbors.

Why the grid doesn't have this bug: `kanban-app/ui/src/components/data-table.tsx` uses no virtualizer. Rows are in natural DOM flow inside an `overflow: scroll` container. Scrolling changes scroll offset on the container, not transforms on children. `getBoundingClientRect()` returns accurate viewport-relative coordinates even without a ResizeObserver trigger, because the observed size doesn't change — but the observer's registered reports are tied to layout changes that DO fire. In practice grid rows stay accurate through scrolling because their positions relative to the viewport change in sync with the parent's scroll, not via transforms.

### Root cause in detail

`kanban-app/ui/src/components/column-view.tsx:576-582`:

```tsx
function virtualRowStyle(startPx: number): React.CSSProperties {
  return {
    position: "absolute",
    top: 0,
    left: 0,
    width: "100%",
    transform: `translateY(${startPx}px)`,
  };
}
```

Each `VirtualRowItem` (line 728+) wraps a card via `ref={measureElement}` — the virtualizer's own measurement ref — but inside the card, the `FocusScope` uses `useRectObserver` in `kanban-app/ui/src/components/focus-scope.tsx:127-162`:

```tsx
useEffect(() => {
  // ...
  const report = () => {
    const r = el.getBoundingClientRect();
    invoke("spatial_register", { args: { key, moniker, x: r.x, y: r.y, w: r.width, h: r.height, ... } });
  };
  report();
  const observer = new ResizeObserver(report);
  observer.observe(el);
  return () => observer.disconnect();
}, [...]);
```

`ResizeObserver` fires only when the observed element's *size* changes. A `transform: translateY` change is a *position* change — observer never fires. `getBoundingClientRect()` would report the correct current position if called — but it only gets called from `report()`, which only runs on observer callbacks.

Further: when a card scrolls far enough out of view, the virtualizer unmounts the `<VirtualRowItem>`. Its `FocusScope` cleanup fires `spatial_unregister`. That card disappears from Rust's entries entirely, so even "reach by pressing j repeatedly" can't work — the target doesn't exist in the candidate pool.

### The fix — report rects on scroll

Two cooperating changes:

1. **Add a scroll listener to re-report rects for every mounted FocusScope when its scroll container scrolls.** The simplest mechanism: have `useRectObserver` also observe its nearest scrollable ancestor's `scroll` event (throttled with `requestAnimationFrame`). On scroll → call `report()` → push fresh rect to Rust.

   Alternative: attach a single scroll listener at the column-view level that calls all child scopes' report functions. More plumbing, marginal perf gain; start with the first approach.

2. **Decide what to do with off-screen (unmounted) cards.** Two options:
   - **(a)** Accept that only mounted cards are nav targets. When pressing j past the last visible card, Rust's navigate returns None. React's `focus-changed` listener (or a new command handler) reacts by scrolling the column so the next card mounts, then re-attempts the nav. Auto-scroll on nav-at-edge is a common pattern.
   - **(b)** Pre-register placeholder rects for all cards in the column whether mounted or not. Requires knowing the virtualizer's full layout ahead of time. More complex and couples spatial state to virtualizer internals.
   
   **Prefer (a).** Scope the first iteration of this task to just the stale-rect bug (mounted cards have wrong coords). Auto-scroll-on-edge is a separate task.

### Files to modify

- `kanban-app/ui/src/components/focus-scope.tsx` — extend `useRectObserver` to also listen to `scroll` events on the nearest scrollable ancestor. The ancestor is found by walking up from `elementRef.current` checking `getComputedStyle(el).overflow` against `auto|scroll|overlay`. Attach a passive scroll listener that schedules a `requestAnimationFrame` to call `report()` (RAF throttling prevents flooding `invoke` on fast scrolls).
- `kanban-app/ui/src/components/column-view.tsx` — no change needed if the above is self-sufficient. If scroll-ancestor detection proves flaky, the alternative is to expose the `scrollRef` via context so FocusScopes inside the virtualizer can subscribe directly.

### Files NOT to modify

- `swissarmyhammer-spatial-nav/src/` — the Rust algorithm is correct. The bug is frontend rect reporting.
- `data-table.tsx` — working as intended, non-virtualized.

### Relationship to the perspective-tab task `01KPVT95H4FTCC5Q4E7G644CHD`

The perspective tab bug could share the same root cause IF the tab bar is inside a scroll container that the tabs' rects don't track. Worth checking in passing — if the tabs' rects are reported accurately on initial mount (tabs don't scroll), the tab bug has a different cause and this task doesn't fix it. Don't merge the two tasks; they likely have independent fixes.

## Acceptance Criteria

- [ ] Scroll a board column down by several cards, then press `j` from a visible card — focus moves to the next visible card below, correctly
- [ ] Scroll back up, verify nav still works in the original layout
- [ ] In the macOS unified log, after scrolling, a `spatial_register` trace fires for each currently-mounted card's moniker — confirms rects are being re-reported
- [ ] `__spatial_dump` after a scroll shows each mounted card's `rect.y` matches its actual on-screen position (to within 1px)
- [ ] RAF throttling prevents `spatial_register` from being invoked more than ~60 times per second per scope during a fast scroll (check log output: consecutive registers for the same moniker are at least ~16ms apart)
- [ ] `h`/`l` across columns still works at every scroll position — nav from a scrolled card lands on an appropriately-positioned card in the adjacent column, not a stale rect's phantom
- [ ] The grid (non-virtualized) is unchanged — existing data-table nav tests still pass
- [ ] All existing tests green

## Tests

- [ ] Add a vitest-browser test in `kanban-app/ui/src/test/spatial-nav-board.test.tsx` that:
  - Mounts a board column with >20 cards (more than the visible viewport)
  - Captures the initial `spatial_register` invoke count
  - Simulates a scroll event on the column's scroll container
  - Asserts `spatial_register` is re-invoked for every mounted card's moniker with updated `y` coordinates
  - The test fails on the current broken code (ResizeObserver-only) and passes after the scroll-listener fix
- [ ] Add a unit test for `useRectObserver` (or whatever helper handles scroll-ancestor detection) that verifies a scroll event on the nearest scrollable ancestor triggers a `report()` call
- [ ] Run `cd kanban-app/ui && npm test` — all green

## Workflow

- Use `/tdd`. Write the failing scroll-re-report test first.
- Measure scroll-listener performance with a console.warn during development. If flood of `spatial_register` invokes is visible in the OS log during fast scroll, the RAF throttle isn't right — fix before committing.
- Remove any temporary instrumentation before closing.
- Do not expand scope to auto-scroll-on-nav-at-edge. That's a follow-up task; note it in a comment but don't implement.

