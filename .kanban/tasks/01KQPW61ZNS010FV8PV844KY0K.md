---
assignees:
- claude-code
position_column: todo
position_ordinal: c780
project: spatial-nav
title: Delete redundant ui:view FocusZone — overlaps ui:board / ui:grid
---
## What

Delete the `ui:view` `<FocusZone>` wrapper from `kanban-app/ui/src/components/view-container.tsx`. It is a viewport-filling chrome zone whose rect exactly overlaps the inner view's own zone (`ui:board` for `<BoardView>`, `ui:grid` for `<GridView>`), and it adds no semantic value to the spatial graph — it just inserts an extra zone level the cascade has to traverse.

### Current state

`view-container.tsx:86–107` defines `ViewSpatialZone`, which wraps the active view in:

```
<FocusZone
  moniker={asSegment("ui:view")}
  showFocusBar={false}
  className="flex-1 flex flex-col min-h-0 min-w-0"
>
  {children}
</FocusZone>
```

The inner view zones use the same flex chain to fill the same rect:

- `BoardSpatialZone` (board-view.tsx:1145–1173) → `<FocusZone moniker={asSegment("ui:board")} showFocusBar={false} className="flex flex-1 min-h-0">`.
- `GridSpatialZone` (grid-view.tsx:894–917) → `<FocusZone moniker={asSegment("ui:grid")} showFocusBar={false} className="flex-1 flex flex-col min-h-0">`.

Result: two overlapping zones (`ui:view` and `ui:board`/`ui:grid`) registered with the kernel for the same screen real estate, both with `showFocusBar={false}`. The cascade has to step through `ui:view` for nothing.

The `view:{viewId}` `<CommandScopeProvider>` on `ViewContainer` (view-container.tsx:57) stays — it's the React command-scope frame, not a spatial zone, and is still needed so view-scoped commands resolve.

### Fix shape

1. In `kanban-app/ui/src/components/view-container.tsx`:
   - Delete `ViewSpatialZone` and its imports (`FocusZone`, `useOptionalEnclosingLayerFq`, `useOptionalSpatialFocusActions`, `asSegment`).
   - In `ViewContainer`, replace `<ViewSpatialZone>{...}</ViewSpatialZone>` with the children directly.
   - Keep `<CommandScopeProvider moniker={moniker}>` as-is.

2. The placeholder branch in `ActiveViewRenderer` (the "X view (Y) is not yet implemented" `<main>`) currently has no inner spatial zone. It will continue to have none after this change, which is fine — the placeholder has no focusable descendants, so no `parent_zone` is needed.

### Tests / docs to update

- `kanban-app/ui/src/components/view-container.spatial-nav.test.tsx` — every assertion about `ui:view` registration and the `[data-segment='ui:view']` wrapper is no longer valid. Either delete the file (if it exists solely to test the wrapper) or replace its tests with regression assertions that **no** `ui:view` zone registers and **no** `[data-segment='ui:view']` element is present even when wrapped in `SpatialFocusProvider` + `FocusLayer`.
- `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts:125` — the `wraps the rendered view in FocusZone with moniker ui:view` source guard must be deleted (or inverted to a "must NOT contain" guard).
- `kanban-app/ui/src/components/perspective-view.spatial.test.tsx` — review every assertion that mentions `ui:view`, `ui:perspective.view`, or registration counts (test #1 and #6 around lines 276, 307). After removal, `ui:perspective`'s direct child becomes `ui:board` / `ui:grid`. Update the parent-zone assertions accordingly.
- `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx:1154` — selector `[data-segment='ui:view']` will return null after the fix; update or delete that assertion.
- `kanban-app/ui/src/components/grid-view.tsx:887` and `kanban-app/ui/src/components/board-view.tsx:1133–1143` — doc comments claim `parent_zone` is `ui:view`. After this change `ui:perspective` becomes the parent for `ui:board` and `ui:grid` (until/unless `ui:perspective` is removed in a follow-up). Update the docstrings.

### Out of scope

- Removing `ui:perspective` (perspective-container.tsx:154–174) is the same kind of redundancy but the user did not call it out — file separately if desired. Do NOT include it here. Stay on task.

## Acceptance Criteria

- [ ] `ViewSpatialZone` no longer exists in `view-container.tsx`. `grep -r "ui:view"` in `kanban-app/ui/src` returns no production source matches.
- [ ] Mounting `<ViewContainer>` inside the production provider stack produces zero `spatial_register_zone` calls with `segment === "ui:view"`. The DOM has no `[data-segment='ui:view']` element.
- [ ] When `<BoardView>` is the active view, `ui:board`'s `parent_zone` resolves to `ui:perspective`'s FQM (one zone level closer to the root). When `<GridView>` is the active view, `ui:grid`'s `parent_zone` similarly resolves to `ui:perspective`'s FQM.
- [ ] `ViewContainer`'s `<CommandScopeProvider moniker="view:{viewId}">` is still present — view-scoped command resolution unchanged.
- [ ] `pnpm -C kanban-app/ui test` passes with no skipped tests added. Updated `perspective-view.spatial`, `view-container.spatial-nav`, `perspective-spatial-nav.guards`, and `spatial-nav-end-to-end.spatial` suites all green.
- [ ] `pnpm -C kanban-app/ui typecheck` passes with no unused imports left over from the removed `ViewSpatialZone`.

## Tests

- [ ] Add a regression test in `kanban-app/ui/src/components/view-container.spatial-nav.test.tsx` (or rename the file if its premise no longer fits): `it("does NOT register a ui:view zone", …)` that mounts `<ViewContainer>` inside `SpatialFocusProvider` + `FocusLayer`, lets effects flush, and asserts `mockInvoke.mock.calls.filter(c => c[0] === "spatial_register_zone" && c[1].segment === "ui:view")` is empty AND `container.querySelector("[data-segment='ui:view']")` is `null`.
- [ ] Add `it("ui:board's parent_zone is ui:perspective when board view is active", …)` in `kanban-app/ui/src/components/perspective-view.spatial.test.tsx` (or the analogous board-view test file) that captures both registrations and asserts `ui:board`'s `parentZone` arg equals `ui:perspective`'s `fq` arg — i.e. the `ui:view` hop is gone.
- [ ] Update / delete `perspective-spatial-nav.guards.node.test.ts:125` so the source no longer asserts the `ui:view` `<FocusZone>` exists; flip to assert the wrapper is absent.
- [ ] Run `pnpm -C kanban-app/ui test view-container perspective-view perspective-spatial-nav spatial-nav-end-to-end` and confirm all four target suites pass with the updated assertions.
- [ ] Run the full `pnpm -C kanban-app/ui test` to catch any other test that relied on `ui:view` being present (parent-zone assertions, scope-chain length assertions, drill-out targets).

## Workflow

- Use `/tdd` — write the regression "no `ui:view` zone registers" test first (RED against current code), delete `ViewSpatialZone` from `view-container.tsx`, then sweep the dependent tests / doc comments until the suite is green again.
