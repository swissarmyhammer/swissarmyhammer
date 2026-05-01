---
assignees: []
position_column: todo
position_ordinal: '8280'
title: 'Fix: inspector edits of non-hardcoded fields don''t update board/card view without refresh'
---
## What

**Bug**: When a user edits a non-hardcoded (custom/schema-defined) field in the EntityInspector, the change does NOT propagate to the board/card view automatically. The user must refresh the page to see the new value. This violates the event architecture (see `feedback_store_event_loop` and `feedback_event_architecture` in memory): every command must produce an event that updates the UI without a round-trip.

Hardcoded task fields (e.g. `title`) may or may not exhibit this — confirm during TDD. The bug is general across entity types when a field is not in the hardcoded "special" list.

### Pipeline map (where to look)

The dispatch → event → UI loop has these hops. Find which one is broken.

1. **Dispatch** — `kanban-app/ui/src/lib/field-update-context.tsx` lines 39–69: `FieldUpdateProvider.updateField()` dispatches `entity.update_field` with `{ entity_type, id, field_name, value }`. Used by `kanban-app/ui/src/components/fields/field.tsx` line 255 (`useFieldValue`) and line 160 (commit handler).
2. **Command handler** — `swissarmyhammer-kanban/src/entity/update_field.rs` lines 147–164: validates field against schema, computed-vs-raw branch, then `entity.set(field_name, value)` and `ectx.write(&entity)`.
3. **Cache event emission** — `swissarmyhammer-entity/src/cache.rs` lines 364–451: `EntityCache::write()` diffs raw fields, emits `EntityEvent::EntityChanged { changes: Vec<FieldChange> }`. Cache test at `cache.rs:1414` confirms it "reports only the changed and added fields" — so custom fields SHOULD be present in `changes`.
4. **Bridge translation** — `kanban-app/src/watcher.rs`:
   - `resolve_entity_changed()` lines 479–511 receives the cache event
   - `build_field_changed_event()` lines 539–554 merges enriched read + raw `changes`
   - `append_computed_changes()` lines 404–442 appends computed fields on top (does NOT strip raw changes — verify this via a unit test)
5. **Tauri event** — `WatchEvent::EntityFieldChanged` → `entity-field-changed` Tauri event.
6. **Frontend listener** — `kanban-app/ui/src/components/rust-engine-container.tsx` lines 360–387 (`handleEntityFieldChanged`) and lines 395–421 (`useEntityEventListeners`). Line 366 is an early return when `changes` is empty.
7. **Store update** — `kanban-app/ui/src/lib/entity-store-context.tsx` lines 70–102 (`FieldSubscriptions.diff()`) — diffs old vs new entities and notifies field subscribers.
8. **Field re-render** — `kanban-app/ui/src/components/fields/field.tsx` line 255: `useFieldValue()` (entity-store-context.tsx lines 216–234) via `useSyncExternalStore`. Cards use this same path — `kanban-app/ui/src/components/entity-card.tsx` lines 200–226 (`CardField` → `Field`).

### Hypotheses to test (in this order)

- **H1 (most likely)**: The raw field change IS in the cache's `changes` vector and IS in the Tauri event, but the frontend store update path has a subtle bug. For example: `setEntitiesFor` in `rust-engine-container.tsx` updates entity fields, but the subscriber diff in `entity-store-context.tsx` compares by reference or misses the change. Write a unit test that fires `entity-field-changed` with a synthetic custom field and asserts `useFieldValue` subscribers fire.
- **H2**: The bridge's `build_field_changed_event()` or `resolve_entity_changed()` drops raw fields under some condition (e.g., when enriched read succeeds and rewrites `changes`). Write a Rust unit test at `kanban-app/src/watcher.rs` that drives a custom-field change through `build_field_changed_event()` and asserts the custom field appears in the output.
- **H3**: Card components are memoized or pull values from a prop-drilled entity snapshot rather than `useFieldValue`, so they never subscribe. Grep card-rendering components for direct `entity.fields[...]` reads.
- **H4**: The inspector uses a different commit path (e.g., debounced via `text-editor.tsx` onChange) that doesn't call `updateField` until blur, and the test isn't waiting long enough — confirm by adding a waitFor in the test.

### Approach

Use `/tdd`. Start by writing two failing tests that together isolate the break:
1. A Rust unit/integration test in `kanban-app/src/watcher.rs` (or a new `watcher_field_propagation.rs`) that dispatches a custom-field update through `KanbanContext` and asserts a `WatchEvent::EntityFieldChanged` is emitted with the custom field in `changes`.
2. A frontend integration test in `kanban-app/ui/src/lib/entity-event-propagation.test.tsx` (extend existing file) that fires a simulated `entity-field-changed` payload containing a custom field and asserts that a mounted `<Field>` component inside a card subscribes via `useFieldValue` and re-renders with the new value — no refetch required.

Fix whichever layer fails first. Re-run both tests green.

## Acceptance Criteria

- [ ] Editing a non-hardcoded field in the EntityInspector for ANY entity type causes the corresponding board/card view to display the new value without any manual refresh.
- [ ] The fix holds for at least three field shapes: a text field, a select/enum field, and a date field (all non-hardcoded).
- [ ] The fix holds for at least two entity types other than `task` (e.g., `tag`, `project`) to confirm it's not a task-only patch.
- [ ] No `invoke("get_entity")` refetch is added to the update path — the field must update purely from the `entity-field-changed` event payload (per `feedback_event_architecture`: "thin events, no enrichment reads").
- [ ] All existing tests in `kanban-app/ui/src/lib/entity-event-propagation.test.tsx`, `kanban-app/ui/src/lib/field-update-context.test.tsx`, and the Rust `update_field.rs` tests still pass.

## Tests

- [ ] **Rust bridge test** — new test in `kanban-app/src/watcher.rs` (or sibling test file): drive a custom-field update (field declared via YAML schema but not in any hardcoded list) through the cache→bridge pipeline; assert `WatchEvent::EntityFieldChanged { changes, .. }` contains a `FieldChange` for that field with the new value. Must fail before the fix.
- [ ] **Frontend integration test** — extend `kanban-app/ui/src/lib/entity-event-propagation.test.tsx`: fire `entity-field-changed` for a custom field on a non-task entity, render a `<Field>` (or minimal card) that subscribes via `useFieldValue`, assert the rendered value changes without any `invoke("get_entity")` call. Must fail before the fix.
- [ ] **Field subscriber diff test** — in `kanban-app/ui/src/lib/entity-store-context.test.tsx` (create if missing): unit test `FieldSubscriptions.diff()` for a custom-field-only change — assert the subscriber is notified exactly once for the changed field.
- [ ] Run `cd kanban-app/ui && bun test` — all green.
- [ ] Run `cargo nextest run -p kanban-app -p swissarmyhammer-kanban` — all green.
- [ ] Manual smoke: `cargo tauri dev` in `kanban-app/`, edit a non-hardcoded field in the inspector, confirm the card updates without refresh. Check `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` for any warnings on the update path.

## Workflow

- Use `/tdd` — write failing tests first (start with the Rust bridge test to bisect backend-vs-frontend), then implement to make them pass.
- Investigate via `/explore` before jumping to a fix — the pipeline has ~8 hops, don't guess.
- Follow `feedback_event_architecture` and `feedback_store_event_loop` from memory. #bug #events #kanban-app #frontend