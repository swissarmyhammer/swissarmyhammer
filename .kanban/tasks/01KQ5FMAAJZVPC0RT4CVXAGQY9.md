---
assignees:
- claude-code
depends_on:
- 01KQ5FJ0VXEQZVKHZBN49Q5GFS
position_column: todo
position_ordinal: '8980'
title: 'single-changelog 2/2: read field-level history by replaying store-layer text patches; drop entity-format writes & band-aid skip'
---
#single-changelog #refactor #entity #tech-debt

## Why

After `single-changelog 1/2` (`01KQ5FJ0VXEQZVKHZBN49Q5GFS`), the entity layer no longer writes `ChangeEntry` records to disk. New per-entity JSONL files contain only `swissarmyhammer-store::ChangelogEntry` records (text patches). Old files contain a mix of both shapes — accumulated history.

`EntityContext::read_changelog` and its callers still need to expose field-level change history (the `Vec<ChangeEntry>` shape: `who changed which field from X to Y at time T`). That information is recoverable: walk the store-layer log forward, apply each `forward_patch` to a running text state, parse before/after as `Entity`, diff field maps, synthesize a `ChangeEntry`. O(N) per entity history view, paid only when callers ask.

This card swaps the read implementation. Public API of `EntityContext::read_changelog` is unchanged — same signature, same return type — so callers in `swissarmyhammer-kanban/src/context.rs:716` and `swissarmyhammer-entity/src/cache.rs:727` need no modification.

## Scope: entities, perspectives, views

Audited 2026-04-26 alongside card 1. This card touches **`swissarmyhammer-entity` only**. Each domain has its own changelog reader on its own file shape — they do not share code.

| Domain | Reader | Affected by this card? |
|---|---|---|
| Entity | `swissarmyhammer-entity::changelog::read_changelog` (`changelog.rs:324`), `EntityContext::read_changelog` (`context.rs:939`) | **YES** — this card replaces the implementation while preserving the public API |
| Perspectives | `swissarmyhammer-store::Changelog::read_all` and `find_entry` (used by `StoreHandle::undo`) — perspectives have no `read_changelog` of their own, history is read through the store layer | **NO** — store-layer reader is untouched; perspectives' undo path keeps working unchanged |
| Views | `swissarmyhammer-views::changelog::read_changelog` (`views/changelog.rs:79`) reading `views.jsonl` (`ViewChangeEntry`) | **NO** — different file, different format, different reader, independent crate |

## What

### Replace `read_changelog` with a projecting reader

`swissarmyhammer-entity/src/changelog.rs` `read_changelog` becomes a synthesizing function that produces `Vec<ChangeEntry>` from whatever shape is on disk:

1. Read the file's lines.
2. For each line, classify by shape:
   - `ChangeEntry` (entity-format, legacy on-disk): use as-is. Fast path — the field diff is already there.
   - `swissarmyhammer-store::ChangelogEntry` (text patch): hold for replay.
   - Blank or genuinely malformed: skip with a warning (current behavior).
3. Walk the file in order, maintaining a "current text" cursor. For each store-format record:
   - Apply `forward_patch` to current text → new text.
   - Parse old text and new text as `Entity` (using the entity type's schema — the function already takes a `&Path`; either add an `EntityTypeName` parameter, or have the caller supply the schema, or use a `parse_entity_text` helper that infers the type from the path's parent directory name).
   - Run `diff_entities(&old_entity, &new_entity)` to get `Vec<(String, FieldChange)>`.
   - Construct a `ChangeEntry { id: stored_id_to_change_id(entry.id), timestamp: entry.timestamp, op: store_op_to_string(entry.op), entity_type, entity_id, changes, ... }`.
   - Advance current text to new text.
4. Merge fast-path entries and synthesized entries by timestamp; return.

The function's signature stays `pub async fn read_changelog(path: &Path) -> Result<Vec<ChangeEntry>>`, but it likely needs to know the entity type to parse correctly. Two reasonable options — pick whichever fits the existing callers:

- **(A)** Add a sibling `pub async fn read_changelog_for(entity_type: &EntityTypeName, path: &Path) -> Result<Vec<ChangeEntry>>` and have `EntityContext::read_changelog` (which already knows the type) call it. Leave the original `read_changelog` for back-compat, deprecated, returning entity-format lines only.
- **(B)** Infer entity type from `path.parent().file_name()` (`tasks` → `task`, `tags` → `tag`, etc.). Brittle but no signature churn.

Option (A) is the better engineering choice. Pick it unless implementation reveals a reason not to.

### Helpers

- `swissarmyhammer-entity/src/changelog.rs` — `fn try_parse_entity_format(line: &str) -> Option<ChangeEntry>` and `fn try_parse_store_format(line: &str) -> Option<swissarmyhammer_store::ChangelogEntry>`. Replace the existing `is_store_changelog_line` band-aid with these two predicates.
- `fn replay_store_log(entity_type: &EntityTypeName, store_entries: &[ChangelogEntry]) -> Result<Vec<ChangeEntry>>` — the replay engine. Owns the running-text cursor and YAML/markdown parsing.
- `fn parse_entity_text(entity_type: &EntityTypeName, text: &str, id: &EntityId) -> Result<Entity>` — wrap the existing frontmatter parser so the replay can use it. Likely already exists somewhere — grep `parse_entity_file` in `kanban-app/src/watcher.rs` (deleted in entity-cache 4/4) for the prior form, and the entity layer's existing read path for the canonical implementation.

### Drop the band-aid

Once `read_changelog` projects from store records, the `is_store_changelog_line` skip branch added on 2026-04-26 (`swissarmyhammer-entity/src/changelog.rs`, after `read_changelog`'s match arm) is no longer doing useful work — store-format lines are now first-class input, not malformed. **Delete** `is_store_changelog_line` and the test `read_changelog_silently_skips_store_layer_lines`.

### Mark `append_changelog` deprecated

Add `#[deprecated(note = "single-changelog: write through StoreHandle instead")]` to `pub async fn append_changelog` in `swissarmyhammer-entity/src/changelog.rs:303`. It's still in use by tests as a fixture; deletion is a follow-up cleanup once the test fixtures move to using `StoreHandle` for setup. Don't delete it here — that's scope creep.

## Acceptance criteria

- [ ] `EntityContext::read_changelog("task", id)` for a task with N store-format edits returns N+1 `ChangeEntry` values (1 create + N updates), each with `changes` populated by `diff_entities` against the replayed before/after states.
- [ ] For mixed-shape files (entity-format lines from before card 1, store-format lines from after), `read_changelog` returns the union: legacy lines as-is, replayed synthesized entries from store records, all sorted by timestamp.
- [ ] `is_store_changelog_line` is deleted from `swissarmyhammer-entity/src/changelog.rs`. The skip branch in `read_changelog` is gone.
- [ ] `append_changelog` carries a `#[deprecated]` attribute; build is clean (`-D warnings` is not on, but `cargo build` shows no new warnings outside test code).
- [ ] The kanban app's history pane (whatever consumes `read_changelog` at `swissarmyhammer-kanban/src/context.rs:716`) renders the same content for files written before and after `single-changelog 1/2`. Manual smoke check.
- [ ] No regression in perspectives' or views' history readers (`swissarmyhammer-store::Changelog::read_all`, `swissarmyhammer-views::changelog::read_changelog`) — neither was modified by this card. Sanity-confirmed by green tests in their respective crates.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app -p swissarmyhammer-perspectives -p swissarmyhammer-views` green.

## Tests

- [ ] `swissarmyhammer-entity/src/changelog.rs` — `read_changelog_replays_store_format_to_field_changes`: write a synthetic file containing 3 store-format `ChangelogEntry` records (create + 2 updates with known patches), call `read_changelog_for("task", path)`, assert 3 `ChangeEntry` returned with the right `op`, timestamps, and `changes` field reflecting the YAML diff.
- [ ] `swissarmyhammer-entity/src/changelog.rs` — `read_changelog_handles_mixed_legacy_and_store_lines`: file containing `[entity-line, store-line, entity-line, store-line]`, assert 4 `ChangeEntry` results in timestamp order.
- [ ] `swissarmyhammer-entity/src/changelog.rs` — `read_changelog_replay_handles_create_from_empty`: store record with `op: Create` and `forward_patch` against `""`, assert `ChangeEntry` with all fields surfacing as `FieldChange::Set { value }`.
- [ ] `swissarmyhammer-entity/src/changelog.rs` — `read_changelog_replay_skips_genuinely_malformed_lines`: file with one valid store record + one `"{not json"`, assert 1 entry returned + 1 warning logged. Replaces the deleted `read_changelog_silently_skips_store_layer_lines` (different intent — that one checked the band-aid).
- [ ] `swissarmyhammer-entity/src/context.rs` — `read_changelog_after_three_writes_yields_three_field_diffs`: integration test against `EntityContext`. Write entity 3 times with different titles, call `read_changelog`, assert 3 entries, each with a `changes` containing `("title", FieldChange::TextDiff { ... })`.
- [ ] `swissarmyhammer-entity/src/cache.rs` — verify the computed-field cache (which calls `read_changelog` at `cache.rs:727`) produces the same outputs after this card as before. The existing tests that exercise computed fields should pass unchanged.
- [ ] `cargo nextest run -p swissarmyhammer-entity` green.
- [ ] `cargo nextest run -p kanban-app` green.
- [ ] `cargo nextest run -p swissarmyhammer-perspectives -p swissarmyhammer-views` green (sanity check that scoping held).

## Workflow

`/tdd` — start with `read_changelog_replays_store_format_to_field_changes` against a synthesized fixture file. Implement the replay engine until it passes. Add the mixed-shape and create-from-empty tests next. Then swap `EntityContext::read_changelog` to call the new path and verify the integration test passes. Finally delete `is_store_changelog_line` and rerun the full suite.

## Scope / depends_on

- depends_on: `01KQ5FJ0VXEQZVKHZBN49Q5GFS` (`single-changelog 1/2`) — without it, this card would double-count: legacy entity-format lines being written today + replayed store-format records would yield 2× entries per change.
- Blocks: nothing. After this lands, the only remaining cleanup is deleting `append_changelog` and migrating its test fixtures — small enough to fold into ambient maintenance, no card needed.

## Why this design and not alternatives

- **Why not parameterize `TrackedStore::Change` per the previous turn's draft?** Considered and rejected. Putting field-level structure on the store layer's payload would force views/perspectives/etc. to either invent a `Change` type or carry text patches as a degenerate `Change` — either way, the store layer ends up knowing about its consumers. Replay-on-read keeps the store layer dumb (text in, text out) and confines schema awareness to the entity layer.
- **Why not just delete history?** `EntityContext::read_changelog` has callers in the cache (computed-field inputs) and likely the UI history pane. Removing the capability is a product decision, not a refactor decision.
- **Why is replay cost OK?** Per-entity logs measured 2026-04-26 across 2,123 task files: median ~5 entries, max 26. Replay cost is dominated by the diffy patch apply (microseconds per entry) and YAML parsing (microseconds). Computed-field cache memoizes the result. Worst case is single-digit milliseconds, paid lazily.
