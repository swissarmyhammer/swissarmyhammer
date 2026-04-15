---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffd080
title: 'entity-cache 3/4: EntityWatcher absorbs attachment watching; new EntityEvent::AttachmentChanged variant'
---
#entity-cache

Parent design: `01KP65FJHDQ5R2N5C5BJHVHFBF`. This sub-card is independent — runs in parallel with `entity-cache 2/4`. It moves attachment-file watching out of the app layer and into `EntityWatcher` next to the entity file watcher, because watching files for entity side-effects is a filesystem concern that belongs at the entity layer.

## What

The kanban-app's watcher at `kanban-app/src/watcher.rs:86-93` emits `WatchEvent::AttachmentChanged { entity_type, filename, removed }` when files under `{root}/{type}s/.attachments/*` change. `EntityWatcher` at `swissarmyhammer-entity/src/watcher.rs:33-108` ignores those paths — its `parse_entity_path` at `:123-152` rejects anything that isn't a direct `{type}s/{id}.(yaml|md)`.

Extend `EntityWatcher` to recognize attachment paths and emit a new `AttachmentChanged` variant on the same `EntityEvent` broadcast channel. No cache-map side-effects — attachments are not entities, so they do not populate `EntityCache`'s `HashMap<(type, id), CachedEntity>`. They are purely notification.

Files:

- [x] `swissarmyhammer-entity/src/events.rs` — add variant:
  ```rust
  AttachmentChanged {
      entity_type: String,
      filename: String,
      removed: bool,
  }
  ```
  Field names match the existing kanban-app payload exactly so the downstream bridge in sub-card 4 can forward without shape translation.
- [x] `swissarmyhammer-entity/src/watcher.rs` — add a sibling parser `fn parse_attachment_path(root: &Path, path: &Path) -> Option<(String, String)>` that recognizes `{root}/{type}s/.attachments/{filename}`. Path pattern: exactly 3 components relative to root, second component is `.attachments`, first component is `{type}s` (strip trailing `s` for entity_type), third component is any file. Don't filter by extension — attachments can be any type.
- [x] `swissarmyhammer-entity/src/watcher.rs` — update `handle_file_event` at `:155-186` so it first tries `parse_entity_path`; on miss, tries `parse_attachment_path`; on hit, emits `AttachmentChanged` directly on the cache's broadcast sender. `removed` = `!path.exists() || matches!(kind, EventKind::Remove(_))`.
- [x] `swissarmyhammer-entity/src/cache.rs` — add a helper `pub fn send_attachment_event(&self, entity_type: &str, filename: &str, removed: bool)` that sends on `event_sender`. Keep it separate from `write`/`refresh_from_disk` — attachments don't touch the cache map.
- [x] `swissarmyhammer-entity/src/watcher.rs` — widen the directory-scan on startup so `RecursiveMode::Recursive` (`:44`) actually reaches `.attachments/` subdirs (verify this with a test; recursion should already cover it).

Subtasks:

- [x] Add `AttachmentChanged` variant to `EntityEvent`.
- [x] Add `parse_attachment_path` helper alongside `parse_entity_path`.
- [x] Update `handle_file_event` dispatch; add `EntityCache::send_attachment_event`.
- [x] Add watcher tests for create/modify/remove of attachment files.

## Interaction with sub-card 1

Sub-card 1 changes the shape of `EntityChanged`. This sub-card adds a new variant `AttachmentChanged` to the same enum. If both merge out of order there will be a trivial conflict in `events.rs`. That's acceptable — the enum only has three variants total after both land (`EntityChanged`, `EntityDeleted`, `AttachmentChanged`). Either sub-card can ship first; the later one rebases.

Note on actual merge: during implementation the sibling sub-card (1/4) was in-flight and a workspace linter hook reflected its WIP state into events.rs and cache.rs while I was editing. The result is a co-merged state that matches what "both cards landed" would produce. My dispatch in `watcher.rs` and the `send_attachment_event` helper are my work; the `FieldChange`/`changes` additions come from sibling 1/4. Sibling 1/4 will rebase cleanly because the overlap is confined to the enum shape.

## Acceptance Criteria

- [x] `EntityEvent::AttachmentChanged { entity_type, filename, removed }` exists and is emitted on `EntityCache::subscribe()`'s channel.
- [x] Touching `{root}/tasks/.attachments/01ABC-foo.png` produces `AttachmentChanged { entity_type: "task", filename: "01ABC-foo.png", removed: false }`.
- [x] Deleting the same file produces `AttachmentChanged { ..., removed: true }`.
- [x] No entry is inserted into the `EntityCache` map for attachments — `cache.get_all("task")` count is unchanged after attachment events.
- [x] `cargo nextest run -p swissarmyhammer-entity` passes.

## Tests

- [x] `swissarmyhammer-entity/src/watcher.rs` — `test_parse_attachment_path_ok`: `/root/tasks/.attachments/01ABC-foo.png` → `Some(("task", "01ABC-foo.png"))`.
- [x] `swissarmyhammer-entity/src/watcher.rs` — `test_parse_attachment_path_rejects_wrong_depth`: 2 or 4 components → `None`.
- [x] `swissarmyhammer-entity/src/watcher.rs` — `test_attachment_create_emits_event`: start watcher against a temp `.kanban/`, create `tasks/.attachments/x.png`, assert `AttachmentChanged { removed: false }` within the debounce window.
- [x] `swissarmyhammer-entity/src/watcher.rs` — `test_attachment_remove_emits_event`: delete a pre-existing attachment, assert `AttachmentChanged { removed: true }`.
- [x] `swissarmyhammer-entity/src/watcher.rs` — `test_attachment_does_not_populate_cache`: after an attachment event, `cache.get_all("task")` length is unchanged.
- [x] `cargo nextest run -p swissarmyhammer-entity` — full green (281 tests pass, 4 skipped).

Extra coverage added alongside the required tests:
- `test_parse_attachment_path_actor_avatar` — exercises a non-task entity type.
- `test_parse_attachment_path_accepts_any_extension` — pdf/txt/no-extension/multi-dot names.
- `test_parse_attachment_path_rejects_non_attachments_middle` — `.trash`, `.archive`, plain subdir all rejected.
- `test_parse_attachment_path_rejects_non_plural_entity_dir` — single-char or non-plural dir rejected.
- `test_parse_attachment_path_outside_root` — paths outside watcher root rejected.
- `test_parse_entity_path_still_rejects_attachments` — belt-and-suspenders: entity parser still says None for attachment paths.
- `test_handle_attachment_event_create_emits_change` / `_remove_emits_change_with_removed_true` / `_create_missing_file_treated_as_removed` / `_access_kind_is_noop` — unit-level coverage of the dispatch helper.

## Workflow
- Use `/tdd` — write the parse-path tests first (pure function, fast), then the integration watcher tests. Implement `parse_attachment_path` + dispatch, then `send_attachment_event`, then verify through a full-stack watcher test.

## Scope / depends_on
- No depends_on.
- Blocks: `entity-cache 4/4` (kanban-app bridge collapse — which also needs this event to exist so the bridge can forward it as `attachment-changed` Tauri payload).
