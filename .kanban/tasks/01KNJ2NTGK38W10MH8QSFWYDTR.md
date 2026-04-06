---
assignees:
- claude-code
position_column: todo
position_ordinal: 8f80
title: Computed fields (tags, progress) not updated after body edit
---
## What

When a task body is edited (adding `#tag` patterns or checking off `- [x]` items), the computed `tags` and `progress` fields don't update in the UI — neither on autosave nor manual save.

**Root cause:** The watcher (`kanban-app/src/watcher.rs`) reads raw entity files via `cache_file()` → `parse_entity_file()` and diffs them with `diff_fields()`. Computed fields like `tags` (derive: `parse-body-tags`) and `progress` (derive: `parse-body-progress`) are NOT stored on disk — they're derived at read time by the `ComputeEngine` in `EntityContext`. So when body changes, the watcher emits `entity-field-changed` with only `body` in the changes array. The frontend patches `body` but never receives updated `tags` or `progress`.

The fix should happen in `flush_and_emit_for_handle()` (`kanban-app/src/commands.rs:1424`). After the watcher produces raw field diffs, if any changed field is a dependency of a computed field, re-derive that computed field and include it in the emitted changes. The field definitions (`swissarmyhammer-kanban/builtin/definitions/tags.yaml`, `progress.yaml`) declare `type.derive` which maps to handlers in `swissarmyhammer-kanban/src/derive_handlers.rs`. The `FieldsContext::computed_fields_depending_on()` method at `swissarmyhammer-fields/src/context.rs:292` already identifies which computed fields depend on a trigger type.

**Key files:**
- `kanban-app/src/watcher.rs` — `diff_fields()`, `resolve_change()`, `flush_and_emit()`
- `kanban-app/src/commands.rs` — `flush_and_emit_for_handle()` (line 1424)
- `swissarmyhammer-kanban/src/defaults.rs` — `kanban_compute_engine()` (line 89), `parse-body-tags` and `parse-body-progress` derivation handlers
- `swissarmyhammer-fields/src/context.rs` — `computed_fields_depending_on()` (line 292)
- `swissarmyhammer-kanban/src/derive_handlers.rs` — `ParseBodyTags` handler

**Approach:** In `flush_and_emit_for_handle`, after collecting watcher events, for each `EntityFieldChanged` event where `body` is in the changes, read the entity through `EntityContext` (which runs compute), extract the current computed field values (`tags`, `progress`), and append them to the changes array if they differ from the watcher's raw diff.

## Acceptance Criteria
- [ ] Editing a task body to add `#bug` (where `bug` is an existing tag) updates the `tags` field in the entity inspector without a page refresh
- [ ] Editing a task body to check off `- [x]` items updates the `progress` field in the entity inspector without a page refresh
- [ ] Both autosave and manual save trigger the computed field update
- [ ] The `entity-field-changed` event includes computed field values (`tags`, `progress`) when `body` changes

## Tests
- [ ] `kanban-app/src/commands.rs` — integration test: call `entity.update_field` on a task body with `#tag`, assert the emitted `entity-field-changed` event includes both `body` and `tags` in its changes
- [ ] `kanban-app/src/watcher.rs` — unit test: `flush_and_emit` for a task with body containing `#tag` includes recomputed `tags` in field changes
- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — test that an `entity-field-changed` event with `body` and `tags` changes patches both fields in the entity store
- [ ] Run: `cargo nextest run -p kanban-app` and `cd kanban-app/ui && npx vitest run`

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.