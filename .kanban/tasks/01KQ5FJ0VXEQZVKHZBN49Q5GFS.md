---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
title: 'single-changelog 1/2: stop the entity-layer JSONL writer; store layer becomes the only writer per item'
---
#single-changelog #refactor #entity #tech-debt

## Why

Two writers currently append to the same per-entity JSONL file at `{root}/{type}s/{id}.jsonl`:

1. `swissarmyhammer-store::Changelog::append` writes records carrying `forward_patch` / `reverse_patch` / `item_id` (used by `StoreHandle::undo` / `redo` — `swissarmyhammer-store/src/handle.rs:339`).
2. `swissarmyhammer-entity::changelog::append_changelog` writes records carrying `entity_type` / `entity_id` / `changes` (used today by `EntityContext::read_changelog` for history surfacing and by the cache's computed-field inputs).

The two shapes are mutually unparseable, so `read_changelog` had to grow a band-aid (`is_store_changelog_line`) to skip the other writer's lines without warning. The duplication is also wasteful at runtime: every entity write does two `OpenOptions::open + append + write_all` round-trips.

The architectural answer (agreed in design discussion 2026-04-26): undo/redo stays at the store layer because it's generic across entity / view / perspective. Field-level info is a *projection* of the same text change — derivable, not worth persisting. The entity layer's runtime field-change events already flow through `EntityCache::subscribe()` (delivered in `entity-cache 4/4`, `01KP661D7CDKAAGTR51DX7CHM6`), so the on-disk entity-format log is purely persisted derived data.

This card kills the second writer. The companion card (`single-changelog 2/2`) makes `read_changelog` project field-level history from store records on demand.

## Scope: entities, perspectives, views — what's affected and what isn't

Audited 2026-04-26. The dual-writer-to-the-same-file problem is **entity-only**.

| Domain | Store-layer writer | Domain-layer writer | Same file? | In scope? |
|---|---|---|---|---|
| Entity (task / tag / column / actor / board) | `StoreHandle<EntityTypeStore>` writes `ChangelogEntry` (text patches) to `{type}s/{id}.jsonl` (`swissarmyhammer-store/src/handle.rs:52-58`) | `EntityContext` writes `ChangeEntry` (field diffs) to the same `{type}s/{id}.jsonl` (`swissarmyhammer-entity/src/context.rs:386: path.with_extension("jsonl")`) | **YES** | **YES** — this card fixes it |
| Perspectives | `StoreHandle<PerspectiveStore>` writes `ChangelogEntry` to `perspectives/{id}.jsonl` (`PerspectiveStore::extension() = "yaml"`, store handle puts the changelog beside it) | None — `swissarmyhammer-perspectives` has no `append_changelog` function and no other changelog writer | n/a | **NO** — perspectives already has a single writer |
| Views | None — `swissarmyhammer-views` has no `TrackedStore` impl, doesn't use `StoreHandle` | `swissarmyhammer-views::changelog::append_changelog` writes `ViewChangeEntry` (whole-document `previous`/`current` JSON snapshots) to a single shared `views.jsonl` (`ViewsChangelog::new(root.join("views.jsonl"))` at `swissarmyhammer-kanban/src/context.rs:120`); also implements its own undo via `views/changelog.rs::undo_entry:95` | n/a | **NO** — views has a single writer in a different file with its own undo machinery |

So this card touches `swissarmyhammer-entity` only. Perspectives and views are out of scope; both already have one writer per file.

A separate observation worth recording (not addressed here): views reinvent the changelog/undo wheel instead of using `swissarmyhammer-store`. That's a different smell — *redundant invention* rather than dual-writing — and a candidate for a future refactor card if anyone wants to unify undo/redo across all three domains. Not in the path of `single-changelog`.

## What

Stop calling `swissarmyhammer-entity::changelog::append_changelog` from the entity write path. Every entity mutation already produces a `swissarmyhammer-store::ChangelogEntry` via `StoreHandle::write/delete/archive/unarchive`; the second `ChangeEntry` write is now redundant.

### Production call sites to remove (4)

- `swissarmyhammer-entity/src/context.rs:387` — entity create / update path (the helper that does `path.with_extension("jsonl")`)
- `swissarmyhammer-entity/src/context.rs:461` — second call site
- `swissarmyhammer-entity/src/context.rs:623` — third call site
- `swissarmyhammer-entity/src/context.rs:699` — fourth call site

(Verify by grep at implementation time — the line numbers above are a 2026-04-26 snapshot. Use `grep -nE 'append_changelog\(' swissarmyhammer-entity/src/context.rs` to enumerate them fresh.)

### What stays

- `pub async fn append_changelog(path, entry)` itself stays defined and exported. It's used by tests (`swissarmyhammer-entity/src/changelog.rs` test module, `swissarmyhammer-entity/src/cache.rs:1525, 2107, 2174, 2336`, `context.rs:3437, 3440`) to set up legacy on-disk state. After both `single-changelog` cards land, `single-changelog 2/2` will mark it `#[deprecated]` and the next cleanup pass deletes it.
- `is_store_changelog_line` (added 2026-04-26) stays. Pre-existing entity-format lines remain on disk forever; the reader still has to skip store-format lines until card 2 changes the reader.
- `swissarmyhammer-views::changelog::append_changelog` and `swissarmyhammer-perspectives` are untouched — see scoping table above.

### What about the runtime event path?

`EntityCache::write` already computes the field diff in memory and broadcasts it on the cache's channel — that's how the UI gets `entity-field-changed` today (per `entity-cache 4/4`). Stopping the on-disk write does not affect that channel. Verified by the watcher tests in `kanban-app/src/watcher.rs` (`bridge_end_to_end_*`): they subscribe to the cache, not to the file.

### Backwards compatibility

- Old on-disk entity-format records remain readable via `read_changelog` (no shape change in this card; card 2 changes the reader).
- Mixed files (where some lines are entity-format and some are store-format) keep working — the band-aid skip already handles this.
- Undo/redo continues to work — it reads only the store-format records.

## Acceptance criteria

- [ ] No production code path in `swissarmyhammer-entity` calls `append_changelog`. Verify with `grep -nE 'append_changelog\(' swissarmyhammer-entity/src/*.rs | grep -v 'mod tests\|#\[cfg(test)\]\|#\[test\]\|#\[tokio::test\]'`.
- [ ] After running the kanban app and editing a task, `wc -l .kanban/tasks/{id}.jsonl` grows by exactly 1 per edit (was: 2). Verify by tailing the file before and after a single command.
- [ ] No new lines containing `"changes":[` appear in `.kanban/tasks/*.jsonl` after this card lands. Old lines remain.
- [ ] No regression in perspectives or views: `.kanban/perspectives/*.jsonl` and `.kanban/views.jsonl` continue to be written by their respective single writers, with line counts unchanged from current behavior.
- [ ] Frontend continues to receive `entity-field-changed` events with the same JSON shape — frontend tests in `kanban-app/ui` unchanged and green.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app -p swissarmyhammer-perspectives -p swissarmyhammer-views` green.
- [ ] No regression in undo/redo — `swissarmyhammer-kanban/tests/undo_cross_cutting.rs` passes unchanged.

## Tests

- [ ] `swissarmyhammer-entity/src/context.rs` — add `write_does_not_append_to_entity_changelog`: build an `EntityContext` against a tempdir, write an entity, verify the per-entity `.jsonl` file contains zero `"changes":[` lines (only store-format records). Locks the regression in.
- [ ] `swissarmyhammer-entity/src/context.rs` — add `delete_does_not_append_to_entity_changelog`: same shape, for the delete path.
- [ ] Update `read_changelog`-related tests as needed. The fixture for any test that previously assumed a write produced a `ChangeEntry` line on disk needs to either (a) call `append_changelog` directly to set up legacy state, or (b) be reframed to test card-2 projection (better deferred to that card).
- [ ] `cargo nextest run -p swissarmyhammer-entity` green.
- [ ] `cargo nextest run -p kanban-app` green.
- [ ] `cargo nextest run -p swissarmyhammer-perspectives -p swissarmyhammer-views` green (sanity check that scoping held).

## Workflow

`/tdd` — start with the two regression tests above. They fail today (the writes produce two lines; the test asserts one matches the store format). Remove the four `append_changelog` call sites, watch the tests turn green, run the full nextest, run the app once and tail a `.jsonl` to confirm one-line-per-edit.

## Scope / depends_on

- depends_on: nothing — `entity-cache 4/4` (`01KP661D7CDKAAGTR51DX7CHM6`) already moved UI events off the file-watching path.
- Blocks: `single-changelog 2/2` — that card replaces the reader and depends on this card having silenced the writer first (otherwise the projection would double-count).
