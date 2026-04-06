---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff80
title: Computed fields (tags, progress) not updated after body edit
---
## What

When a task's fields change (e.g., body edited to add `#tag` patterns or check off `- [x]` items), computed fields (`tags`, `progress`, and any future computed fields) don't update in the UI — neither on autosave nor manual save.

**Root cause:** The watcher (`kanban-app/src/watcher.rs`) reads raw entity files via `cache_file()` → `parse_entity_file()` and diffs them with `diff_fields()`. Computed fields are NOT stored on disk — they're derived at read time by the `ComputeEngine` in `EntityContext`. So when a field changes, the watcher emits `entity-field-changed` with only the raw changed fields. The frontend patches those fields but never receives updated computed field values.

This is a generic problem: ANY computed field that reads from ANY stored field will be stale after updates. The `depends_on` on `FieldType::Computed` tracks entity-type dependencies (e.g., "when a tag entity changes, recompute me"), but field-level dependencies are implicit in the derive function logic. So the fix must be generic — re-derive ALL computed fields for the entity after any field change.

**Fix location:** `flush_and_emit_for_handle()` in `kanban-app/src/commands.rs:1424`. After collecting watcher events, for each `EntityFieldChanged` event, read the entity through `EntityContext` (which runs `ComputeEngine.derive_all()`), compare the computed field values against the raw diff, and append any computed fields that changed to the event's changes array.

**Key files:**
- `kanban-app/src/commands.rs` — `flush_and_emit_for_handle()` (line 1424): enrich watcher events with computed fields
- `kanban-app/src/watcher.rs` — `diff_fields()`, `resolve_change()`, `flush_and_emit()`: produces raw diffs (no changes here)
- `swissarmyhammer-fields/src/compute.rs` — `ComputeEngine::derive_all()`: the generic derivation engine
- `swissarmyhammer-kanban/src/defaults.rs` — `kanban_compute_engine()` (line 89): registers all derive handlers
- `swissarmyhammer-kanban/builtin/definitions/tags.yaml`, `progress.yaml`: field definitions with `kind: computed`

**Approach:**
1. In `flush_and_emit_for_handle`, after watcher produces raw field diffs
2. For each `EntityFieldChanged` event, read the entity via `EntityContext.read()` (which runs `derive_all`)
3. For each computed field in the schema, compare the derived value against what's in the event's changes
4. If a computed field's value differs from what the frontend currently has (i.e., it's not already in the changes array with the correct value), append it
5. This is generic — works for tags, progress, and any future computed fields

## Acceptance Criteria
- [ ] Editing a task body to add `#bug` (where `bug` is an existing tag) updates the `tags` field in the entity inspector without refresh
- [ ] Editing a task body to check off `- [x]` items updates the `progress` field without refresh
- [ ] Both autosave and manual save trigger computed field updates
- [ ] The `entity-field-changed` event includes recomputed values for ALL computed fields when any dependency changes
- [ ] Works generically — no hardcoded field names; any new computed field added in the future gets the same treatment

## Tests
- [ ] `kanban-app/src/commands.rs` — integration test: update a task's body field containing `#tag`, assert the emitted `entity-field-changed` event includes both `body` and `tags` changes
- [ ] `kanban-app/src/commands.rs` — integration test: update a task's body with `- [x]` items, assert `progress` is in the changes
- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — test that `entity-field-changed` with `body` + `tags` changes patches both in the store
- [ ] Run: `cargo nextest run -p kanban-app` and `cd kanban-app/ui && npx vitest run`

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.