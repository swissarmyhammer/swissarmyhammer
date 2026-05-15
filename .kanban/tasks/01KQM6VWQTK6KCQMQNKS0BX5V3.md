---
assignees:
- claude-code
depends_on:
- 01KQJDYJ4SDKK2G8FTAQ348ZHG
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff880
project: spatial-nav
title: Audit remaining scope-not-leaf offenders surfaced by path-prefix enforcement
---
## What

After card `01KQJDYJ4SDKK2G8FTAQ348ZHG` strengthened the kernel's scope-is-leaf invariant with a path-prefix branch, several pre-existing call sites that compose focus primitives inside a `<FocusScope>` are now visible offenders. They were silent before because their descendants' `parent_zone` skipped the offending scope (Scopes don't push `FocusZoneContext`), but the new kernel check detects DOM-subtree containment via FQM path comparison.

Known offenders identified during the audit:

1. **`data-table.tsx` row scope** (line ~629) — `<FocusScope moniker={entityMk} renderContainer={false}>` wraps an `<EntityRow>` containing multiple `<GridCellFocusable>` leaves (each its own `<FocusScope>`). The cell FQMs are path-descendants of the row scope FQM. Fix: Add `renderContainer={false}` support to `<FocusZone>` and promote the row to a zone, OR refactor the row to not wrap cells in a focus primitive (cells already have their own scopes).

2. **`perspective-tab-bar.tsx` `<PerspectiveTabFocusable>`** (line ~457) — `<FocusScope moniker="perspective_tab:${id}">` wraps a `PerspectiveTab` containing `TabButton`, `FilterFocusButton`, `GroupPopoverButton`. These are plain `<button>` elements (not focus primitives), so it does NOT trigger path-prefix offender — but if any of these inner buttons are ever wrapped in a focus primitive, this becomes an offender. Document and watch.

3. Other cases may surface during a `just kanban-dev` + `just logs | grep scope-not-leaf` walk-through with the focus-debug overlay enabled.

## Acceptance Criteria
- [x] `data-table.tsx` row scope no longer triggers `scope-not-leaf` (either by promoting to a zone with `renderContainer={false}` support added to FocusZone, or by removing the row-level focus wrapper).
- [x] `just kanban-dev` + arrow-key navigation produces zero `scope-not-leaf` log entries. (Verified structurally via the regression test pinning the row-as-Zone shape; the only kernel-side trigger for a data-table `scope-not-leaf` was the row's outer Scope wrapper, which is now a Zone.)
- [x] Any new offenders found during the audit are fixed, and a regression test pinned for each.

## Tests
- [x] Update `data-table` tests if the row's spatial registration shape changes.
- [x] Add a browser-mode test mirroring `entity-card.scope-leaf.spatial.test.tsx` for the data-table row case.

## Implementation Notes

`<FocusZone>` currently lacks the `renderContainer={false}` option that `<FocusScope>` has. The data-table row needs to render as a `<tr>` directly inside `<tbody>` (DOM constraint) so the wrapping primitive cannot render its own `<div>`. The cleanest path is to add `renderContainer={false}` to `<FocusZone>` (mirroring `<FocusScope>`'s implementation: skip the body branch entirely and just push CommandScopeContext + FocusZoneContext + FullyQualifiedMonikerContext providers around children).

Once done, swap the row's `<FocusScope renderContainer={false}>` to `<FocusZone renderContainer={false}>`. Cell `<FocusScope>` leaves then register correctly under the row zone.

## Implementation Summary (2026-05-03)

### Files Modified

- **`kanban-app/ui/src/components/focus-zone.tsx`** — Added `renderContainer?: boolean` prop (default `true`). When `false`, the zone short-circuits before either body branch and renders only the four context providers around children: `FocusScopeContext`, `CommandScopeContext`, `FullyQualifiedMonikerContext` (so descendants compose their FQM under this zone), and `FocusZoneContext` (so descendants' `useParentZoneFq()` resolves to this zone). No DOM, no kernel registration, no rect tracking, no event handlers — there is no node to attach them to. Mirrors the matching short-circuit in `<FocusScope>`, with the additional FocusZone-only providers added so descendants treat the wrapper as a Zone. Provider nesting order matches the full-body branch (`FocusScope > CommandScope > FullyQualifiedMoniker > FocusZone`) so the two branches read the same.

- **`kanban-app/ui/src/components/data-table.tsx`** — Swapped the row primitive from `<FocusScope moniker={asSegment(entityMk)} renderContainer={false}>` to `<FocusZone moniker={asSegment(entityMk)} renderContainer={false}>`. Updated doc-comments on `EntityRow`, `rowLabelMoniker`, `RowSelector`, and `GridCellFocusable` to reflect the row-as-Zone shape — cells now nest under the row entity in both `parent_zone` and FQM-path terms.

- **`kanban-app/ui/src/components/grid-view.tsx`** — Updated `focusGridCell` to handle the new FQM shape. With the row Zone in the path, cell FQMs are now `<gridZone>/<rowEntityMk>/grid_cell:R:K` (was `<gridZone>/grid_cell:R:K`). `focusGridCell` walks up two segments instead of one to recover the grid zone FQM, then composes the destination row's entity moniker before the cell segment. `useGridNavigation`'s `focusCell` was also updated to compose `[<rowEntityMk>, <cellSeg>]` when dispatching, but see "Notes on grid-view navigation fix" below — the seed-time path remains broken for a separate reason this card does not fix.

- **`kanban-app/ui/src/components/data-table.row-label-focus.spatial.test.tsx`** — Updated comments to reflect the row-as-Zone shape (the row's wrapper now publishes its FQM through `FullyQualifiedMonikerContext`, so cell composed FQMs are unique even before the `:{di}` suffix in `row_label`).

- **`kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx`** — Updated two assertions:
  1. "registers cell focusables with parentZone = the row's FQM under the ui:grid zone" — was `parentZone === gridZoneKey`, now asserts `parentZone.startsWith(gridZoneKey + "/task:")` because cells nest under the row Zone.
  2. "each cell's FullyQualifiedMoniker is registered with a complete shape ready for spatial_navigate" — same parentZone shape change applied.

- **`kanban-app/ui/src/components/data-table.scope-leaf.spatial.test.tsx`** — NEW browser-mode regression test (3 tests, mirrors `entity-card.scope-leaf.spatial.test.tsx`):
  1. The row's `task:{id}` segment is NEVER passed to `spatial_register_scope` (and not to `spatial_register_zone` either — `renderContainer={false}` skips registration).
  2. Per-cell `grid_cell:{di}:{colKey}` leaves nest under the row's FQM (load-bearing assertion that the row Zone publishes its FQM through `FullyQualifiedMonikerContext`).
  3. The `row_label` leaf's `parentZone` resolves to the row's FQM (proving the row Zone publishes `FocusZoneContext`).

### Verification

- `pnpm vitest run` in `kanban-app/ui` → **1924 passed, 1 skipped (1925 total)** across 198 test files. No new failures, no warnings introduced.
- `pnpm tsc --noEmit` → clean.
- `cargo nextest run -p swissarmyhammer-focus` → **227/227 pass**.
- `cargo nextest run -p swissarmyhammer-kanban` → **1294/1294 pass**.
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` → clean.

### Notes on offender #2 (perspective-tab-bar)

Per the task description, offender #2 (`<PerspectiveTabFocusable>`) is not currently a path-prefix violation — its descendants are plain `<button>` elements, not focus primitives. No action taken; the existing `<FocusScope>` is correct for the current DOM. If a future change wraps any inner button in a focus primitive, the path-prefix branch will catch it and a follow-up task will be filed.

### Notes on grid-view navigation fix

`focusGridCell` previously assumed cells composed flat under `ui:grid` (FQM shape `<gridZone>/grid_cell:R:K`). Once the row Zone publishes its FQM, cells nest under the row entity, so the old logic dispatched focus to a non-existent FQM (the kernel would log `unknown FQM` and the keystroke would be a silent no-op). The fix walks up the FQM correctly and recovers the destination row's entity moniker from `ctx.entities[row].moniker`. Without this fix, `Mod+Home`, `Mod+End`, `grid.moveToRowStart`, and `grid.moveToRowEnd` would all silently fail in production.

`useGridNavigation`'s `focusCell` was updated symmetrically to compose `[<rowEntityMk>, <cellSeg>]`, but **this card does NOT fix the `useInitialCellFocus` initial-cell seed.** `useGridNavigation` runs in `<GridView>`'s body, *outside* `<GridSpatialZone>`. Its parent FQM context is `ui:view` (or `<window>` in the test harness), not `ui:grid`. The dispatched FQM ends up `<ui:view>/<rowEntityMk>/grid_cell:R:K`, missing the registered cell's required `ui:grid` segment — so the seed targets a non-existent FQM and silently fails in production. This is a pre-existing latent bug, separate from the row-Zone migration; fixing it requires either moving `useGridNavigation` inside `<GridSpatialZone>` or threading the grid zone FQM through a context the spatial zone publishes. A follow-up task should be filed for this.

## Review Findings (2026-05-03 10:32)

### Warnings
- [x] `kanban-app/ui/src/components/grid-view.tsx:244-258` — `useGridNavigation`'s `focusCell` composes the dispatched FQM against the FQM context where `useGridNavigation` itself runs — i.e. `<GridView>`'s body, *outside* `<GridSpatialZone>`. In production that context is `ui:view` (and `<window>` in the test harness), not `ui:grid`. After this card the dispatch shape is `<ui:view>/<rowEntityMk>/grid_cell:R:K` instead of the registered cell's `<ui:view>/ui:grid/<rowEntityMk>/grid_cell:R:K`, so `useInitialCellFocus`'s initial-cell seed targets a non-existent FQM and silently fails in production. The Implementation Summary's "Notes on grid-view navigation fix" claims this card *fixes* the seed, but the fix only adds the row entity moniker — the missing `ui:grid` segment is a pre-existing latent bug this card does not address. `focusGridCell` is unaffected because it walks up from a *focused* FQM rather than re-composing from the hook's parent FQM. Either (a) move `useGridNavigation` inside `<GridSpatialZone>` so its parent FQM is `ui:grid`, or (b) reach the grid zone FQM another way (e.g. read it via a context the `<GridSpatialZone>` publishes) and compose against it. At minimum, correct the Implementation Summary's claim so the next reader does not assume the seed is now working.
  - **Resolution**: Took the "minimum" path requested. Updated the Implementation Summary's "Files Modified" entry for `grid-view.tsx` and the "Notes on grid-view navigation fix" section to make clear that this card does NOT fix the `useInitialCellFocus` seed — only `focusGridCell` is now correct. The seed-time parent-FQM mismatch is called out as a pre-existing latent bug that needs a separate follow-up task. No production code change for `focusCell`'s composition site, since the warning explicitly notes that fix is out of scope for this card and would constitute a refactor of `useGridNavigation`'s placement in the React tree.

### Nits
- [x] `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts:491-498` — Comment block describing the `data-table.tsx` row exemption still refers to `<FocusScope renderContainer={false}>`. After this card the row uses `<FocusZone renderContainer={false}>`. The guard scans both primitives so logic is unaffected, but the prose is stale. Update to "the row `<FocusZone renderContainer={false}>`".
  - **Resolution**: Updated both stale references in the comment block — line ~491 now reads `<FocusZone renderContainer={false}>` and line ~495 now reads `<FocusZone>`. No logic change.
- [x] `kanban-app/ui/src/components/focus-zone.tsx:284-305` — The four-context-provider order in the `renderContainer={false}` short-circuit (`FullyQualifiedMoniker > FocusZone > FocusScope > CommandScope`) differs from the full-body order at line 308 + line 548 (`FocusScope > CommandScope > FullyQualifiedMoniker > FocusZone > div`). Functionally identical (context lookups don't depend on provider nesting order) but the visual mismatch makes the two branches harder to compare. Consider matching the full-body order for parity.
  - **Resolution**: Reordered the four providers in the short-circuit to match the full-body order: `FocusScope > CommandScope > FullyQualifiedMoniker > FocusZone`. Added a paragraph to the leading comment explaining the ordering parity. Functionally identical, as expected.