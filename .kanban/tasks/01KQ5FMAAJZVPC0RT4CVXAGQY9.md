---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '80'
project: single-changelog
title: 'single-changelog: project field-level entity history by replaying store-layer text patches (lands first)'
---
#single-changelog #refactor #entity #tech-debt

## Goal of the parent initiative

`done` = unified storage for the workspace: one writer per file, one changelog format on disk (`swissarmyhammer-store::ChangelogEntry`, text patches via diffy), one global undo stack (`undo_stack.yaml`), one diff/projection mechanism for surfacing field-level history. No parallel undo systems, no orphan log code, no orphan on-disk data.

This card is the read-side foundation. It must land **before** the entity dual-writer is removed, because the existing reader returns no field-level entries for store-format records — silencing the duplicate writer with the old reader in place would blank out the history pane and computed-field cache for every new edit.

## Why this is foundational

The store layer's `ChangelogEntry` already carries `forward_patch` + `reverse_patch`. From those plus the entity schema, every "title changed from X to Y" projection is recoverable: walk the log forward, apply each patch to a running text cursor, parse before/after as `Entity`, diff field maps. O(N) per entity history view, paid only when a caller asks. Median per-entity log measured 2026-04-26 across 2,123 task files: ~5 entries; max: 26.

Once this lands, the entity-layer changelog becomes redundant — that's the next card.

## What

### Replace `swissarmyhammer-entity/src/changelog.rs::read_changelog`

Become a synthesizing reader that produces `Vec<ChangeEntry>` from whatever shape is on disk:

1. Read the file.
2. Per line, classify:
   - `ChangeEntry` (entity-format, legacy): use as-is. Fast path.
   - `swissarmyhammer-store::ChangelogEntry` (text patch): hold for replay.
   - Blank or genuinely malformed: warn + skip.
3. Walk in file order with a "current text" cursor. Each store record:
   - Apply `forward_patch` to current text → new text.
   - Parse old + new text as `Entity` using the entity-type's schema.
   - `diff_entities(&old_entity, &new_entity)` → `Vec<(String, FieldChange)>`.
   - Synthesize `ChangeEntry { id, timestamp: store_entry.timestamp, op: store_op_to_string(store_entry.op), entity_type, entity_id, changes }`.
   - Advance cursor.
4. Merge fast-path + synthesized entries by timestamp; return.

### Signature

`read_changelog` needs the entity type to parse correctly. Add `pub async fn read_changelog_for(entity_type: &EntityTypeName, path: &Path) -> Result<Vec<ChangeEntry>>` and have `EntityContext::read_changelog` call it (the context already knows the type). Keep the old `read_changelog(path)` returning entity-format-only results for any caller that lacks the schema, marked deprecated.

### Helpers to add

- `fn try_parse_entity_format(line: &str) -> Option<ChangeEntry>`
- `fn try_parse_store_format(line: &str) -> Option<swissarmyhammer_store::ChangelogEntry>`
- `fn replay_store_log(entity_type: &EntityTypeName, store_entries: &[ChangelogEntry]) -> Result<Vec<ChangeEntry>>`
- `fn parse_entity_text(entity_type: &EntityTypeName, text: &str, id: &EntityId) -> Result<Entity>` — wrap the existing frontmatter parser.

### Drop the band-aid added 2026-04-26

The skip branch `if is_store_changelog_line(line) { continue; }` and the `is_store_changelog_line` helper become obsolete the moment store-format lines are first-class input. Delete both, and the `read_changelog_silently_skips_store_layer_lines` test that validated them. Replace with the new replay tests below.

## Acceptance

- [ ] `EntityContext::read_changelog("task", id)` for a task with N store-format edits returns N+1 `ChangeEntry` values (1 create + N updates). Each carries `changes` populated by `diff_entities` against the replayed before/after states.
- [ ] Mixed-shape files (entity-format lines from before `single-changelog 2/?`, store-format lines after) produce the union, sorted by timestamp.
- [ ] `is_store_changelog_line` and its test are deleted.
- [ ] Computed-field cache inputs (`swissarmyhammer-entity/src/cache.rs:727`) produce the same outputs after this card as before. Existing computed-field tests pass unchanged.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` green.

## Tests

- [ ] `read_changelog_replays_store_format_to_field_changes` (changelog.rs): synthetic file with 3 store-format records (create + 2 updates), assert 3 `ChangeEntry`s with correct `op`, timestamps, and `changes`.
- [ ] `read_changelog_handles_mixed_legacy_and_store_lines` (changelog.rs): `[entity-line, store-line, entity-line, store-line]`, assert 4 results in timestamp order.
- [ ] `read_changelog_replay_handles_create_from_empty` (changelog.rs): store record with `op: Create` and `forward_patch` against `""`, assert all fields surface as `FieldChange::Set { value }`.
- [ ] `read_changelog_replay_skips_genuinely_malformed_lines` (changelog.rs): valid store record + `"{not json"`, assert 1 entry + 1 warning. Replaces the deleted band-aid test.
- [ ] `read_changelog_after_three_writes_yields_three_field_diffs` (context.rs integration): `EntityContext::write` 3 times with different titles, call `read_changelog`, assert 3 entries each with `("title", FieldChange::TextDiff { ... })`.

## Workflow

`/tdd`. Synthetic-fixture tests first; build the replay engine until they pass. Add the integration test next. Then swap `EntityContext::read_changelog` to the new path and delete the band-aid.

## Scope

- depends_on: nothing.
- Blocks: the writer-off card (`01KQ5FJ0VXEQZVKHZBN49Q5GFS`) — it depends on this card landing first so new edits remain readable.