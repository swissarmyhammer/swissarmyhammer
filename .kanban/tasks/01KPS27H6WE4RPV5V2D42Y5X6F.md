---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8580
project: spatial-nav
title: 'Toolbar: wrap NavBar buttons in FocusScope so Up from perspective/LeftNav reaches them'
---
## What

The top app toolbar (`kanban-app/ui/src/components/nav-bar.tsx`) has no `FocusScope` wrappers on any interactive element. The Rust spatial engine never receives rect registrations for the toolbar, so pressing Up from the perspective tab bar or LeftNav has no candidate above — nothing to land on.

### Current state (nav-bar.tsx)

The `<header>` is 48px tall (`h-12`) and contains four interactive elements, all bare buttons/components:

| Element | Line | What it does | Moniker proposal |
|---|---|---|---|
| `BoardSelector` (board name + dropdown) | 35-43 | Switches boards, edits name | `toolbar:board-selector` |
| Info button (inspect board) | 45-62 | Dispatches `ui.inspect` with the board moniker | `toolbar:inspect-board` |
| `Field` (percent_complete compact) | 63-71 | Renders the board's percent — this is already a field-style component; may already produce its own FocusScope via the field layer | (confirm during implementation) |
| Search button | 72-84 | Dispatches `app.search` | `toolbar:search` |

None are `FocusScope`-wrapped. The layout geometry is fine — `header` is at `top: 0`, perspective tabs at `top: 48`, LeftNav buttons start below the header — so rect math would work if rects were registered.

### Fix direction

Wrap each interactive element in `<FocusScope renderContainer={false}>` using the existing pattern proven in LeftNav (`ViewButton` / `ViewButtonElement`) and perspective tabs (`ScopedPerspectiveTab`):

- Use `renderContainer={false}` — the button is the DOM node, no wrapper div
- Attach `useFocusScopeElementRef()` to the button via a ref callback (so `ResizeObserver` measures the button's rect)
- Pick namespaced monikers: `toolbar:board-selector`, `toolbar:inspect-board`, `toolbar:search`
- Leave the FocusLayer inheritance as-is — AppShell's `<FocusLayer name="window">` is the parent, and toolbar scopes should live in that layer so they're reachable from any view

### Enter-activate integration

Per the pattern from `01KPRS0WVK7YMS20PEY12ZY70W`, each toolbar FocusScope should pass `commands` that bind Enter to the button's existing click action:

- `toolbar:inspect-board` — Enter → `dispatchInspect({ target: board.board.moniker })` (same as the existing `onClick`). Note: after `01KPRS0WVK7YMS20PEY12ZY70W` lands with the YAML-level `ui.inspect` Enter binding, this may become redundant for the inspect button specifically. Confirm during implementation.
- `toolbar:search` — Enter → `dispatchSearch()`
- `toolbar:board-selector` — more nuanced: Enter should open the dropdown (equivalent to clicking). Depends on BoardSelector's internal wiring — inspect `kanban-app/ui/src/components/board-selector.tsx` to find the correct activation path and mirror it.

### The `Field` percent_complete component

Confirmed during implementation: `Field` does not produce its own FocusScope (no `FocusScope` or `data-moniker` in `components/fields/field.tsx`), so a thin `<div>` wrapper under a new `toolbar:percent-complete` scope is required.

### Files modified

- `kanban-app/ui/src/components/nav-bar.tsx` — wrapped 4 interactive elements in FocusScope with commands bound to Enter (board-selector, inspect, percent-complete, search)
- `kanban-app/ui/src/components/nav-bar.test.tsx` — added `EntityFocusProvider` + tauri mocks + 4 moniker-registration tests
- `kanban-app/ui/src/test/spatial-toolbar-fixture.tsx` — NEW fixture composing NavBar + PerspectiveTabBar + LeftNav under the shared spatial-fixture shell
- `kanban-app/ui/src/test/spatial-nav-toolbar.test.tsx` — NEW spatial contract suite (6 tests, all green)
- `kanban-app/ui/src/test/spatial-parity-cases.json` — NEW parity case: Up from perspective-tab → toolbar:inspect-board
- `kanban-app/ui/src/components/board-selector.tsx` — no changes needed; Enter on the `toolbar:board-selector` scope clicks the existing SelectTrigger via `document.querySelector` (Radix-stable `data-slot="select-trigger"` attribute)
- `kanban-app/ui/src/components/fields/field.tsx` — no changes needed; scope wrapper sits above Field

## Acceptance Criteria

- [x] Each interactive element in `NavBar` (BoardSelector, Info/Inspect button, percent_complete Field, Search button) registers a spatial rect in the window layer via `FocusScope`
- [x] From a focused perspective tab, pressing Up lands on the toolbar element directly above it (test: `k from a focused perspective tab lands on a toolbar moniker`)
- [x] From a focused LeftNav button, pressing Up eventually reaches a toolbar element (test: `repeated k from a LeftNav button eventually reaches a toolbar moniker` — two `k` presses cross the perspective bar, then the toolbar)
- [x] Toolbar buttons show the standard `data-focused` focus bar indicator — written by `FocusScope.useFocusDecoration`; asserted inline in the perspective-tab test
- [x] Pressing Enter on the Info/Inspect button opens the board inspector (dispatches `ui.inspect` with `board:b1` target — test green)
- [x] Pressing Enter on the Search button dispatches `app.search` (test green)
- [x] Pressing Enter on the BoardSelector opens its dropdown (Enter command binds to a handler that clicks the Radix SelectTrigger via `data-slot="select-trigger"`)
- [x] Pressing Down from a toolbar button lands on the perspective tab bar or LeftNav — covered indirectly by the h/l test (focus moves between toolbar scopes, then down reverses); the symmetric k/j contract is covered by spatial-nav-perspective and spatial-nav-leftnav
- [x] h/l between toolbar buttons moves left/right across the toolbar (test: `h/l walks between toolbar elements within the same strip`)
- [x] No regression in any existing spatial nav test — full `npm test` is 1377/1377 green

## Tests

- [x] vitest-browser test in `kanban-app/ui/src/components/nav-bar.test.tsx` asserts each interactive element exposes `data-moniker:toolbar:*`
- [x] Spatial nav test `kanban-app/ui/src/test/spatial-nav-toolbar.test.tsx` renders NavBar + PerspectiveTabBar + LeftNav in the fixture shell; `k` from perspective tab lands on a `toolbar:*` moniker
- [x] Parity case in `kanban-app/ui/src/test/spatial-parity-cases.json` for Up-from-perspective-to-toolbar — Rust `cargo test -p swissarmyhammer-spatial-nav --test parity` and JS `spatial-shim-parity.test.ts` both green
- [x] Enter-activation tests green (ui.inspect w/ board moniker; app.search)
- [x] `cd kanban-app/ui && npm test` — 1377 tests pass
- [ ] Manual: click a perspective tab, press k → focus lands on toolbar. Click a LeftNav button, press k → focus lands on toolbar (from the topmost LeftNav button) or the previous LeftNav button (if not at the top).

## Workflow

- Use `/tdd` — write the failing "Up from perspective tab lands on toolbar" test first, then add the FocusScope wrappers.
- Start with the simplest toolbar element (Search button) — fewest moving parts, cleanest pattern.
- Do the BoardSelector last — it may expose internal state-machine complexity that deserves its own follow-up task if the existing click path is deeply coupled.

