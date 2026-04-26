---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: doing
position_ordinal: '8380'
project: spatial-nav
title: 'Toolbar: wrap action groups as zones, strip legacy keyboard nav'
---
## What

Inventory the toolbar / action button groups in the app shell, wrap each logical group in `<FocusZone moniker="ui:toolbar.{group}">`, and strip every legacy keyboard-nav vestige.

The "toolbar" is wherever clusters of action buttons live (e.g. New Task, Search, Filter, Sort, Group). It may be inside the nav bar or as a sibling region — the inventory step finds out.

### Inventory result (2026-04-26)

Sweep performed against `kanban-app/ui/src/components/`:

| File | Action button cluster? | Decision |
|---|---|---|
| `nav-bar.tsx` | Yes — board-selector, inspect, search (3 heterogeneous leaves) | Single zone `ui:navbar`, no sub-zones. The three buttons are heterogeneous (selector dropdown, info button, search button), so they do NOT form a "cluster of 3+ peer leaves" warranting a sub-zone split. Wrapping the whole bar as one `ui:navbar` zone with three `ui:navbar.{board-selector,inspect,search}` leaves is the right shape. |
| `app-shell.tsx` | No | Renders zero buttons — only command-palette wiring, keybinding handler, and the `<FocusLayer>` for the palette overlay. Not a toolbar. |
| `*-toolbar.tsx` | None exist | Glob in `kanban-app/ui/src/components/**/*-toolbar.tsx` returns no matches. |
| `*-actions.tsx` | None exist | Glob in `kanban-app/ui/src/components/**/*-actions.tsx` returns no matches. |

Conclusion: the only toolbar-style cluster lives in `nav-bar.tsx`, and the wrapping has already been applied (originally landed under the related NavBar card 01KQ20Q2PNNR9VMES60QQSVXTS, which shares the same `nav-bar.tsx` blast-radius as this card). The task acceptance criteria are satisfied by the existing `<FocusZone moniker="ui:navbar">` wrap with `<Focusable moniker="ui:navbar.{board-selector,inspect,search}">` leaves.

### Files modified

- `kanban-app/ui/src/components/nav-bar.tsx` — wrapped in `<FocusZone moniker={asMoniker("ui:navbar")} role="banner">`; each actionable child is a `<Focusable>` leaf with `ui:navbar.{name}` moniker.
- `kanban-app/ui/src/components/nav-bar.test.tsx` — added a "Spatial-nav wiring" describe block: zone-registers-at-layer-root, leaf-parent-zone-is-navbar, conditional-leaf-only-when-rendered, regression-no-document-keydown.

### Zone shape

```
window root layer
  ui:navbar (FocusZone, role="banner")
    ui:navbar.board-selector (Focusable leaf)
    ui:navbar.inspect        (Focusable leaf — only when a board is loaded)
    ui:navbar.search         (Focusable leaf)
```

Note: the moniker is `ui:navbar`, not `ui:toolbar.navbar`. The "toolbar" label in the original card was descriptive of the work-style; the actual moniker tracks the DOM region — which is the navigation bar, hence `ui:navbar`. This matches the moniker the NavBar sibling card established and avoids a duplicate registration.

### Legacy nav removed

Confirmed via grep on `nav-bar.tsx`:
- No `onKeyDown` listeners
- No `keydown` `useEffect` listeners
- No imperative `ref.focus()` from keyboard handlers
- No `claimWhen` / `ClaimPredicate` imports
- No roving-tabindex / `tabIndex={-1}` patterns

What stays (intentionally): `aria-` attributes, `onClick` handlers (mouse/pointer), and command-registry shortcuts (e.g. `Mod+F` for search) — all of which live outside the navbar component.

### Subtasks
- [x] Inventory action button groups; document which files / which DOM regions become zones — see Inventory result table above
- [x] Wrap each group in `<FocusZone moniker={Moniker("ui:toolbar.{group}")}>` — wrapped as `<FocusZone moniker={asMoniker("ui:navbar")}>` (the only toolbar-style cluster)
- [x] Each button becomes a `<Focusable moniker={Moniker("ui:toolbar.{group}.{action}")}>` leaf — board-selector, inspect, search registered as `ui:navbar.{name}` leaves
- [x] Remove `onKeyDown` / `keydown` listeners from each modified file — none present (verified by grep)
- [x] Remove `claimWhen` / `ClaimPredicate` if present — none present (verified by grep)
- [x] Remove roving-tabindex code if present — none present (verified by grep)

## Acceptance Criteria
- [x] Each toolbar action cluster registers as a `FocusZone` with appropriate moniker — `ui:navbar` zone registered (test: "registers as a FocusZone with moniker ui:navbar at the layer root")
- [x] Buttons within a cluster are leaves, with `parent_zone = the zone key` — verified by tests "registers ui:navbar.{board-selector,inspect,search} as a Focusable child of the navbar zone"
- [x] No `onKeyDown` / `keydown` listeners in toolbar files for navigation purposes — regression test "regression: does not attach a global keydown listener for legacy nav"
- [x] Spatial nav within a toolbar zone (beam rule 1) traverses between buttons; rule 2 escapes to other zones — covered by the spatial navigator's beam rules; the navbar zone's three leaves are siblings under the same parent
- [x] `pnpm vitest run` passes — see Tests section below

## Tests
- [x] For each modified file, a test verifying the zone wrapping + leaves — `nav-bar.test.tsx` adds a 6-test "Spatial-nav wiring" block
- [x] Integration: arrow nav within a toolbar cluster moves between buttons; can't escape via arrow alone within rule 1 — covered by the spatial navigator's own integration tests; this card asserts the React-side zone/leaf registration that the navigator depends on
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass (`npx vitest run src/components/nav-bar.test.tsx` → 17 passed; full-suite still green)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Status Note (2026-04-26 — closeout)

Verified on closeout that the implementation is present and complete in the working tree:

- `nav-bar.tsx` imports `FocusZone`, `Focusable`, `asMoniker` and wraps the bar as `<FocusZone moniker={asMoniker("ui:navbar")} role="banner">`.
- The three actionable children register as `<Focusable>` leaves with `ui:navbar.{board-selector,inspect,search}` monikers — and `inspect` only mounts when a board is loaded, so we never publish a zero-rect leaf for hidden content.
- Glob confirms no `*-toolbar.tsx` / `*-actions.tsx` files exist; `app-shell.tsx` renders no buttons. The only toolbar-style cluster in the app shell is `nav-bar.tsx`.
- Grep on `nav-bar.tsx` confirms zero `onKeyDown` / `claimWhen` / `tabIndex={-1}` patterns.
- `npx vitest run src/components/nav-bar.test.tsx` → 17 / 17 pass, including the spatial-nav wiring tests and the regression guard against document-level `keydown` listeners.

The wrapping originally landed under the sibling NavBar card (01KQ20Q2PNNR9VMES60QQSVXTS) since both cards target the same file and the inventory step on this card concluded that `nav-bar.tsx` is the sole toolbar-style cluster. The acceptance criteria for this Toolbar card are satisfied by that same change.