---
assignees:
- claude-code
depends_on:
- 01KQJDYJ4SDKK2G8FTAQ348ZHG
position_column: todo
position_ordinal: ba80
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
- [ ] `data-table.tsx` row scope no longer triggers `scope-not-leaf` (either by promoting to a zone with `renderContainer={false}` support added to FocusZone, or by removing the row-level focus wrapper).
- [ ] `just kanban-dev` + arrow-key navigation produces zero `scope-not-leaf` log entries.
- [ ] Any new offenders found during the audit are fixed, and a regression test pinned for each.

## Tests
- [ ] Update `data-table` tests if the row's spatial registration shape changes.
- [ ] Add a browser-mode test mirroring `entity-card.scope-leaf.spatial.test.tsx` for the data-table row case.

## Implementation Notes

`<FocusZone>` currently lacks the `renderContainer={false}` option that `<FocusScope>` has. The data-table row needs to render as a `<tr>` directly inside `<tbody>` (DOM constraint) so the wrapping primitive cannot render its own `<div>`. The cleanest path is to add `renderContainer={false}` to `<FocusZone>` (mirroring `<FocusScope>`'s implementation: skip the body branch entirely and just push CommandScopeContext + FocusZoneContext + FullyQualifiedMonikerContext providers around children).

Once done, swap the row's `<FocusScope renderContainer={false}>` to `<FocusZone renderContainer={false}>`. Cell `<FocusScope>` leaves then register correctly under the row zone.