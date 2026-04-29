---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 9e8180
project: spatial-nav
title: Column header registers two spatial primitives for the same surface — collapse the synthetic `column:&lt;id&gt;.name` FocusScope into the inner `<Field>` zone
---
## What

The column header wraps `<ColumnNameField>` in a `<FocusScope moniker="column:<id>.name">` (an entity-namespaced "synthetic navigation leaf"), and `<ColumnNameField>` renders a `<Field>` which is itself a `<FocusZone moniker="field:column:<id>.name">` (`fields/field.tsx:585`). Two spatial-nav primitives, one DOM surface, two kernel registrations, two click handlers, two debug overlays — visible as overlapping dashed borders (emerald scope + blue zone) when the spatial-nav debug visibility is on.

The outer `<FocusScope>` exists today purely to give the column name an entity-namespaced navigation moniker (`column:<id>.name`) separate from the field-namespaced one. The comment at `column-view.tsx:711-715` calls it out explicitly: "`column:<id>.name` is a synthetic navigation leaf wrapping a `<Field>` zone (which itself owns the per-field inspect opt-in)." The architectural guard in `focus-architecture.guards.node.test.ts:482-485` even carves out an `inspect:exempt` carve-out specifically to tolerate this duplicate.

The redundancy is real — the outer FocusScope and the inner Field zone both register with `parentZone = column:<id>` against approximately the same rect (the inner zone is a child of the outer scope's `<div>`). Beam search treats them as distinct candidates. The user observes the duplication once debug overlays make the geometry visible.

## Where this lives

- `kanban-app/ui/src/components/column-view.tsx`
  - `columnNameMoniker = column.moniker + ".name"` at line 584.
  - `<FocusScope moniker={asMoniker(columnNameMoniker)} className="inline">` wrap at line 716–723 (inside `ColumnHeader`).
  - `setFocus(columnNameMoniker)` capture-phase handler at line 709 on the header `<div>`.
  - `ColumnNameField` rendering `<Field fieldDef={nameFieldDef} entityType="column" entityId={column.id} mode="compact" .../>` at lines 668–693.
- `kanban-app/ui/src/components/fields/field.tsx`
  - `<Inspectable moniker={fmk}><FocusZone moniker={fmk} ...>` at lines 583–593, with `fmk = fieldMoniker("column", column.id, "name")` ⇒ `"field:column:<id>.name"`.
  - `<Field>` is documented as "a `<FocusZone>` whose moniker is `field:{type}:{id}.{name}`" (line 15).

## Existing references to `column:<id>.name` (must update or drop)

- `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx:285` — asserts a leaf is registered with moniker `column:col-doing.name` parented at `column:col-doing` zone. After the refactor, the assertion is on the `field:column:col-doing.name` zone (kind change from leaf to zone) parented the same way.
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx:1180–1195` — clicks the column-name surface and asserts `column:<id>.name` gains focus and renders the indicator. After the refactor, the moniker becomes `field:column:<id>.name`.
- `kanban-app/ui/src/components/app-layout.test.tsx:511–518` — excludes `column:<id>.name` from a column-count selector via `[data-moniker^="column:"]:not([data-moniker*="."])`. After the refactor the synthetic moniker no longer exists — the selector simplifies to `[data-moniker^="column:"]` for the column body zones (cards still carry `task:<id>` so no false matches).
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts:482–494` — `inspect:exempt` allow-list comment + scan logic. After the refactor the column-name no longer needs the exemption (the only remaining wrapper IS the `<Field>` zone, which is already paired with `<Inspectable>`).
- `kanban-app/ui/src/test/fixtures/end-to-end-board.ts:24` — comment string mentioning `column.name`. Stale documentation only — no code change needed; update if convenient.

## Approach

### Decision: remove the outer `<FocusScope>` wrap; the inner `<Field>` zone is the sole registration

The inner `<Field>` zone already provides:
- A spatial registration parented at the enclosing column zone (via `useParentZoneKey`).
- A click handler that fires `spatial_focus`.
- An `<Inspectable>` wrap for inspector dispatch (Field handles this internally — for column name, Inspectable's double-click is **not** desired because the column name's `onDoubleClick` should enter edit mode; this is what the existing comment "double-click on the column name routes to the field editor's `onEdit`, not to the inspector" calls out).
  - **Wrinkle**: `<Field>` ALWAYS wraps in `<Inspectable>` (`field.tsx:584`). So removing the outer `<FocusScope>` keeps the `<Inspectable>` wrap. For `field:column:<id>.name`, double-click would dispatch `ui.inspect` for that field-moniker. Verify what the dispatcher does with a `field:` moniker — if it falls through to the column entity, behavior matches the user's expectation; if it does nothing, that's acceptable; if it crashes or opens the wrong inspector, file a separate fix. **Spike this in the failing test before deciding the refactor is safe.**

If the Inspectable double-click on `field:column:<id>.name` is wrong, fall back to:
- **Option B**: add a `disableSpatial?: boolean` prop to `<Field>` so a parent that owns the spatial registration can disable Field's own. The outer `<FocusScope moniker="column:<id>.name">` keeps the navigation identity; the inner Field renders display/editor logic without registering. The `<Inspectable>` wrap inside Field also becomes optional in this mode (parent decides). This is a Field-API expansion.

Default to **Option A** (remove outer wrapper) unless the spike test forces Option B.

### File-by-file changes (Option A)

1. `column-view.tsx`
   - Delete the `<FocusScope moniker={asMoniker(columnNameMoniker)} className="inline">` wrap at lines 716–723; render `<ColumnNameField .../>` directly inside the header `<div>`.
   - Remove the `columnNameMoniker` local at line 584 if unused after the deletion.
   - Remove the `onClickCapture={() => setFocus(columnNameMoniker)}` handler at line 709 — `<Field>`'s own click handler now owns focus dispatch.
   - Remove the `setFocus` prop pulled into `ColumnHeader` if no longer needed elsewhere in the header (the AddTaskButton still uses `setFocus` for the new-task focus jump — keep that path).
   - Remove the `// inspect:exempt — column:<id>.name is a synthetic navigation leaf...` comment block at lines 711–715 (the carve-out is no longer needed).
   - Update the docstring at lines 600–603 ("the column-name `<FocusScope>` leaf in the header") to describe the new shape: "the column-name `<Field>` zone in the header".
2. `column-view.spatial-nav.test.tsx`
   - Update the test at line 273 (`registers the column-name field as a leaf inside the column zone`) to assert a registered ZONE with moniker `field:column:<id>.name` parented at the column zone. Rename to `registers the column-name field zone inside the column zone` for accuracy.
3. `focus-on-click.regression.spatial.test.tsx`
   - Update the column-name regression at lines 1179–1195 to use moniker `field:column:<id>.name` and assert the indicator on that zone's wrapper. Update the parent-monikers list if the test's structural assumption changes.
4. `app-layout.test.tsx`
   - Drop the `:not([data-moniker*="."])` exclusion from the selector at line 516 — the synthetic moniker no longer exists, and column body zones don't carry a dot in their moniker so the simpler selector is exact.
5. `focus-architecture.guards.node.test.ts`
   - Remove the explicit `column:<id>.name` mention from the exemption comment at lines 482–485 if the carve-out is no longer needed at runtime; the `// inspect:exempt` mechanism stays in place for any other consumer that genuinely needs it. Verify by running the guard test against the modified column-view.tsx.

### Sanity check beam-search reachability

The unified-cascade iter-0 same-kind filter (documented in `01KQ7S6WHK9RCCG2R4FN474EFD`) means a leaf-origin search (e.g. from a `task:<id>` card scope) **skips** sibling zones. After the refactor, beam-searching `Up` from the topmost card in a column will not land on the column name (it's now a zone, not a leaf). Per the navbar's percent-complete precedent, this is acceptable — the user reaches the column header by drilling out (Escape → column zone), and from the column zone arrow-nav into the header. Pin this trajectory with a Rust kernel test against a fixture that mirrors the production shape (column zone + column-name field zone + sibling card scopes); assert `Up` from the topmost card lands on the column zone, and `Down` from the column zone lands on the column-name field zone (or first card — verify which the unified cascade picks).

If the trajectory is broken in a way the user notices, file a follow-up task to refine the unified cascade or add a navigation override on the column zone — do not bundle it with this redundancy fix.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [ ] Mounting a column produces exactly ONE spatial-nav registration for the column-name surface — the `field:column:<id>.name` zone — and zero `column:<id>.name` registrations. Pinned by an explicit "no scope is registered with moniker matching `column:<id>.name`" assertion in the column registration test.
- [ ] With debug overlays enabled, the column header shows exactly ONE dashed border around the column name (the blue zone-kind border for the Field zone), not the previous two-overlay overlap.
- [ ] Clicking the column name still moves focus to the column-name surface and renders the focus indicator. Pinned by `focus-on-click.regression.spatial.test.tsx`.
- [ ] Double-click on the column name still enters the field editor's edit mode (via the field's `onEdit` callback wired in `ColumnNameField`). The `<Inspectable>` wrap inside `<Field>` does NOT open the entity inspector for `field:column:<id>.name` — the dispatcher's behavior for a `field:` moniker is either a no-op or already the desired path. Pin this with a unit test that simulates double-click and asserts `setEditingName(true)` is called and `dispatch("ui.inspect")` is **not** called for the column name's field moniker.
- [ ] Pre-existing column-view tests (`column-view.test.tsx`, `column-view.spatial-nav.test.tsx`, `column-view.spatial.test.tsx`, `column-view.scroll-rects.browser.test.tsx`, `column-reorder.browser.test.tsx`, `column-dragover.browser.test.tsx`) all keep passing after the moniker rename.
- [ ] `focus-architecture.guards.node.test.ts` keeps passing without an explicit `column:<id>.name` carve-out (or with the carve-out simplified to remove the column reference).

## Tests

All tests are automated. No manual verification.

### `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx` (modify)

- [ ] Update existing test at line 273 to assert ZONE registration with moniker `field:column:col-doing.name` parented at the column zone. Rename appropriately.
- [ ] Add a new test `does_not_register_a_synthetic_column_name_scope` — assert NO registered scope or zone has the moniker `column:col-doing.name`. Regression guard against accidentally re-adding the duplicate wrapper.

### `kanban-app/ui/src/components/column-name-double-click.test.tsx` (new file)

- [ ] `double_click_enters_edit_mode_not_inspector` — render a single `<ColumnHeader>` with a real `<Field>` inside (not mocked), simulate double-click on the column name, assert `setEditingName(true)` was called, assert `mockDispatch` was NOT called with `"ui.inspect"` for any `field:column:` moniker. Pin the `<Inspectable>` interaction is correct after the wrapping change.

### `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` (modify)

- [ ] Update the "column name leaf" describe at line 1179 to use moniker `field:column:<id>.name`. Rename the describe to "column name field zone" for accuracy. The parent-monikers list updates if the structural ancestry changes.

### `kanban-app/ui/src/components/app-layout.test.tsx` (modify)

- [ ] Drop the `:not([data-moniker*="."])` exclusion from the selector at line 516. Existing column-count assertions still pass.

### `swissarmyhammer-focus/tests/column_header_arrow_nav.rs` (new file or extend an existing fixture-based test)

- [ ] `up_from_topmost_card_lands_on_column_zone_or_field_zone` — fixture: column zone with a sibling column-name field zone and three task scopes. Assert `Direction::Up` from `task:t0` lands on a moniker matching `column:c0` OR `field:column:c0.name` (whichever the unified cascade picks). Pins the trajectory after the column-name surface flipped from leaf to zone.
- [ ] `down_from_column_zone_lands_on_field_zone_or_first_card` — symmetric. Pin which surface the kernel picks.

If the trajectory turns out to be unreachable or surprising, file a separate follow-up — do not block this card on it.

Test commands:
- `cd kanban-app/ui && bun test column-view column-name focus-on-click app-layout focus-architecture` — all green.
- `cargo test -p swissarmyhammer-focus --test column_header_arrow_nav` (if added) — all green.

## Workflow

- Use `/tdd` — start by adding the "no synthetic scope" assertion in `column-view.spatial-nav.test.tsx` (fails today). Then add the double-click safety test (`column-name-double-click.test.tsx`). Then make both green by removing the outer `<FocusScope>` wrap. Update the dependent tests as the moniker rename lands.
- Spike the `<Inspectable>` double-click behavior on a `field:column:<id>.name` moniker FIRST — if it dispatches the wrong inspector, switch to Option B (Field gains a `disableSpatial` prop) and update the task to reflect that pivot.
- Keep the change scoped to the column-name surface. Do not generalise to other entity-namespaced synthetic monikers in this card; if any other call site has the same redundancy, file a follow-up.

## FQM Refactor Notice (added 2026-04-29)

Coordinate with `01KQD6064G1C1RAXDFPJVT1F46` (path-monikers as spatial keys) before driving this task. Specific updates needed under the new contract:

- `columnNameMoniker = column.moniker + ".name"` (string concat) is exactly the construction pattern the FQM refactor obsoletes. Under FQM, the segment passed to `<FocusZone>` / `<FocusScope>` IS just the relative segment (`"column:<id>.name"`), and the kernel-side identity is the FQM (`/window/board/column:<id>/column:<id>.name`) — derived from React context, not string-concatenated by consumers.
- `setFocus(columnNameMoniker)` (which this task already deletes) was a flat-moniker setter. Under FQM, `setFocus` accepts only `FullyQualifiedMoniker`. The deletion in this task aligns with the new contract.
- Test assertions like `data-moniker="field:column:<id>.name"` may need to change to `data-fq-moniker="/window/.../field:column:<id>.name"` if the React layer surfaces the FQM as a data attribute. Verify with the FQM refactor's React adapter implementation.

If this task is implemented BEFORE the FQM refactor lands, the work uses today's flat `Moniker`. After the refactor, segment vs FQM gets a mechanical rename. Either order is fine.

#frontend #spatial-nav #kanban-app