---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffc380
title: 'entity-cache 1/4: EntityEvent::EntityChanged carries Vec&lt;FieldChange&gt;; EntityCache computes the diff'
---
#entity-cache

Parent design: `01KP65FJHDQ5R2N5C5BJHVHFBF`. This sub-card moves field-level diffing into the entity layer so the cache's broadcast channel carries the same shape the frontend's `entity-field-changed` Tauri event already consumes.

## What

`swissarmyhammer-entity/src/events.rs` currently emits `EntityChanged { entity_type, id, version }` — coarse, no field detail. The kanban-app watcher re-derives the diff from raw YAML via `diff_fields` at a higher layer. Move the diff into the cache where both the before-image and after-image live anyway, and make it part of the event payload.

There is an existing `changelog::FieldChange` enum in `swissarmyhammer-entity/src/changelog.rs:48-80` with `Set`/`Removed`/`Changed`/`StringChanged` variants — that is the rich shape used for undo/redo. Do **not** reuse it for events; the frontend contract wants a simple `{field, value}`. Introduce a distinct `events::FieldChange` struct.

Files:

- [x] `swissarmyhammer-entity/src/events.rs` — add:
  ```rust
  pub struct FieldChange {
      pub field: String,
      pub value: serde_json::Value,  // Null on removal
  }
  ```
  Change the variant to `EntityChanged { entity_type, id, version, changes: Vec<FieldChange> }`. Keep `serde(tag = "type")` and `Serialize/Deserialize` derives. Document that removals are encoded as `value: Null` (matches the frontend's existing patch semantics).
- [x] `swissarmyhammer-entity/src/cache.rs` — add a private `fn diff(old: Option<&Entity>, new: &Entity) -> Vec<FieldChange>` helper in the cache module (NOT re-exporting from changelog). Logic: walk `new.fields` — if key missing from `old` or value differs, emit `{field: key, value: new_value}`. Walk `old.fields` — if key missing from `new`, emit `{field: key, value: Null}`. For brand-new entities (`old.is_none()`), emit every `new.fields` entry.
- [x] `swissarmyhammer-entity/src/cache.rs` — `EntityCache::write` (`:152-203`): capture the pre-write cached entity as `old` before `self.inner.write`, compute `changes = diff(old.as_ref(), &canonical)`, emit `EntityChanged { changes }` (only when `changed`). The existing `old_hash` probe at `:154-158` still suppresses no-op writes — hash match ⇒ empty changes, no event.
- [x] `swissarmyhammer-entity/src/cache.rs` — `EntityCache::refresh_from_disk` (`:244-274`): same pattern — capture the pre-refresh cached entity, diff against the newly-read one, emit with `changes`.

Subtasks:

- [x] Add `FieldChange` struct + change `EntityChanged` shape in `events.rs`.
- [x] Add private `diff` helper in `cache.rs`.
- [x] Update `EntityCache::write` and `EntityCache::refresh_from_disk` to compute and emit `changes`.
- [x] Update existing `cache.rs` tests that destructure `EntityChanged { entity_type, id, version }` to also handle the `changes` field. Add new diff-specific tests (below).

## Acceptance Criteria

- [x] `EntityEvent::EntityChanged` carries `Vec<FieldChange>` (field + value pairs). No-op writes (hash unchanged) emit no event.
- [x] Removals are encoded as `{field, value: Null}`.
- [x] Brand-new entities (first write, no prior cache entry) emit `changes` containing every field of the new entity.
- [x] `changelog::FieldChange` is untouched — undo/redo tests stay green.
- [x] `cargo nextest run -p swissarmyhammer-entity` passes including new tests.

## Tests

- [x] `swissarmyhammer-entity/src/cache.rs` — `test_entity_changed_carries_field_diff`: write `{a:1, b:2}`; write `{a:1, b:3, c:4}`; receiver sees `EntityChanged { changes }` with entries for `b` and `c` (any order), no entry for `a`.
- [x] `swissarmyhammer-entity/src/cache.rs` — `test_entity_changed_encodes_removal_as_null`: write `{a:1, b:2}`; write `{a:1}`; receiver sees `EntityChanged { changes: [{b, Null}] }`.
- [x] `swissarmyhammer-entity/src/cache.rs` — `test_entity_changed_new_entity_lists_all_fields`: first write of `{a:1, b:2}`; receiver sees `EntityChanged { changes }` containing both `a` and `b` with their values.
- [x] `swissarmyhammer-entity/src/cache.rs` — update `write_emits_entity_changed_event` (`:465-486`), `write_same_content_no_event` (`:510-524`), `versions_monotonically_increasing` (`:527-550`), `refresh_from_disk_emits_event_on_change` (`:553-570`) to destructure the new `changes` field.
- [x] `cargo nextest run -p swissarmyhammer-entity` — full green.

## Workflow
- Use `/tdd` — start with the three new diff-specific tests (they fail because the current event has no `changes` field and won't compile), add the struct + field, then implement the `diff` helper until tests pass.

## Scope / depends_on
- No depends_on.
- Subsequent sub-cards depend on this: `entity-cache 2/4` (context wiring), `entity-cache 4/4` (kanban-app bridge collapse) both consume the new event shape.
