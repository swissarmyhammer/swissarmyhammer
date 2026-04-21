---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff480
project: spatial-nav
title: 'Nav bar (LeftNav): reachable from every view via left-edge nav'
---
## What

The LeftNav view-switcher strip sits on the left edge of the window. The user expects universal spatial nav: from anywhere in the window layer, pressing `h` enough times eventually lands on a LeftNav button. No view should trap focus.

Today, LeftNav renders plain `<button>` elements with no FocusScope — it's invisible to spatial nav. Also, `h` at the leftmost column of a grid / board stays put (soft clamp) instead of crossing the boundary into LeftNav.

This task is the GENERIC version — ensure LeftNav is reachable from the board, the grid, and any future view that lives in the window layer. Inspector is excluded by design (it's a layer above the window; nav is trapped — confirmed by `01KPNWPEMK`).

### Harness / infrastructure already available (2026-04-20 19:45)

The vitest-browser spatial harness from `01KPNWGFTF` is now landed and green:
- `kanban-app/ui/src/test/spatial-shim.ts` — `SpatialStateShim`
- `kanban-app/ui/src/test/setup-spatial-shim.ts` — mocks Tauri modules, synchronous emit
- `kanban-app/ui/src/test/spatial-fixture-shell.tsx` — shared FixtureShell + FixtureKeybindingHandler
- `kanban-app/ui/src/test/spatial-board-fixture.tsx`, `spatial-grid-fixture.tsx` — board / grid fixtures
- `kanban-app/ui/src/test/spatial-nav-{board,grid,inspector,canonical}.test.tsx` — precedent

This task's fixture needs to compose the LeftNav strip alongside the board/grid fixture. Suggested file: `kanban-app/ui/src/test/spatial-leftnav-fixture.tsx` that wraps the existing board/grid fixture with a real `<LeftNav />` on the left edge. Reuse `spatial-fixture-shell.tsx`.

The `FocusScopeElementRefContext` pattern and the `spatial?: boolean` prop on `FocusScope` both landed as part of `01KPNWH82X` (grid). LeftNav button wrapping can follow the same pattern.

### TDD — failing tests (one per view)

Under `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx`:

```ts
describe("LeftNav reachable from all views", () => {
  it("h from leftmost card in a board column moves focus to the active LeftNav button", async () => {
    // renderWithBoardAndLeftNavFixture()
    // click card-1-1 (leftmost column, row 0)
    // keyboard("h") — already at leftmost column
    // expect focused moniker to match /^view:/
  });

  it("h from the row selector in a grid moves focus to the active LeftNav button", async () => {
    // renderWithGridAndLeftNavFixture()
    // click rowSelectorEl(0)
    // keyboard("h")
    // expect focused moniker to match /^view:/
  });

  it("j/k between LeftNav view buttons", async () => {
    // Click the top button. Press j. Focus lands on the next view's button.
  });

  it("l from an active LeftNav button moves into the active view", async () => {
    // click viewButtonEl("board"); keyboard("l")
    // expect focused moniker to match /^(task:|tag:|field:)/
  });
});
```

### Approach

1. Wrap each view button in `LeftNav` in a `FocusScope` with `moniker("view", view.id)`.
2. Because the button is the DOM element (not a div), use `renderContainer={false}` and wire the element ref via `FocusScopeElementRefContext` — same pattern used in `data-table.tsx` for row selectors.
3. `showFocusBar={false}` — LeftNav has its own active-state styling.
4. Button's existing onClick still dispatches `view.switch:<id>` — preserved verbatim.

### Acceptance

- [x] All 4 E2E tests pass
- [x] Clicking a LeftNav button still dispatches `view.switch:<id>`
- [x] Tooltip still shows on hover
- [x] No regression in any other nav test (board, grid, inspector)
- [x] Focus highlight does not duplicate — LeftNav's `data-active="true"` styling is the primary visual; the FocusScope's focus bar stays hidden

### Implementation notes (2026-04-20)

- `kanban-app/ui/src/components/left-nav.tsx` — extracted `ViewButton` and `ViewButtonElement`. The outer `ViewButton` wraps each view in `<FocusScope moniker={moniker("view", view.id)} renderContainer={false} showFocusBar={false}>`. The inner `ViewButtonElement` is `forwardRef`-wrapped so Radix `TooltipTrigger asChild` can pass its Slot ref and so `useFocusScopeElementRef()` can attach the spatial-scope ref to the same `<button>` node via a composite ref callback. The click handler now calls `setFocus(mk)` before the existing `dispatch(\`view.switch:${view.id}\`)` — `FocusScope` with `renderContainer={false}` does not wire its own click handler, so setting focus is the consumer's responsibility (same pattern as `data-table.tsx` row selectors).
- `kanban-app/ui/src/test/spatial-leftnav-fixture.tsx` — composes `<LeftNav />` with inlined copies of the board and grid bodies (can't nest existing fixtures because each already mounts its own `EntityFocusProvider` + `FixtureShell`). Wraps the whole tree in `<TooltipProvider>` because LeftNav renders `Tooltip` around every button; production's `window-container.tsx` provides this ancestor, and the fixture must reproduce it.
- `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx` — mocks `@/lib/views-context` at the `vi.mock` level with a self-contained factory (no module-level references to avoid the hoist trap) and routes all spatial IPC through the existing shim. Asserts via `handles.focusedMoniker()` for LeftNav buttons because `showFocusBar={false}` means `data-focused` never lands on the `<button>`.
- Pre-existing test failures in `spatial-nav-perspective.test.tsx` and `perspective-tab-bar.test.tsx` (28 total) verified unrelated — they fail identically with or without this change.