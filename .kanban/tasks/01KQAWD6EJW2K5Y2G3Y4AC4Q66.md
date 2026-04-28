---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
title: 'BLOCKER: navbar is keyboard-inaccessible in production — no focus indicator, arrow keys do not traverse the bar, debug overlay also missing'
---
## What

**Release blocker.** The navbar in the running app is non-functional for keyboard users:

1. **No visible focus indicator** on any navbar leaf when keyboard focus arrives there. The user sees no cursor-bar next to the board-selector, the inspect button, the percent-complete field, or the search button — even though the kernel reports those leaves as registered (the per-test assertions in `nav-bar.focus-indicator.browser.test.tsx` pass against an isolated mount).
2. **Arrow keys do not traverse the navbar.** The user cannot Left/Right between navbar entries. They cannot Up from the board into the navbar, nor Down from the navbar into the perspective bar / board.
3. **Debug overlay missing too** (the original symptom). When `<FocusDebugProvider enabled>` is mounted at `kanban-app/ui/src/App.tsx:72`, no blue dashed border appears around the navbar zone and no emerald borders appear around the navbar leaves — even though overlays render correctly elsewhere in the same provider tree.

All three symptoms point to the **same underlying break** localised to the navbar surface. The kernel-side registration is happening (the spatial-nav test mocks confirm `spatial_register_zone` / `spatial_register_scope` are invoked at first paint), and the per-component browser-mode tests in `nav-bar.focus-indicator.browser.test.tsx` and `navbar_arrow_nav.rs` pass — but the production tree is broken. Either:

- The production provider stack differs from the isolated test harness in a way that's load-bearing (a missing context, a different layer parent, an HMR-only edge), OR
- The kernel's focus-claim subscription on the navbar leaves doesn't reach React state in production (the `useFocusClaim` callback never fires the `setFocused` it gets handed), OR
- The navbar's spatial registration silently lands on the wrong layer / parent zone in production (so beam-search filters it out and the indicator's claim subscription is keyed to nothing), OR
- A recent change broke an end-to-end seam that the unit-level tests don't exercise.

The earlier card `01KQ9XWHP2Y5H1QB5B3RJFEBBR` was marked done because its tests passed — but its tests don't reproduce the production failure mode. **That card's "regression guards" are insufficient.** This card is the production-tree fix.

## Where this lives

- Navbar surface: `kanban-app/ui/src/components/nav-bar.tsx:67-162`
  - Zone: `<FocusZone moniker="ui:navbar" showFocusBar={false} className="relative flex h-12 items-center border-b px-4 gap-2">` at line 79.
  - Scopes: `ui:navbar.board-selector` (line 85), `ui:navbar.inspect` (line 97, gated on `board`), `ui:navbar.search` (line 133, `className="ml-auto"`).
  - Field zone (peer of leaves): `<Field>` percent-complete at lines 124-132 — registered as `field:board:<id>.percent_complete`.
- Provider stack: `kanban-app/ui/src/App.tsx:69-104` — `DiagErrorBoundary` → `FocusDebugProvider enabled` → `SpatialFocusProvider` → `FocusLayer name="window"` → `CommandBusyProvider` → `RustEngineContainer` → `WindowContainer` → `AppModeContainer` → `BoardContainer` → `<div className="h-screen ...flex flex-col overflow-hidden">` → `<NavBar />`.
- Spatial primitives: `kanban-app/ui/src/components/focus-zone.tsx`, `focus-scope.tsx`, `focus-layer.tsx`, `focus-indicator.tsx`, `focus-debug-overlay.tsx`.
- Focus-claim subscription: `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `useFocusClaim(key, setFocused)` is the seam that drives `data-focused` and the `<FocusIndicator>` render. If the per-key callback never fires, the indicator never mounts.
- Existing claim that "navbar already works": `01KQ9XWHP2Y5H1QB5B3RJFEBBR` (done) and `01KQ9Z56M556DQHYMA502B9FKB` (done) — both depend on `01KQ7S6WHK9RCCG2R4FN474EFD` for the unified-policy supersession.

## Hypotheses (test in this order — root cause first)

### H1 — Production tree mounts the navbar **outside** the window-root layer

The navbar is rendered inside `BoardContainer` → `<div>` at `App.tsx:80`. `BoardContainer` is a conditional render (loading / empty / active) that may, depending on the board state, render its children inside a different inner subtree. Verify the `<NavBar>` is reached by descendants of the `<FocusLayer name="window">` push at App.tsx:74 in **every** board state. If `BoardContainer` short-circuits (e.g. an empty-state branch that renders the navbar in a fallback that is NOT a child of FocusLayer's context provider), the navbar's `useContext(FocusLayerContext)` returns `null`, the FocusZone falls through to `FallbackFocusZoneBody` (`focus-zone.tsx:613-700`), and the spatial registration is **skipped entirely** — no kernel registration, no claim subscription, no indicator, no debug overlay, no arrow nav. Tests don't catch this because they mount NavBar inside an explicit `<FocusLayer>` wrapper.

This is the most likely root cause.

### H2 — `FocusDebugProvider`'s `<div className="relative">` wrap (when debug is on) breaks an ancestor ref / context

`focus-layer.tsx:200-204` wraps the entire layer body in a `<div className="relative">` when `useFocusDebug()` returns `true`. That wrapper sits between the `FocusLayerContext.Provider` and its descendants in the DOM but **not** in React tree, so context propagation is unaffected. But: the wrapper changes the offset-parent chain, the scrollable-ancestor chain, and the React Profiler's component tree. If anything in `useTrackRectOnAncestorScroll` or the focus-claim subscription depends on a particular ancestor, the wrapper could break it. Toggle debug off in App.tsx (set `enabled={false}`) and verify the navbar focus indicator + arrow nav come back. If yes, this hypothesis is alive — narrow further.

### H3 — `useFocusClaim` callback never fires for navbar keys in production

The kernel emits `focus-changed` events; `useFocusClaim` subscribes per `SpatialKey` and calls `setFocused`. If the navbar's keys are registered but the `focus-changed` listener filter (e.g. by layer key) excludes them, the callback never fires. Verify by adding a `console.log` in the navbar `<FocusScope>` body's `useFocusClaim(key, setFocused)` site and observing whether it ever logs in the running app when the user clicks a navbar button. The isolated test mocks `listen("focus-changed", cb)` and replays events — it doesn't exercise the same end-to-end path.

### H4 — A recent change moved the navbar out of the keyboard-event-handling layer

The arrow-key router (Rust kernel + React adapter) routes keys based on the active layer. If the active layer when focus is anywhere on the board is the inspector or some other modal layer, arrows route there and the navbar is unreachable. Verify the active layer in production via DevTools (or a debug overlay toggle that prints the active layer key).

### H5 — Production-only HMR / suspense / portal artifact

Vite HMR or React 19 suspense can re-mount components in a way that orphans subscriptions. If the FocusZone for the navbar mounts before its `<FocusLayer>` finishes pushing its layer (a race the test mocks paper over by resolving `pushLayer` synchronously), the registration could land with a stale `parent` and beam-search wouldn't find it. Pin via a test that delays `pushLayer` resolution and asserts the navbar still ends up registered under the layer key.

### H6 — Original symptom (debug overlay only)

Even after H1–H5 are fixed, the debug-overlay-specific symptoms (occlusion / zero-rect / cascade override) from the original task hypotheses still need a quick eyeball check. They become irrelevant if H1–H5 already explains all three symptoms (no registration → no claim → no indicator → no overlay).

## Approach

### 0. Reproduce and bisect

1. Run `cargo tauri dev`. Open DevTools.
2. Inspect `<div data-moniker="ui:navbar">`. Confirm:
   - It exists in the DOM.
   - It carries `data-focused` toggling on click.
   - It does NOT carry `data-focused` on arrow keys (confirms arrow nav is broken).
3. In the React DevTools, find the `<FocusZone>` for `ui:navbar`. Look at its rendered branch:
   - If `SpatialFocusZoneBody` rendered → the layer context is reachable; H1 is wrong.
   - If `FallbackFocusZoneBody` rendered → **H1 is the root cause.** No spatial registration is happening at all. Fix the production tree's layer ancestry.
4. If `SpatialFocusZoneBody` rendered but the indicator doesn't show on click, attach a temporary `console.log` inside `useFocusClaim`'s subscriber for that key. Click a navbar button.
   - If the log fires AND `setFocused(true)` runs but the indicator does not visibly mount → render-side bug; check `<FocusIndicator>` is in the tree, check stacking context.
   - If the log never fires → H3 is the root cause; check the kernel's `focus-changed` filtering.
5. Toggle `<FocusDebugProvider enabled={false}>` at App.tsx:72 (temporary). Reload. Test arrow nav and click focus on the navbar.
   - If the indicator + arrow nav come back → H2 is alive; narrow to which observer the debug wrapper breaks.
   - If they're still broken → H2 is dead; the debug wrapper is innocent.

### 1. Write the failing production-tree test FIRST

Existing per-component tests pass against isolated mounts. The bug is in the production tree composition. Add a new browser-mode test that exercises the **full App tree**:

`kanban-app/ui/src/components/nav-bar.production-tree.browser.test.tsx`

Mount `<App />` (the real one, not a synthetic) inside a per-test backend, with a board open, and assert against the production composition:

- The `<div data-moniker="ui:navbar">` exists AND lives inside a `SpatialFocusZoneBody` (assert by checking that a `[data-debug="zone"]` child exists when debug is enabled, OR by attaching a probe ref through context — pick whichever is more reliable).
- After `spatial_focus(boardSelectorKey)` (called from the test), the navbar's board-selector wrapper carries `data-focused="true"` AND a `[data-testid="focus-indicator"]` descendant is rendered.
- After `nav.right` from the board-selector, the inspect leaf becomes focused (or the search leaf, per the unified cascade's same-kind iter-0 behavior — pin whichever is correct).
- After `nav.up` from the topmost card in the board, focus lands on the navbar (one of: a navbar leaf, the navbar zone, or via the unified cascade's drill-out path — pin the production-correct trajectory).

This test FAILS today. The fix makes it pass.

### 2. Fix the root cause the bisect points at

Most likely H1: the navbar mounts in a branch of `BoardContainer` that bypasses the layer context. Fix by ensuring `<NavBar>` is always a descendant of the `<FocusLayer>` in the React tree, regardless of board state. This may mean restructuring `BoardContainer` or moving the layer wrap to a tighter ancestor.

If H1 is wrong, fix the seam the bisect points at — H2 (debug wrapper), H3 (claim filter), H4 (active-layer routing), H5 (race) — each has a different fix surface.

Do **not** patch all hypotheses at once. The failing production-tree test localises the bug; fix only the failing seam.

### 3. Strengthen the regression suite

The existing per-component tests pass against the broken production tree — that's the gap. Add a guard that catches H1-class regressions (FallbackFocusZoneBody rendering in production) and one that catches H3-class regressions (focus-claim subscription not firing for the navbar keys).

`kanban-app/ui/src/components/nav-bar.guards.node.test.ts` (new file, source-level guard)

- [ ] Parse `App.tsx` and assert that the JSX path from `<App>` root to `<NavBar />` passes through `<FocusLayer name="window">` — using a static AST walk so the guard fires at lint time, not just at runtime.

`kanban-app/ui/src/components/nav-bar.production-tree.browser.test.tsx` (the new test from step 1)

- [ ] Assert SpatialFocusZoneBody (not Fallback) is the rendered branch for every navbar primitive.
- [ ] Assert focus claim end-to-end: drive `focus-changed` from the kernel and observe `data-focused` AND `[data-testid="focus-indicator"]` on the navbar leaf wrapper.
- [ ] Assert keyboard-event end-to-end: simulate `KeyboardEvent("ArrowRight")` and observe focus moves to the next navbar leaf.

### 4. Lock down the debug overlay too

Once H1 is fixed, the debug overlay should reappear on the navbar (the overlay is just another consumer of the same registration). Re-run the original task's overlay assertions and confirm. Keep them as a sub-set of the new test file.

## Acceptance Criteria

All asserted by automated tests **mounted against the real production tree**. Per-component tests are not enough — the existing per-component tests already pass against the broken production code.

- [ ] In `cargo tauri dev` on a fresh window with a board open, clicking any navbar button visibly moves focus to that button (cursor-bar appears next to it).
- [ ] In the same window, pressing ArrowRight from a focused navbar leaf moves focus to the next navbar leaf (board-selector → inspect → search per the unified cascade); ArrowLeft walks the symmetric path.
- [ ] Pressing ArrowUp from the topmost board element (a card or the perspective bar) moves focus to a navbar entry (which one depends on the unified cascade's drill-out target — pin the actual production trajectory in the test).
- [ ] Pressing ArrowDown from a navbar leaf moves focus out of the navbar (to the perspective bar or first column — pin which).
- [ ] When `<FocusDebugProvider enabled>` is mounted, the navbar shows the blue `[data-debug="zone"]` border and three emerald `[data-debug="scope"]` borders. (Subset of the original task's overlay assertions.)
- [ ] All five existing tests in `nav-bar.focus-indicator.browser.test.tsx` keep passing (no regression to the unit-level guarantees).
- [ ] All six existing tests in `swissarmyhammer-focus/tests/navbar_arrow_nav.rs` keep passing (no regression to the kernel-level guarantees).
- [ ] The new `nav-bar.production-tree.browser.test.tsx` test passes.
- [ ] The new `nav-bar.guards.node.test.ts` source-level guard passes.

## Tests

All automated. The signature of this task is "passes against the production App, not just an isolated NavBar mount."

### `kanban-app/ui/src/components/nav-bar.production-tree.browser.test.tsx` (new file)

- [ ] `navbar_renders_under_focus_layer_in_production_tree` — mount `<App />` with the per-test backend, await first paint, assert the `[data-moniker="ui:navbar"]` host has a `[data-debug="zone"]` child when debug is enabled (proves SpatialFocusZoneBody is the rendered branch). If H1 is the root cause, this test fails before the fix.
- [ ] `navbar_leaf_indicator_renders_on_kernel_focus_in_production_tree` — same `<App />` mount, drive `spatial_focus(boardSelectorKey)`, assert `data-focused="true"` AND `[data-testid="focus-indicator"]` on the board-selector leaf.
- [ ] `navbar_arrow_right_traverses_in_production_tree` — same mount, focus the board-selector, simulate `KeyboardEvent("ArrowRight")` on `document.body` (or on the focused element — match the production key-router's listening surface), assert focus advances to inspect (or search per the same-kind iter-0 filter — pin which).
- [ ] `navbar_arrow_up_from_board_lands_on_navbar_in_production_tree` — same mount, focus a board card, simulate `KeyboardEvent("ArrowUp")`, assert focus lands on a navbar moniker.
- [ ] `navbar_arrow_down_from_navbar_lands_on_board_or_perspective_in_production_tree` — symmetric.
- [ ] `navbar_debug_overlay_renders_in_production_tree` — assert the blue zone border and three emerald scope borders exist on the navbar surface (subsumes the original task's overlay assertions).

Test command: `cd kanban-app/ui && bun test nav-bar.production-tree.browser` — all six pass.

### `kanban-app/ui/src/components/nav-bar.guards.node.test.ts` (new file)

- [ ] `app_tree_renders_navbar_inside_focus_layer` — parse `App.tsx` AST; walk the JSX from the App's root export down to the `<NavBar />` element; assert a `<FocusLayer>` ancestor exists in that path. Failure mode: a future refactor moves NavBar out of the layer subtree without this guard catching it.

Test command: `cd kanban-app/ui && bun test nav-bar.guards` — passes.

### Existing tests must keep passing

- [ ] `kanban-app/ui/src/components/nav-bar.focus-indicator.browser.test.tsx` — five tests.
- [ ] `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` — full file.
- [ ] `swissarmyhammer-focus/tests/navbar_arrow_nav.rs` — six tests.
- [ ] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` — overlay component contract.

Test command: `cd kanban-app/ui && bun test nav-bar focus-debug-overlay && cargo test -p swissarmyhammer-focus --test navbar_arrow_nav` — all green.

## Workflow

- Use `/tdd` — start by writing the failing production-tree test (`nav-bar.production-tree.browser.test.tsx`). It must fail today.
- Bisect via DevTools per the **Approach → 0. Reproduce and bisect** steps to identify which hypothesis (H1 most likely) the failure matches.
- Fix only the failing seam. Do not bundle multiple hypothesis-driven fixes.
- Re-open or supersede `01KQ9XWHP2Y5H1QB5B3RJFEBBR` and `01KQ9Z56M556DQHYMA502B9FKB` if the root cause invalidates their "already works in production" claim — at minimum, leave a comment on those cards pointing here.
- This is a **release blocker**. Treat as the highest-priority spatial-nav card. Other surfaces can wait.

#blocker #frontend #spatial-nav #kanban-app