---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
title: Board watcher ignores .jsonl — external task creates/updates don't propagate to the UI
---
## What

When a task is created or changed by another process (the `kanban` CLI, the in-process MCP server's own `KanbanContext`, an external agent), the open kanban-app board does not pick it up — the new task never appears and external updates never show.

The board *does* run a filesystem watcher: `BoardHandle::open` (`apps/kanban-app/src/state.rs`) calls `KanbanContext::start_watcher`, which starts an `EntityWatcher` (`crates/swissarmyhammer-entity/src/watcher.rs`) watching `.kanban/` recursively. On a recognized file event it calls `EntityCache::refresh_from_disk`, which emits `EntityChanged`; the app's `run_bridge` forwards that to the frontend. `refresh_from_disk` already handles a never-seen id (treats it as new and emits) and the bridge already classifies it as `EntityCreated`.

The break is in `parse_entity_path` (`crates/swissarmyhammer-entity/src/watcher.rs`). It only accepts a watched file as an entity when its extension is `.yaml` or `.md`:

```rust
if extension != "yaml" && extension != "md" {
    return None;
}
```

and there is an explicit test `parse_entity_path_ignores_jsonl` asserting `.jsonl` is ignored.

But on disk **every kanban entity is `<id>.jsonl` plus a companion** — `.md` for tasks, `.yaml` for tags/columns/projects/actors (confirmed: `.kanban/tasks/<ULID>.jsonl` + `<ULID>.md`). The `.jsonl` is the authoritative append-only event log — it holds the task's column, assignees, tags, status, ordinal. The companion `.md` is only the description.

Consequences:
- A task **mutation** (move column, assign, tag, complete, reorder) appends only to `.jsonl`; the `.md` does not change. The watcher ignores the `.jsonl` event entirely, so the change never reaches the cache or the UI.
- Task creation only happens to be caught when the companion `.md` is written; any creation whose observable event is the `.jsonl` (no description companion, or write/event ordering) is missed. The watcher must not depend on the companion file to notice a `.jsonl`-backed entity.

The watcher must treat `.jsonl` as a first-class entity file.

## Approach

In `crates/swissarmyhammer-entity/src/watcher.rs`, `parse_entity_path`: accept `.jsonl` as a valid entity-file extension alongside `.yaml` and `.md`. The id is still the file stem (`<id>.jsonl` → `<id>`). `handle_file_event` then routes `.jsonl` Create/Modify/Remove through `EntityCache::refresh_from_disk` / `evict` exactly as it does for `.yaml`/`.md`.

`refresh_from_disk` is idempotent (it hashes and no-ops when unchanged), so a task firing both a `.jsonl` and a `.md` event in one debounce window simply refreshes once effectively — no double emission of a real change.

Replace the `parse_entity_path_ignores_jsonl` test — it encodes the bug. `.jsonl` under `{type}s/` must now parse to `Some((type, id))`. Keep the existing rejection of unrelated files (`activity/` `.jsonl` changelogs are outside `{type}s/` and already excluded by the 2-component check; verify that still holds).

## Acceptance Criteria
- [ ] `parse_entity_path` returns `Some(("task", "<id>"))` for `.kanban/tasks/<id>.jsonl` (and the same for `.yaml`/`.md` as before).
- [ ] An external write that appends to an existing task's `.jsonl` (e.g. a column move) triggers `refresh_from_disk` and an `EntityChanged` event.
- [ ] An external new-task write (`<id>.jsonl` [+ `<id>.md`]) into a watched `.kanban/tasks/` results in the entity entering the cache and an `EntityChanged` being emitted.
- [ ] Non-entity `.jsonl` files outside `{type}s/` (e.g. `.kanban/activity/*.jsonl`) are still ignored.
- [ ] No change to `refresh_from_disk`, the app `run_bridge`, or the bridge's create/field-change classification.

## Tests
- [ ] In `crates/swissarmyhammer-entity/src/watcher.rs` tests: replace `parse_entity_path_ignores_jsonl` with a test asserting `.kanban/tasks/01ABC.jsonl` parses to `Some(("task", "01ABC"))`.
- [ ] Add a `handle_file_event` test: write a task entity to disk, evict it, fire a `Create` event for the `.jsonl` path, assert the entity is reloaded into the cache.
- [ ] Add a test that a `.jsonl` `Modify` event for an externally-changed task emits `EntityChanged` with the changed field.
- [ ] Assert `.kanban/activity/changelog.jsonl` (or similar non-entity `.jsonl`) still yields `None` from `parse_entity_path`.
- [ ] Extend the kanban-app end-to-end watcher integration tests (`apps/kanban-app/src/watcher.rs`, the integration tests at the bottom of the file) with a case: create a task file externally and assert the bridge emits an `EntityCreated`/`WatchEvent` for it.
- [ ] Run `cargo test -p swissarmyhammer-entity -p kanban-app` and `cargo clippy -p swissarmyhammer-entity -- -D warnings` — all green.

## Workflow
- Use `/tdd` — write the failing `.jsonl` parse + reload tests first, then change `parse_entity_path`.
