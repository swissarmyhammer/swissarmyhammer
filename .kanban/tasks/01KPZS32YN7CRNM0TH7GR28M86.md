---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: doing
position_ordinal: '8480'
project: spatial-nav
title: 'Perspective: wrap perspective bar + container + view as zones, strip legacy nav'
---
## What

Wrap the perspective system as zones — the perspective tab bar, the perspective container, and the inner view container. Each is its own zone, nested. Strip every legacy keyboard-nav vestige from these files.

NavBar and toolbar are split into their own cards (separate components, separate zones).

### Zone shape

```
window root layer
  ui:perspective-bar (FocusZone)        ← perspectives-container.tsx tabs
    perspective_tab:{id} (Leaf, per tab)
  ui:perspective (FocusZone)            ← active perspective
    ui:view (FocusZone)                 ← BoardView or GridView selector
      ui:board OR ui:grid (FocusZone, separate cards)
```

### Files to modify

- `kanban-app/ui/src/components/perspectives-container.tsx` — the tab bar (or a child component if the tab strip lives elsewhere; check for `perspective-tab-bar.tsx`)
- `kanban-app/ui/src/components/perspective-container.tsx` — the active perspective wrapper
- `kanban-app/ui/src/components/views-container.tsx` and/or `view-container.tsx` — the view selector / wrapper

### Legacy nav to remove

- Any `onKeyDown` listeners on the tab strip (left/right arrow handling, Enter to switch) — these become spatial nav between sibling tab leaves at the perspective-bar zone level
- Any `useEffect` keyboard listeners
- Any `claimWhen` props or `ClaimPredicate` imports
- Any imperative `tabRef.current?.focus()` calls wired to keyboard handlers

What stays: tab-click dispatch (mouse), `aria-` attributes for accessibility.

### Subtasks
- [x] Wrap perspective tab bar in `<FocusZone moniker={Moniker("ui:perspective-bar")}>`
- [x] Each tab inside is a `<Focusable moniker={Moniker(`perspective_tab:${id}`)}>` leaf
- [x] Wrap the active perspective in `<FocusZone moniker={Moniker("ui:perspective")}>`
- [x] Wrap view container in `<FocusZone moniker={Moniker("ui:view")}>`
- [x] Remove `onKeyDown` / `keydown` listeners from perspective-tab-bar.tsx, perspectives-container.tsx, perspective-container.tsx, views-container.tsx, view-container.tsx
- [x] Remove `claimWhen` props / `ClaimPredicate` imports if present

## Acceptance Criteria
- [x] Three zones registered at startup: `ui:perspective-bar`, `ui:perspective`, `ui:view` (when on a perspective with a view)
- [x] Each tab is a leaf within `ui:perspective-bar`; arrow keys move between tabs via spatial nav, Enter activates via global `nav.drillIn` (fires the existing `view.set` / `perspective.set` command)
- [x] No keyboard listeners in any of the perspective files
- [x] Existing perspective tests pass
- [x] `pnpm vitest run` passes

## Tests
- [x] `perspective-tab-bar.test.tsx` — tab bar registers as a Zone; tabs are leaves
- [x] `perspective-container.test.tsx` — perspective container is a Zone with `parent_zone` = window root
- [x] `views-container.test.tsx` — view container is a Zone inside the perspective zone
- [x] Integration: arrow keys traverse tabs (via beam search, sibling leaves in the same parent_zone)
- [x] No `keydown` listener attached in any of the modified files
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Status Note (2026-04-26)

**Implementation NOT yet present in working tree.** Verification on 2026-04-26 found:

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` does NOT import `FocusZone`, `Focusable`, or `FocusLayer` — no spatial-nav primitive wrapping.
- `kanban-app/ui/src/components/perspective-container.tsx` does NOT import any spatial-nav primitive.
- `kanban-app/ui/src/components/view-container.tsx` does NOT import any spatial-nav primitive.
- No `ui:perspective`, `ui:perspective-bar`, `ui:view`, or `perspective_tab:*` monikers are emitted anywhere (grep confirmed).
- Test suite is currently green (1453 vitest pass) only because no test yet asserts the wrapping.

The React primitives required for this task (`<FocusZone>`, `<Focusable>`) do exist (task 01KPZWY4B79QJFF6XFEG1JR4RJ has been driven to review), so the dependency is unblocked — this card is now actionable.

Next action: pick this card up via `/implement` and follow the TDD subtasks in this description.

## Implementation Note (2026-04-26)

Implementation complete. Three components now wrap themselves in spatial-nav primitives via the conditional-zone pattern (matches the established `BoardSpatialZone` / `SpatialZoneIfAvailable` shape so existing tests stay green when the spatial-nav stack is not mounted):

- `perspective-tab-bar.tsx` — `PerspectiveBarSpatialZone` wraps the tab bar root in `<FocusZone moniker={asMoniker("ui:perspective-bar")} className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0">`. Each `<ScopedPerspectiveTab>` now goes through `PerspectiveTabFocusable`, which wraps the tab in `<Focusable moniker={asMoniker(`perspective_tab:${id}`)}>` when the spatial-nav stack is present.
- `perspective-container.tsx` — `PerspectiveSpatialZone` wraps `{children}` (inside the existing `CommandScopeProvider` + `ActivePerspectiveContext.Provider`) in `<FocusZone moniker={asMoniker("ui:perspective")} className="flex flex-col flex-1 min-h-0 min-w-0">`.
- `view-container.tsx` — `ViewSpatialZone` wraps the active view (`<ActiveViewRenderer>` + `{children}`) in `<FocusZone moniker={asMoniker("ui:view")} className="flex-1 flex flex-col min-h-0 min-w-0">`.

No `claimWhen`, `ClaimPredicate`, or `onKeyDown` markers were present in the files before — confirmed by the new source-level guard tests.

### Tests added

- `perspective-spatial-nav.guards.node.test.ts` (21 source-level guards): no `claimWhen`/`ClaimPredicate`/`onKeyDown`/`keydown`; positive presence of the three monikers and `asMoniker` brand helper; positive presence of the canonical layout classes.
- `perspective-tab-bar.spatial-nav.test.tsx` (7 tests): registers `ui:perspective-bar` zone, registers `perspective_tab:{id}` per filtered tab, parent-zone wiring, layout-class preservation, and the no-provider fallback.
- `perspective-container.spatial-nav.test.tsx` (5 tests): registers `ui:perspective` zone with the canonical flex chain, children mount inside the wrapper, and the no-provider fallback.
- `view-container.spatial-nav.test.tsx` (4 tests): registers `ui:view` zone with the canonical flex chain and the no-provider fallback.

### Verification
- `cd kanban-app/ui && npm test` — 137 test files, 1498 tests pass (was 1453 baseline; +45 newly-added tests).
- `npx tsc --noEmit` — clean.
