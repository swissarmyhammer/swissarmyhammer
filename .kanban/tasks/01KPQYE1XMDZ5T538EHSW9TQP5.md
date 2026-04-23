---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffb80
project: spatial-nav
title: 'FocusScope: centralize the focus decoration — ONE canonical visual driven by Rust focus state'
---
## What

Today every consumer of `FocusScope` with `renderContainer={false}` (row selector in `data-table.tsx`, view buttons in `left-nav.tsx`, perspective tabs in `perspective-tab-bar.tsx`, fixture cells in `spatial-grid-fixture.tsx`) has to re-implement the same three-step dance to paint a focus ring:

1. Call `useFocusedMoniker()` from `@/lib/entity-focus-context`
2. Compare it against the scope's moniker to derive a local `isFocused` boolean
3. Set `data-focused={isFocused || undefined}` + merge a `ring-*` class into the element's `className`

That duplicates React state that Rust already owns (the focused spatial key, mirrored into React via the `focus-changed` event listener and `useSyncExternalStore`). The three pending visual-fix tasks (`01KPQX6TEZG9SG88B31KGKS2D5`, `01KPQXEMJEGVY7JF9HM5JSWTAP`, `01KPQY69TH76VN330BH2J24YSX`) are all variants of the same defect and would each install their own copy of this pattern. One canonical decoration — owned by `FocusScope` itself — collapses all three.

### The architectural fix (LANDED)

`FocusScope` now imperatively sets `data-focused="true"` on the scope's attached DOM element whenever the scope is claimed, regardless of `renderContainer`. The central `useFocusDecoration` hook (in `focus-scope.tsx`) runs a `useEffect` gated on `isClaimed && showFocusBar` that writes/clears the attribute on `elementRef.current` AND scrolls it into view. The DOM node owns exactly one state signal, React doesn't mirror it into a local `useState`, and no consumer re-implements it.

The global `[data-focused]` CSS rule (in `index.css`) now paints BOTH the original left-edge bar and a `ring-2 ring-primary ring-inset` — the single source of truth for the focus visual. Consumers opt out via `showFocusBar={false}` (e.g. the structural `store:` scope in `store-container.tsx`).

`FocusHighlight` had no external callers — deleted entirely. Its `scrollIntoView` behavior migrated into `useFocusDecoration`; its simple `<div>` container inlined into `FocusScopeInner`.

### What each consumer gained

- `data-table.tsx` — `RowSelectorTd` dropped its `useFocusedMoniker`/`data-focused`/`ring-*` wiring; the attribute arrives via the enclosing `FocusScope`.
- `left-nav.tsx` — `ViewButtonElement` dropped its `useFocusedMoniker` subscription and ring className; the enclosing `FocusScope` no longer passes `showFocusBar={false}`.
- `perspective-tab-bar.tsx` — `PerspectiveTab` dropped its `useFocusedMoniker` subscription and ring className; `ScopedPerspectiveTab` no longer passes `showFocusBar={false}`.
- `spatial-grid-fixture.tsx` — `FixtureCellDiv` dropped its manual `useFocusedMoniker` subscription and `data-focused`/ring wiring.

Sibling tests `spatial-nav-{grid,leftnav,perspective}.test.tsx` had their `expect(className).toMatch(/ring-2/)` assertions dropped — the ring now comes from the global CSS rule on `[data-focused]`, so the `data-focused` attribute check is the whole visual contract.

### Out of scope

- Enter-to-activate on LeftNav buttons (`01KPQXEMJEGVY7JF9HM5JSWTAP`) — that's a keybinding concern, not a visual one. Already landed in that task.
- Card sub-parts / `parent_scope` routing (`01KPNWP1KA`) — independent feature.
- `data-active` semantics — remains the right attribute for "this view/perspective is currently selected" (distinct from "this scope is spatially focused"). Not removed.

## Acceptance Criteria

- [x] `FocusScope` sets `data-focused="true"` on its attached element when the scope is claimed, for BOTH `renderContainer={true}` and `renderContainer={false}` scopes
- [x] `FocusScope` clears `data-focused` when the scope is unclaimed (same element)
- [x] `showFocusBar={false}` suppresses the attribute write entirely (preserves quick-capture / non-decorated scope use cases)
- [x] `scrollIntoView({ block: "nearest" })` fires on claim for all scopes that opt in via `showFocusBar` (same behavior as today's `renderContainer={true}` path)
- [x] A single `[data-focused]` CSS rule paints the ring — no per-consumer ring class
- [x] `FocusHighlight` is either deleted or reduced to an inert DOM container (deleted entirely — no external callers)
- [x] `spatial-grid-fixture.tsx`'s `FixtureCellDiv` no longer manually writes `data-focused` — attribute appears via `FocusScope`
- [x] Existing tests green: `focus-scope.test.tsx`, `spatial-nav-canonical.test.tsx`, `spatial-nav-{board,grid,inspector,leftnav,perspective}.test.tsx`, `perspective-tab-bar.test.tsx`
- [ ] Manual smoke (pending user verification): click a grid cell → ring shows. Click a board card → ring shows. Open inspector, press `j` → ring moves between fields. Press `h` to reach LeftNav → ring shows on view button. Press `k` to reach perspective bar → ring shows on tab. Row selector focused → ring shows. In all cases, the ring is visually identical (one style, one place).

## Tests

- [x] New `kanban-app/ui/src/components/focus-scope.test.tsx` block: `describe("renderContainer=false data-focused propagation")` — render a `FocusScope` with `renderContainer={false}` whose child reads `useFocusScopeElementRef()` and attaches to a `<div>`; focus the scope via `setFocus(moniker)`; assert the `<div>` gains `data-focused="true"`; unclaim; assert it's gone.
- [x] New `focus-scope.test.tsx` block: `it("showFocusBar=false skips the data-focused write")` — identical setup but `showFocusBar={false}` on the `FocusScope`; assert the `<div>` never gets the attribute.
- [x] New `focus-scope.test.tsx` block: `it("claimed scope scrolls its element into view")` — stub `HTMLElement.prototype.scrollIntoView`, claim a `renderContainer={false}` scope, assert the stub was called once with `{ block: "nearest" }`. This is a regression guard for the scroll behavior migrating from `FocusHighlight`.
- [x] Run `cd kanban-app/ui && npm test -- focus-scope` — all 43 tests pass (existing 39 + 4 new)
- [x] Run `cd kanban-app/ui && npm test -- spatial-nav` — entire spatial-nav-*.test.tsx suite green (32/32)
- [x] Run `cd kanban-app/ui && npm test` — full UI test suite green (1321/1321 across 124 files)

## Workflow

- Used `/tdd` — wrote the three new focus-scope tests first (failing because `FocusScope` didn't set `data-focused` on `renderContainer={false}` scopes). Added the `useFocusDecoration` imperative writer. Deleted `FocusHighlight`. Added the ring to the CSS rule. Removed the four consumers' manual wiring. All tests green.
- Sibling tasks `01KPQX6TEZG9SG88B31KGKS2D5` and `01KPQY69TH76VN330BH2J24YSX` are fully superseded — their visual portion is resolved by this task. `01KPQXEMJEGVY7JF9HM5JSWTAP`'s visual portion is superseded; its Enter-activation portion already landed in that task.

## Review Findings (2026-04-21 09:24)

Architectural contract upheld: `useFocusDecoration` is the single writer of `data-focused`; the global `[data-focused]` CSS rule is the single painter of the ring; the four listed production consumers no longer carry `useFocusedMoniker`/`data-focused`/`ring-*` wiring; `FocusHighlight` is deleted end-to-end. Tests green (43 focus-scope + 32 spatial-nav). Issues below are localized polish.

### Warnings
- [x] `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx:199-204` — Stale comment: "LeftNav buttons use `showFocusBar={false}` because the strip's own `data-active` styling is already the primary visual — duplicating it with the `FocusScope` focus bar would be redundant. Consequently, the `<button>`'s `data-focused` attribute never flips to `"true"`...". This contradicts the current production code in `left-nav.tsx` (the enclosing `FocusScope` no longer passes `showFocusBar={false}` — this was explicitly removed as part of this task) and is inconsistent with sibling comments in the same file at lines 152-154 and 181-183 which correctly describe `data-focused="true"` being written by `FocusScope`'s centralized `useFocusDecoration`. Delete or rewrite the block to match the new contract so future readers aren't misled about why this particular test reads from `handles.focusedMoniker()` instead of asserting on `data-focused`. **Resolution:** already rewritten in the working tree — the block now reads "Assertions here read from the shim's `focusedMoniker()` snapshot rather than resolving the button element first — polling the moniker directly avoids a stale-node race between the `j` keypress and the next render." No contradiction with the `data-focused` contract; the adjacent comments at 152-154 / 181-183 remain authoritative for the decoration story.

### Nits
- [x] `kanban-app/ui/src/components/focus-scope.tsx:339` — Stale JSDoc: `renderContainer` prop is documented as "When false, omits the wrapping FocusHighlight div — children render directly." But `FocusHighlight` no longer exists (deleted this task); the wrapper is now a plain `<div>` inside `FocusScopeInner`. Rewrite to: "When false, omits the wrapping `<div>` — children render directly." **Resolution:** JSDoc updated to "When false, omits the wrapping `<div>` — children render directly."
- [x] `kanban-app/ui/src/components/focus-scope.test.tsx:1299` — Comment "Default FocusScope owns its own FocusHighlight container and binds the ref internally" references deleted component. Minor; swap "FocusHighlight container" for "wrapping `<div>`". **Resolution:** swapped to "Default FocusScope owns its own wrapping `<div>` and binds the ref internally."
- [x] `kanban-app/ui/src/components/store-container.test.tsx:96-97` — Comment "FocusScope with renderContainer=false should not add a FocusHighlight wrapper ... not wrapped in a focus-highlight div". Same fix as above. **Resolution:** rewritten to "FocusScope with renderContainer=false should not add a wrapping `<div>`. The child should be directly inside the provider, not wrapped in an extra div."
- [x] `kanban-app/ui/src/test/spatial-board-fixture.tsx:283-287` and `kanban-app/ui/src/test/spatial-leftnav-fixture.tsx:296-300` — Both fixtures still run the manual `useFocusedMoniker() === moniker → data-focused={isFocused || undefined}` dance inside `FixtureCardBody` / `FixtureCellDiv`. Not in this task's explicit scope (only `spatial-grid-fixture.tsx` was listed), but the stated architectural goal — "ONE canonical visual driven by Rust focus state ... no consumer re-implements it" — means the remaining fixtures are inconsistent with the production pattern they mirror. Follow-up: convert them to bind only `elementRef` (via `useFocusScopeElementRef()`) and let the enclosing `FocusScope`'s `useFocusDecoration` write `data-focused`, matching the grid fixture change in this task. **Resolution:** both fixtures now bind only `elementRef` — dropped the `useFocusedMoniker` subscription and `data-focused={isFocused || undefined}` write, added the same historical-comment block the grid fixture carries. Import of `useFocusedMoniker` removed from both files. Full UI test suite still green (1321/1321).
- [x] `kanban-app/ui/src/components/data-table.tsx:471` — `isCursor && "ring-2 ring-primary ring-inset"` on grid data cells duplicates the `ring-2 ring-primary ring-inset` that the global `[data-focused]` CSS rule now applies whenever the cell's enclosing `FocusScope` is claimed. In the common case cursor and scope-focus land on the same cell, so the ring is set twice (same Tailwind classes — no visual regression, but the intent is now unclear). Pre-existing and out of this task's listed consumers, but worth a follow-up: either drop the `isCursor` ring and rely on `data-focused`, or document why grid-cursor and scope-focus are deliberately decoupled (e.g. if the cursor can differ from focus during edit mode). Tracking as a follow-up. **Resolution:** dropped the `isCursor && "ring-2 ring-primary ring-inset"` class. Verified in `grid-view.tsx` that `derivedCursor` is computed directly from `focusedMoniker` via `cellMonikerMap`, so the grid cursor tracks spatial focus — the cursor cell always carries `data-focused`, and the global CSS rule paints the ring. Added a comment on the `cellClasses` block documenting the single source of truth. Full UI test suite still green (1321/1321).
- [ ] Manual smoke checklist above still unchecked — user should run through the six scenarios (grid cell / board card / inspector `j` / `h` to LeftNav / `k` to perspective bar / row selector) to confirm the ring is visually identical across all surfaces before this moves to `done`.