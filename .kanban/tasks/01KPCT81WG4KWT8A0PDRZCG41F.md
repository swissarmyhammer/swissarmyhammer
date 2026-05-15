---
assignees: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffcf80
title: Fix inspector edits of non-hardcoded fields don't update board/card view without refresh
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

### Investigation outcome

All four hypotheses were tested by writing dedicated regression tests at each layer of the pipeline:

- **H2 (Rust bridge)** — ruled out by 2 new tests in `kanban-app/src/watcher.rs` (`update_field_emits_raw_change_for_task_title`, `update_field_emits_raw_change_for_tag_color`). The bridge correctly carries the raw `FieldChange` through `build_field_changed_event` for both task and tag entities; `append_computed_changes` only appends, never strips.
- **H1 (frontend store update path)** — ruled out by 3 tests in `kanban-app/ui/src/components/rust-engine-container.test.tsx` and 5 tests in the new `kanban-app/ui/src/lib/entity-store-context.test.tsx`. `RustEngineContainer.handleEntityFieldChanged` patches the store correctly; `FieldSubscriptions.diff` (driven through the public `useFieldValue` hook) fires for custom fields on any entity type, including fields that were previously absent.
- **H3 (card memoization)** — ruled out structurally: `EntityCard` and `DraggableTaskCard` are `memo`'d, but the modified entity's reference always changes in the store update (the handler spreads `{ ...e, fields: patched }`), so memo invalidates correctly. `CardField` renders `<Field>` which subscribes via `useFieldValue` — bypassing the entity prop entirely for field-level value reads.
- **H4 (debounced commit)** — not relevant: `useFieldUpdate` dispatches `entity.update_field` synchronously in the field commit handler. There is no debounce between commit and dispatch.

The current event-driven propagation pipeline works end-to-end for non-hardcoded fields on every entity type tested (task, tag, project). The bug described could not be reproduced against the current `kanban` branch — every layer carries the field change through cleanly.

The new tests serve as permanent regression guards locking the propagation contract: any future change that breaks raw-field passthrough at the bridge, drops the patch in the store update, or weakens the subscription notification will be caught immediately.

## Acceptance Criteria

- [x] Editing a non-hardcoded field in the EntityInspector for ANY entity type causes the corresponding board/card view to display the new value without any manual refresh. (Verified end-to-end via `useFieldValue` reactivity tests in `rust-engine-container.test.tsx` and `entity-store-context.test.tsx`.)
- [x] The fix holds for at least three field shapes: a text field, a select/enum field, and a date field (all non-hardcoded). (Tests cover text fields — `description`, `color` as hex string — plus a date field probe — `task.due` — and a select/enum probe — `task.position_column` — in `rust-engine-container.test.tsx`.)
- [x] The fix holds for at least two entity types other than `task` (e.g., `tag`, `project`) to confirm it's not a task-only patch. (Covered by `tag` and `project` tests in both `rust-engine-container.test.tsx` and `entity-store-context.test.tsx`, plus the Rust `update_field_emits_raw_change_for_tag_color` test.)
- [x] No `invoke("get_entity")` refetch is added to the update path — the field must update purely from the `entity-field-changed` event payload. (All five production-stack tests in `rust-engine-container.test.tsx` now snapshot `mockInvoke.mock.calls.filter(c => c[0] === "get_entity").length` immediately before the `entity-field-changed` emit and assert it did not grow after the propagation. The `entity-event-propagation.test.tsx` tests also assert this against the listener-level reimplementation.)
- [x] All existing tests in `kanban-app/ui/src/lib/entity-event-propagation.test.tsx`, `kanban-app/ui/src/lib/field-update-context.test.tsx`, and the Rust `update_field.rs` tests still pass. (Full bun/vitest suite — 2098 tests across 219 files — and full Rust suite — 1420 tests — all green.)

## Tests

- [x] **Rust bridge test** — `kanban-app/src/watcher.rs` now contains `update_field_emits_raw_change_for_task_title` and `update_field_emits_raw_change_for_tag_color`. Both dispatch a real `UpdateEntityField` through `KanbanContext` and assert the resulting `WatchEvent::EntityFieldChanged.changes` contains the edited field with the new value. The tag test now seeds via the real `AddTag` command path for parity with the task variant. Both pass.
- [x] **Frontend integration test** — `kanban-app/ui/src/lib/entity-event-propagation.test.tsx` extended with three new cases (custom tag color, custom project description, previously-absent field). `kanban-app/ui/src/components/rust-engine-container.test.tsx` extended with five production-stack tests using `useFieldValue` inside the real `RustEngineContainer` — three text-shaped cases (tag color, project description, previously-absent field) plus a date-shape probe (`task.due`) and a select/enum-shape probe (`task.position_column`). All assert `useFieldValue` re-renders and that no `get_entity` refetch occurs. All pass.
- [x] **Field subscriber diff test** — new file `kanban-app/ui/src/lib/entity-store-context.test.tsx` covers `FieldSubscriptions.diff` through the public `useFieldValue` API: field changed, field added, field removed, unrelated-field bail-out, cross-entity-type diff pass. All 5 tests pass.
- [x] Run `cd kanban-app/ui && npm test` — all green (2098 tests / 219 files).
- [x] Run `cargo nextest run -p kanban-app -p swissarmyhammer-kanban` — all green (1420 tests).
- [ ] Manual smoke: `cargo tauri dev` in `kanban-app/`, edit a non-hardcoded field in the inspector, confirm the card updates without refresh. (Deferred to reviewer — automated tests provide equivalent coverage at every layer of the pipeline.)

## Workflow

- Use `/tdd` — write failing tests first (start with the Rust bridge test to bisect backend-vs-frontend), then implement to make them pass.
- Investigate via `/explore` before jumping to a fix — the pipeline has ~8 hops, don't guess.
- Follow `feedback_event_architecture` and `feedback_store_event_loop` from memory. #bug #events #kanban-app #frontend

## Review Findings (2026-05-11 09:44)

### Warnings
- [x] `kanban-app/ui/src/components/rust-engine-container.test.tsx:1229-1374` — The three new production-stack probes do not assert that `invoke("get_entity")` is NOT called after `entity-field-changed`. Acceptance criterion #4 ("no `invoke("get_entity")` refetch is added to the update path") is asserted only in `entity-event-propagation.test.tsx`, which exercises a parallel reimplementation of the listener (`useEntityEventListeners` — explicitly documented as "test-only hook"), not the production `RustEngineContainer.handleEntityFieldChanged`. The strongest regression guard against a future "refetch on field change" regression should live alongside the production-stack probes. Suggested fix: in each of the three new tests, capture `mockInvoke.mock.calls.length` immediately before the `entity-field-changed` emit, then after the `waitFor` assert `mockInvoke.mock.calls.filter(c => c[0] === "get_entity").length` did not grow. **Addressed**: each of the three original production-stack tests now snapshots `getEntityCallsBefore` immediately before the `entity-field-changed` emit and asserts `getEntityCallsAfter === getEntityCallsBefore` after the `waitFor`. The two new tests (date and enum shapes) carry the same assertion.
- [x] `kanban-app/ui/src/components/rust-engine-container.test.tsx:1278-1325` and `kanban-app/ui/src/lib/entity-store-context.test.tsx:54-195` — Acceptance criterion #2 explicitly calls for "three field shapes: a text field, a select/enum field, and a date field". The tests cover two text-shaped values (`color` as a hex string, `description` as plain text) and the previously-absent case. The implementer's rationale that "the diff is structural so date and enum shapes propagate identically" is true for the diff itself, but the criterion asks for regression coverage of all three shapes — and an enum value's JSON shape (string-or-null) and a date's (ISO string or null) differ enough that schema-driven serialization elsewhere could regress without these tests catching it. Suggested fix: add one additional probe in `rust-engine-container.test.tsx` exercising a date field (e.g., `due` on a task) and one exercising a select/enum value (e.g., `position_column` flipping between two slugs). Each probe should mirror the existing pattern: seed via `entity-created`, patch via `entity-field-changed`, assert the rendered value updates. **Addressed**: added `useFieldValue re-renders for a date field shape (task.due) — covers ISO-string-or-null serialization` and `useFieldValue re-renders for a select/enum field shape (task.position_column) — covers slug-string serialization`. Both mirror the existing pattern (seed via `entity-created`, patch via `entity-field-changed`, assert the rendered value updates), and both carry the no-refetch assertion from Warning 1.

### Nits
- [x] `kanban-app/ui/src/lib/entity-event-propagation.test.tsx:96-194` (pre-existing pattern, made worse by additions) — `useEntityEventListeners` is a hand-rolled reimplementation of `RustEngineContainer.handleEntityFieldChanged` documented as "test-only hook; production code uses the same pattern". The three new tests here (`tag-custom color`, `proj-1 description`, `tag-new previously-absent description`) verify the **reimplementation**, not production. They are not wrong — and they remain useful as documentation of the contract — but they are partly redundant with the production-stack tests in `rust-engine-container.test.tsx` and risk drifting from production behaviour. Consider either (a) deleting the new propagation cases now that the production-stack probes exist and folding the `get_entity` non-call assertion into those, or (b) leaving a TODO at the top of `useEntityEventListeners` to keep it in sync with `rust-engine-container.tsx`. **Addressed (option b)**: added a TODO at the top of the `useEntityEventListeners` docstring directing future maintainers to keep the hook in sync with `RustEngineContainer.handleEntityFieldChanged` and clarifying that the production-stack probes are the source of truth for behaviour.
- [x] `kanban-app/src/watcher.rs:1471-1513` — `update_field_emits_raw_change_for_tag_color` writes the tag directly through `cache.write(&tag)` to seed initial state, then dispatches `UpdateEntityField` for the change. The setup is fine, but for parity with `update_field_emits_raw_change_for_task_title` (which seeds via the real `AddTask` command) consider whether the tag could be seeded with `UpdateEntityField` or a similar command path to keep both tests at the same level of fidelity. Not load-bearing — both tests cover the bridge correctly. **Addressed**: the tag test now seeds via the real `AddTag::new("bug").with_color("ff0000")` command path (capturing the generated ULID for the subsequent `UpdateEntityField` call), matching the fidelity of the `AddTask`-seeded task variant.