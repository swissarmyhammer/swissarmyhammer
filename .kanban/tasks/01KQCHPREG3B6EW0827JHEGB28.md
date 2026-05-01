---
assignees:
- claude-code
position_column: todo
position_ordinal: a780
project: spatial-nav
title: Remove redundant FocusScope around ColumnNameField — Field is already the focus primitive at that position
---
## What

`kanban-app/ui/src/components/column-view.tsx:716` wraps `<ColumnNameField>` in `<FocusScope moniker={asMoniker(columnNameMoniker)} className="inline">`. Inside that wrap, `<ColumnNameField>` renders `<Field>` (`column-view.tsx:682`), which itself registers a `<FocusZone moniker="field:column:<id>.name">` at the same anchor.

Two registry entries at the same `(x, y)`, two near-identical monikers (`column:<id>.name` and `field:column:<id>.name`). The outer scope is needless — `<Field>` already provides the focus primitive at that position.

## What changes

`kanban-app/ui/src/components/column-view.tsx`:

1. Delete the `<FocusScope moniker={asMoniker(columnNameMoniker)} className="inline">` wrapper around `<ColumnNameField>` (lines 716–723). `ColumnHeader` becomes:

```jsx
<div className="px-3 py-2 flex items-center gap-2 rounded">
  <ColumnNameField
    column={column}
    nameFieldDef={nameFieldDef}
    editingName={editingName}
    setEditingName={setEditingName}
  />
  <Badge variant="secondary">{taskCount}</Badge>
  <div className="flex-1" />
  {onAddTask && <AddTaskButton ... />}
</div>
```

2. Remove the `onClickCapture={() => setFocus(columnNameMoniker)}` from the header div — Field's own click handler covers focus-on-click for the field zone, which is exactly the right granularity now.

3. Drop `columnNameMoniker` plumbing through `ColumnHeaderProps` and `ColumnBodyProps` if no other consumer remains. Run a grep first; the same identifier may still be needed elsewhere (e.g. for `setFocus(columnNameMoniker)` calls outside the header).

## Trajectory implication — `column:<id>.name` no longer exists

The outer scope's moniker `column:<id>.name` is referenced by several kernel tests and fixtures as the navigation identity that cards arrow **up to** from the topmost card in their column. Today's expected trajectory:

```
task:T1A → column:TODO.name → column:TODO → ui:perspective-bar → ui:navbar
```

After this change, `column:TODO.name` (as a leaf in the column zone's child set) no longer exists. The remaining same-position entry is `field:column:TODO.name`, registered as a **zone** (because `<Field>` is always a zone). The cascade implication:

- **iter 0** from `task:T1A` (a leaf, parent_zone = `column:TODO`) looking Up: same-kind peers means leaves only. The field zone is not a leaf — not a candidate.
- **iter 1** escalates to `column:TODO` (the parent zone), looks for sibling zones above. Other column zones (`DOING`, `DONE`) are at the same `top` — no Up peer.
- **Drill-out** returns `column:TODO`'s moniker.

So **Up from T1A now lands directly on `column:TODO`**, skipping the column-name slot. The column name is still reachable via `nav.drillIn` on `column:TODO` (the column zone's first child by rect top-left is the field zone). One fewer hop in the trajectory; cleaner mental model that `<Field>` is the primitive.

If keeping the column-name as an explicit nav step matters, the alternative is to make `<Field>` register as a leaf when its display has no spatial children (e.g. a single text input). That's a `<Field>` API change that ripples through many callsites and is out of scope here. Pick this trade-off explicitly in the ticket and update the tests.

## Test surface affected

Existing tests reference `column:<id>.name` as a navigation step:

- `swissarmyhammer-focus/tests/unified_trajectories.rs` — `unified_trajectory_a_up_walks_card_to_header_to_column_to_perspective_bar_to_navbar` (line 121) expects `task:T1A → column:TODO.name → column:TODO → ...`. After this change, the trajectory becomes `task:T1A → column:TODO → ...` (skip the name step). Update the test name + assertions.
- `swissarmyhammer-focus/tests/card_directional_nav.rs` (line 184 — `Moniker::from_string("column:TODO.name")`) — same shape, needs update.
- `swissarmyhammer-focus/tests/navigate.rs` (lines 502, 505, 535) — references `column:A.name`, `column:B.name` for similar trajectory assertions.
- `swissarmyhammer-focus/tests/fixtures/mod.rs` (lines 49, 228–230, 511) — registers the column-name leaves in the realistic-app fixture. The fixture stops registering them.
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` (line 1186) — clicks expect `column:<id>.name` to become focused. Update to expect `field:column:<id>.name`.
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` (line 484) — moniker-prefix exclusion list — drop `column:<id>.name` from the list.
- `kanban-app/ui/src/components/app-layout.test.tsx` (line 513) — exclusion comment, drop the reference.

This is the ticket's main work — touching tests + fixtures so the trajectory stays consistent across the kernel and the React layer.

## Approach

1. Remove the wrap and its `setFocus` plumbing in `column-view.tsx`.
2. Update fixtures in `swissarmyhammer-focus/tests/fixtures/mod.rs` to no longer register `column:<NAME>.name` leaves.
3. Update the kernel trajectory tests to reflect the shorter `task → column → perspective-bar → navbar` chain.
4. Update the click-regression and architecture-guard tests on the React side to expect `field:column:<id>.name` (the moniker `<Field>` registers) where they previously expected `column:<id>.name`.
5. Run the full test suite (`cargo test -p swissarmyhammer-focus` + `bun run test`) and fix any other site that referenced the old moniker.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [ ] `column-view.tsx` no longer renders a `<FocusScope>` around `<ColumnNameField>`. The `ColumnHeader` body is the column header div containing `<ColumnNameField>` directly.
- [ ] No call site in `column-view.tsx` references `columnNameMoniker` for spatial focus. The variable can stay if other consumers exist (e.g., a column-name editor opens from a header-level command), but the spatial-graph entry under that moniker is gone.
- [ ] `useFocusedScope()` after clicking the column-name area in production reports `field:column:<id>.name` (the moniker `<Field>` registers).
- [ ] Up from the topmost card in a column lands on `column:<id>` (the column zone), not on `column:<id>.name`. Pinned by the updated unified-trajectory test.
- [ ] No registry contains a `column:<id>.name` entry — neither in kernel fixtures nor in any production mount. Asserted by the fixture builder no longer producing one and a guard test that walks the production registry post-mount and confirms no moniker matching `^column:.+\.name$` (only the `field:` prefix variant).
- [ ] Existing tests that expected the old moniker are updated to expect `field:column:<id>.name` OR are renamed/removed where the trajectory has structurally changed.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/column-view.column-name-no-redundant-scope.browser.test.tsx` (new file)

- [ ] `column_header_does_not_register_column_name_dot_name_moniker` — mount a `<BoardView>` with one column, snapshot every registered moniker via the spatial actions debug API, assert no entry has a moniker matching `/^column:[^.]+\.name$/`. (Pins that the redundant entry is gone.)
- [ ] `column_header_registers_field_column_name_moniker` — assert exactly one entry with moniker `field:column:<id>.name` exists for that column. (Pins that the Field zone is still there.)
- [ ] `clicking_column_name_focuses_field_zone` — click the rendered column-name text, assert `useFocusedScope()` reports `field:column:<id>.name`.
- [ ] `column_header_div_has_no_onClickCapture_setFocus` — source-level guard that the old `onClickCapture` line is gone (so a future revert is caught at lint time).

Test command: `bun run test:browser column-view.column-name-no-redundant-scope.browser.test.tsx` — all four pass.

### Rust kernel — update existing trajectory tests

- [ ] `swissarmyhammer-focus/tests/unified_trajectories.rs` — rename `unified_trajectory_a_up_walks_card_to_header_to_column_to_perspective_bar_to_navbar` to drop the `header_to_column` step. Update the assertions: Up from T1A goes directly to `column:TODO`, then `column:TODO → ui:perspective-bar`, etc.
- [ ] `swissarmyhammer-focus/tests/card_directional_nav.rs` — update `column:TODO.name` references to reflect the new trajectory.
- [ ] `swissarmyhammer-focus/tests/navigate.rs` — update `column:A.name`, `column:B.name` references; the tests' intent (cross-column Right/Left) shouldn't change, but the moniker stops existing.
- [ ] `swissarmyhammer-focus/tests/fixtures/mod.rs` — `column_name_moniker` and the fixture builder no longer produce `column:<NAME>.name` leaves. Either delete the helper entirely or repoint it at `field:column:<NAME>.name` if any test still wants the field-zone identity.

Test command: `cargo test -p swissarmyhammer-focus` — full crate test suite passes.

### Frontend — update existing tests

- [ ] `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` — line 1186, click-on-column-name assertion: update from `column:<id>.name` to `field:column:<id>.name`.
- [ ] `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` — line 484, moniker-prefix exclusion: drop `column:<id>.name`.
- [ ] `kanban-app/ui/src/components/app-layout.test.tsx` — line 513, exclusion comment: drop reference.
- [ ] `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx` — search for `column:<id>.name` and update.

Test command: `bun run test` — full UI test suite passes.

## Workflow

- Use `/tdd` — write the new browser test asserting no `column:<id>.name` entry exists, watch it fail (the entry is registered today), remove the wrap, watch it pass. Then update the existing tests to reflect the new trajectory.
- Single ticket — one redundancy removal, one trajectory update, but it ripples across many tests because the column-name moniker is referenced widely. Do the test updates in the same PR as the source change so the suite stays green.
- Coordinates with `01KQAY5G8GWP992P0EZ01N7806` (overlap tracing) — once that lands, this exact same-(x, y) overlap would be caught automatically as a `WARN` at app startup. This ticket removes the cause; the tracing ticket would prevent it from coming back unnoticed.

## FQM Refactor Notice (added 2026-04-29)

Coordinate with `01KQD6064G1C1RAXDFPJVT1F46` (path-monikers as spatial keys) before driving this task. Specific updates needed under the new contract:

- The "moniker" strings throughout this task description (e.g., `column:TODO.name`, `field:column:<id>.name`, `column:TODO`) are **segment monikers** under the new contract — what consumers pass to `<FocusZone>` / `<FocusScope>`. The kernel-side identity becomes the fully-qualified path (`/window/board/column:TODO/field:column:TODO.name`).
- Test assertions that walk the registry post-mount (`column-view.column-name-no-redundant-scope.browser.test.tsx`) should match against FQM paths, not segment monikers. The regex `^column:.+\.name$` becomes `^/.+/column:.+\.name$` (or, more usefully, walk the registered entries and check their FQM ends in `column:.+\.name`).
- `Moniker::from_string("column:TODO.name")` in Rust tests becomes `SegmentMoniker::from_string("column:TODO.name")` AND the kernel-side lookup uses the FQM equivalent.
- `setFocus(columnNameMoniker)` callers (already being deleted in this task) — under FQM they would have used `composeFq(useFullyQualifiedMoniker(), segment)`. Confirms this task's deletion direction.
- The "no registry contains a `column:<id>.name` entry" assertion stays valid; it just runs against FQM-keyed entries.

If this task is implemented BEFORE the FQM refactor lands, the kernel work uses today's flat `Moniker`. After the refactor, this task's assertions get a mechanical rename. Pick whichever order is convenient.
