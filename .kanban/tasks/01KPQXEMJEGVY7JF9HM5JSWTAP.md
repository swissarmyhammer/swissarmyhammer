---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffa80
project: spatial-nav
title: 'LeftNav: visual focus indicator + Enter to activate the focused view'
---
## What

User report: "on any view, I cannot navigate left to get to the nav bar and I want to, and then navigate between the nav bar items with enter selecting a view."

Per task `01KPNWPX9NWSVGTJAHB4Z1VSED` (done), `h` from any view body does move focus to a LeftNav button and `j`/`k` does move between buttons — verified by vitest-browser assertions on the shim's `focusedMoniker()`. But from the user's seat nothing appears to happen:

1. **No visual focus indicator on LeftNav buttons.** `left-nav.tsx:85-102` renders a `<button>` with `data-active={isActive}` (whose value is driven by *which view is currently open*, not which button is spatially focused) and no `data-focused` attribute. The enclosing `FocusScope` is `showFocusBar={false}` so its own overlay never shows. Net effect: a focused-but-not-active button looks identical to any other inactive button. This is the same defect pattern as the row selector in `01KPQX6TEZG9SG88B31KGKS2D5` — spatial state is correct, the `<td>`/`<button>` just never renders the focused state.

2. **Enter does not activate the focused view.** `ViewButton` at `left-nav.tsx:122` passes `commands={[]}` to its `FocusScope`. Mouse click calls `handleClick` (setFocus + `dispatch(\"view.switch:<id>\")`), but pressing Enter while the button is focused runs nothing — there is no command in scope bound to the Enter key. The user's stated contract is "Enter selects the view"; today it's a no-op.

### Fix approach

**Visual indicator (`left-nav.tsx` → `ViewButtonElement`)**

Match the pattern established by `FixtureCellDiv` (`spatial-grid-fixture.tsx:162`) and requested for the row selector: subscribe to `useFocusedMoniker()` from `@/lib/entity-focus-context`, compare against the `viewMoniker` prop, and set `data-focused={isFocused || undefined}` on the `<button>`. Apply a distinct ring style when focused — `ring-2 ring-primary ring-offset-2 ring-offset-background ring-inset` (the `ring-offset-background` keeps the ring visible against both active and inactive button backgrounds). Do not touch the existing `data-active` styling — focused and active are independent signals that can overlap on the currently-open view if the user navigates back to it.

**Enter → activate (`left-nav.tsx` → `ViewButton`)**

Promote `commands` from `[]` to a per-view array containing one `view.activate` entry bound to Enter across all keymaps. `execute` reuses the existing `handleClick` so mouse and keyboard run the exact same path (`setFocus(mk)` + `dispatch(\"view.switch:<id>\")`). Pass the commands to the `FocusScope`. Because keybinding resolution walks the focused-scope chain and inner scopes win, Enter lands on this command only when the LeftNav button is the focused scope — outer Enter handlers (`inspector.edit`, grid edit, etc.) are not shadowed globally.

### Files touched

- `kanban-app/ui/src/components/left-nav.tsx` — `ViewButtonElement` gains the `useFocusedMoniker` subscription + focus-ring class; `ViewButton` builds a per-view `commands` array with the Enter-bound `view.activate.<id>` and passes it to `FocusScope`.
- `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx` — extend existing "h from ... moves focus to the active LeftNav button" assertion to also require `data-focused=\"true\"` and a `ring-2` class on the button. Add a new test: Enter on a focused LeftNav button dispatches `view.switch:<id>` and lands focus on that button (the click path).
- `kanban-app/ui/src/test/setup-spatial-shim.ts` — add `dispatchedCommands` capture to the shim so tests can assert `dispatch_command` invocations without touching the module-level `invoke` mock directly.

### Out of scope

- Command-palette entry for "Activate view" — the per-button `view.activate.<id>` commands are scope-local by design so they show only when the relevant button is focused. Adding them to the global palette is a separate concern.
- Changing `data-active` semantics — focused and active stay independent.
- Other views' focus indicators (row selector is tracked separately in `01KPQX6TEZG9SG88B31KGKS2D5`; data cells already work via `grid.cursor` / `isCursor`).

## Acceptance Criteria

- [x] A LeftNav `<button>` carries `data-focused=\"true\"` while it is the spatially-focused entry
- [x] When focused, the button paints a visible ring (`ring-2 ring-primary ring-offset-2 ring-offset-background ring-inset`)
- [x] `data-active` styling continues to indicate the currently-open view independently of focus
- [x] Pressing Enter while a LeftNav button is focused dispatches `view.switch:<id>` for that view and leaves focus on that button (identical to a click)
- [x] Existing tests stay green: `spatial-nav-leftnav.test.tsx`, `left-nav.test.tsx` if present, `spatial-nav-{grid,board,inspector}.test.tsx`
- [ ] Manual smoke in the running app: from a grid or board, press `h` enough times to reach LeftNav → focused button has a visible ring → press `j`/`k` → ring moves between buttons → press Enter → corresponding view opens and focus stays on that button

## Tests

- [x] Extend `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx::\"h from leftmost ...\"` (and its grid sibling) — after the existing `focusedMoniker` assertion, also assert `expect(viewButton).toHaveAttribute(\"data-focused\", \"true\")` and `expect(viewButton.className).toMatch(/ring-2/)`. Must fail against HEAD because the production `<button>` never gets a `data-focused` attribute.
- [x] Add new test `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx::\"Enter on a focused LeftNav button dispatches view.switch\"` — focus a LeftNav button (click or nav), then `userEvent.keyboard(\"{Enter}\")`, then assert the invoke mock received `dispatch_command` with `cmd: \"view.switch:<id>\"`. If the dispatch mock in `setup-spatial-shim.ts` does not already observe `dispatch_command`, add a pass-through that records invocations into the returned handles for assertion. Must fail against HEAD because there is no Enter binding in scope.
- [x] Run `cd kanban-app/ui && npm test -- spatial-nav-leftnav` — both new assertions + the new Enter test pass
- [x] Run `cd kanban-app/ui && npm test -- left-nav` — if a unit test file exists for LeftNav, confirm it still passes (no unit test file exists for LeftNav)
- [x] Run `cd kanban-app/ui && npm test -- spatial-nav` — entire spatial-nav-*.test.tsx suite still green (guards against scope-chain regressions from the new commands)

## Workflow

- Use `/tdd` — extend the existing nav assertion first (fails at the visual checks), add the Enter test (fails at dispatch assertion), then add the `useFocusedMoniker` subscription and the `view.activate.<id>` command. Land all changes in one commit so the tests ship paired with the fix.
- Do NOT add a new FocusHighlight wrapper around the button. The scope stays `renderContainer={false}` + `showFocusBar={false}`; the button itself owns the ring styling via the `data-focused` attribute, same as the row selector fix.

## Review Findings (2026-04-21 09:20)

Both contracts verified — Enter-to-activate works (5/5 LeftNav tests pass, including the new `\"Enter on a focused LeftNav button dispatches view.switch\"`), and focus visibility works via the centralized `useFocusDecoration` (tests at `spatial-nav-leftnav.test.tsx:160, 189` assert `data-focused === \"true\"` on the button). The wider `spatial-nav` suite (32 tests) stays green, confirming the new Enter binding doesn't shadow outer Enter handlers. The visual portion approached via the centralization path (supersedes the inline-ring plan from the task body) and lands correctly.

### Nits
- [x] `kanban-app/ui/src/test/spatial-nav-leftnav.test.tsx:199-204` — Stale comment contradicts the rest of the file. The block says \"LeftNav buttons use `showFocusBar={false}`… Consequently, the `<button>`'s `data-focused` attribute never flips to `\\\"true\\\"`\", but the production `FocusScope` in `left-nav.tsx:174-179` uses the default `showFocusBar={true}` (not `false`), and the neighboring tests at lines 160 and 189 explicitly assert `data-focused === \"true\"` on the button. Trim the paragraph down to its still-true half: the `j`-between-buttons assertion reads from `focusedMoniker()` because the test doesn't need to resolve which button element to query before polling. Delete the `showFocusBar={false}` claim and the \"never flips to `\\\"true\\\"`\" sentence so future readers don't chase a non-existent prop.
- [x] `kanban-app/ui/src/components/left-nav.tsx:68-70, 126-131` — Comments reference the focus-ring contract in words (\"the button itself owns the ring styling via the `data-focused` attribute\", \"the global `[data-focused]` CSS rule paints the ring\") but do not point at `index.css:148` where the rule actually lives. A one-line anchor (e.g. `// see index.css:148 — single global [data-focused] ring rule`) would save the next reader a grep.
