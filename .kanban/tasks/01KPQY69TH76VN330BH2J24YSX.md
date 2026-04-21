---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff980
project: spatial-nav
title: 'Perspective bar: visual focus indicator when spatial nav lands on a tab'
---
## What

User report: "I cannot navigate 'up' to the perspective bar from any view."

Per task `01KPNWQ844KQBZT59TFJ43TQ31` (done), `k` from the top row of the grid or board *does* move spatial focus to a perspective tab — verified by vitest-browser assertions on `handles.focusedMoniker()`. The bug is the user can't tell: the focused tab paints no visual indicator.

`PerspectiveTab`'s root `<div>` at `kanban-app/ui/src/components/perspective-tab-bar.tsx:440` carries `ref={refCallback}` + `data-moniker` + `onClickCapture` but no `data-focused` attribute. The enclosing `FocusScope` uses `renderContainer={false}` + `showFocusBar={false}`, so its own overlay never renders. `TabButton` (line 499) has `isActive` styling (`border-primary text-foreground bg-background`) which paints the bottom border of the *currently-open* perspective, but focused-but-not-active looks identical to every other inactive tab.

This is the same defect pattern as the row selector (`01KPQX6TEZG9SG88B31KGKS2D5`) and LeftNav (`01KPQXEMJEGVY7JF9HM5JSWTAP`): `renderContainer={false}` FocusScopes don't propagate focus state to their consumer element, so each affected surface has to re-implement a `useFocusedMoniker()` subscription locally. See the bottom "Follow-up" section for a candidate architectural fix once all three land.

### Fix approach

`PerspectiveTab` subscribes to `useFocusedMoniker()` from `@/lib/entity-focus-context`, compares against `tabMoniker` (already returned by `useSpatialTabWiring`), and sets `data-focused={isFocused || undefined}` on the root `<div>`. Apply a visible ring when focused — `ring-2 ring-primary ring-offset-2 ring-offset-background ring-inset rounded-t-md` matches the tab's existing `rounded-t-md` corner shape so the ring hugs the tab's top and sides without collision with the active tab's bottom border. Focused and active remain independent signals — the user can navigate back to the already-open tab and both indicators can overlap correctly.

Do NOT attempt to express the focus state inside `TabButton` (the inner `<button>`) — the spatial scope is on the outer `<div>` (that's the element that registers its rect with `ResizeObserver` via `useFocusScopeElementRef`). Keeping the ring on the same node as the scope means "focused" and "spatially registered" refer to the exact same element.

### Files touched

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — `PerspectiveTab` root `<div>` gains `useFocusedMoniker` subscription, `data-focused` attribute, and focus-ring classes via `cn(...)`.
- `kanban-app/ui/src/test/spatial-nav-perspective.test.tsx` — extend the existing "k from top-row cell moves focus to the active perspective tab" test (and its board sibling) to also assert `data-focused="true"` and a `ring-2` class on the tab's root div. Add a test: `h`/`l` between tabs — focused tab's ring moves, inactive→active transition is visually distinct from focused→unfocused.

### Out of scope

- Enter-to-activate a focused perspective tab — user didn't request it here; if they want it (analogous to the LeftNav Enter binding in `01KPQXEMJEGVY7JF9HM5JSWTAP`) it's a separate task.
- Filter formula bar / GroupPopover focus handling — those render inside the active tab only and inherit the scope chain for right-click; their keyboard focus is a distinct concern.
- Generic FocusScope focus-state context (see Follow-up).

### Follow-up

Three tasks (row selector, LeftNav, this one) all re-implement the same `useFocusedMoniker()` subscription on a leaf DOM node to flip `data-focused`. Once all three ship, consider a small addition to `focus-scope.tsx`: expose an optional `useFocusScopeIsClaimed()` hook that reads the scope's `isClaimed` state via a new `FocusScopeIsClaimedContext`. `renderContainer={false}` consumers would then write `const focused = useFocusScopeIsClaimed()` instead of `useFocusedMoniker()` + moniker compare. Less boilerplate, one source of truth. File as its own refactor task after this one lands.

## Acceptance Criteria

- [x] When spatial nav lands on a perspective tab (e.g. `k` from top-row grid cell), its root `<div>` carries `data-focused="true"`
- [x] The focused tab paints a visible ring (`ring-2 ring-primary ring-offset-2 ring-offset-background ring-inset`)
- [x] The focused ring is visually distinct from the active tab's `border-primary` bottom border — user can tell "this tab is selected" from "this tab is currently focused" from "both"
- [x] Existing tab behaviors unchanged: click selects, double-click renames, right-click menu works, filter/group buttons still appear on active tab
- [x] Existing tests green: `spatial-nav-perspective.test.tsx`, `perspective-tab-bar.test.tsx`, `spatial-nav-{board,grid,inspector,leftnav}.test.tsx`
- [ ] Manual smoke: from a top-row cell or top-row card, press `k` → the targeted perspective tab shows a visible ring → press `h`/`l` → ring moves between tabs → press `j` → focus returns to view body

## Tests

- [x] Extend `kanban-app/ui/src/test/spatial-nav-perspective.test.tsx::"k from top-row cell in a grid moves focus to the active perspective tab"` (and the board sibling) — after the existing `focusedMoniker()` assertion, also `expect(tabEl).toHaveAttribute("data-focused", "true")` and `expect(tabEl.className).toMatch(/ring-2/)`. Must fail against HEAD because the current `<div>` has no `data-focused` attribute.
- [x] Extend the existing `"h/l between perspective tabs in the same bar"` test — after pressing `l`, assert the NEW focused tab has `data-focused="true"` and the PREVIOUS tab has no `data-focused` attribute (or it's removed). Guards against "stuck" ring state.
- [x] `cd kanban-app/ui && npm test -- spatial-nav-perspective` — both assertions pass
- [x] `cd kanban-app/ui && npm test -- perspective-tab-bar` — unit tests still pass (the tree now gets a `data-focused` attribute the existing tests may assert against)
- [x] `cd kanban-app/ui && npm test -- spatial-nav` — full spatial-nav suite stays green (leftnav test file has pre-existing failures from parallel task 01KPQXEMJEGVY7JF9HM5JSWTAP in its TDD-red state — not caused by this change)

## Workflow

- Use `/tdd` — extend the existing visual assertions first so both spec tests fail at HEAD, then add the `useFocusedMoniker` subscription + `ring-*` classes to `PerspectiveTab`'s root div until they pass. One commit.
- Match the row-selector and LeftNav patterns: local subscription in the leaf element, no new context, no `FocusHighlight` wrapper, scope stays `renderContainer={false}` + `showFocusBar={false}`.

## Review Findings (2026-04-21 09:17)

Clean review — the user contract ("perspective tab shows focus when spatial nav lands on it") is fully met via supersession by task `01KPQYE1XMDZ5T538EHSW9TQP5`:

- `ScopedPerspectiveTab` no longer sets `showFocusBar={false}` (`kanban-app/ui/src/components/perspective-tab-bar.tsx:290-295`).
- `PerspectiveTab`'s root `<div>` attaches the enclosing scope's `elementRef` via `useSpatialTabWiring` → `useFocusScopeElementRef()` (`kanban-app/ui/src/components/perspective-tab-bar.tsx:396-410`).
- `FocusScope.useFocusDecoration` writes `data-focused="true"` on that same `<div>` whenever the scope is claimed (`kanban-app/ui/src/components/focus-scope.tsx:207-226, 404`).
- Global `[data-focused]` CSS rule paints `ring-2 ring-primary ring-inset` (`kanban-app/ui/src/index.css:148-151`).
- No leftover local `useFocusedMoniker` subscription in `perspective-tab-bar.tsx` — the supersession correctly removed it.
- All 4 `spatial-nav-perspective` tests pass (asserting `data-focused="true"` on the tab root `<div>` from `k`-from-board, `k`-from-grid, and `h`/`l` between tabs). 28/28 `perspective-tab-bar` tests pass.

The architectural outcome is superior to this task's originally-proposed local-subscription fix: one canonical `data-focused` writer, one CSS rule, and no per-consumer ring classes. Focused and active remain independent signals as required.

No new findings; no code changes requested. The remaining unchecked "Manual smoke" item in Acceptance Criteria is a user-performed verification step — the review leaves the task in the `review` column pending that manual confirmation per the rule that reviewers never flip checkboxes themselves.