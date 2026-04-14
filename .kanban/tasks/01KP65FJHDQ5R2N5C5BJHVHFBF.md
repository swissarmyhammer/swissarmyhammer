---
assignees:
- claude-code
position_column: todo
position_ordinal: cd80
title: 'Architecture: consolidate the two "EntityCache" types — one in-memory entity store at the entity layer, kanban-app becomes a thin bridge'
---
## What

There are two types named `EntityCache` in the tree, neither of which does the full job on its own. Consolidate them into a single cache that lives in `swissarmyhammer-entity` and is the only in-memory representation of entity state. The kanban-app retains only a **bridge** that subscribes to cache events and emits board-scoped events to the Tauri frontend.

## Layering rule (non-negotiable)

**All entity state, change detection, dedupe, diff, and event emission belong to `swissarmyhammer-entity`.** The kanban-app does not observe the filesystem, does not hash files, does not parse frontmatter, does not diff fields, does not dedupe writes. It *subscribes* to already-resolved change events from the entity crate, tags them with a `board_path`, and forwards them to Tauri. Nothing more.

Concretely:

| Concern | Owner |
|---|---|
| In-memory `(type, id) → Entity` map | `swissarmyhammer-entity::EntityCache` |
| Hash-based change detection (is this content different?) | `swissarmyhammer-entity::EntityCache` |
| Dedupe our own writes vs. fs-notify echo | `swissarmyhammer-entity::EntityCache` (implicit via write-through) |
| File-level watching (`.yaml`, `.md`, `.attachments/**`) | `swissarmyhammer-entity::EntityWatcher` |
| Field-level diff — old `Entity` vs. new `Entity` → `Vec<FieldChange>` | `swissarmyhammer-entity::EntityCache::diff` |
| Computed-field enrichment (`created`, `updated`, tag derivations) | `swissarmyhammer-entity::EntityContext` (already does this on read/list) |
| Event broadcast (`EntityChanged`, `EntityDeleted`, `AttachmentChanged`) | `swissarmyhammer-entity::EntityCache::subscribe()` |
| Board-path scoping of events (`BoardWatchEvent { event, board_path }`) | `kanban-app` bridge |
| Tauri emission | `kanban-app` bridge |

If a consumer (MCP server, CLI, code-context, another Tauri app) wants entity events, they subscribe to the entity cache directly. The kanban-app gets nothing special — it just happens to scope by board and wrap in Tauri.

## The two caches today

**Cache A: `swissarmyhammer_entity::cache::EntityCache`** (`swissarmyhammer-entity/src/cache.rs`) — the proper one, already built.

- Stores parsed, computed-enriched `Entity` objects keyed by `(entity_type, id)`.
- `load_all(type)` bulk preload, `get`/`get_all` O(1) reads, `write` write-through, `refresh_from_disk` / `evict` for external changes.
- Broadcasts `EntityEvent::EntityChanged { entity_type, id, version }` and `EntityDeleted` via `tokio::sync::broadcast`.
- Paired with `EntityWatcher` at `swissarmyhammer-entity/src/watcher.rs:33` that turns fs-notify events into cache refresh/evict calls.
- **Not wired into `EntityContext`.** `EntityContext::list` and `::read` hit disk directly; nothing constructs an `EntityCache` in the Kanban data path.

**Cache B: `kanban_app::watcher::EntityCache`** (`kanban-app/src/watcher.rs:174`) — a change-detector living at the wrong layer.

- `Arc<Mutex<HashMap<PathBuf, CachedEntity { hash, raw_fields }>>>`.
- Keyed by *path*, not (type, id). Stores raw YAML/Markdown field maps, not enriched entities.
- Built first; predates Cache A.
- Every one of its jobs belongs at the entity layer, not the app layer:
  1. **Dedupe our own writes.** `update_cache` pre-populates the hash so the next fs-notify event is a no-op. → Belongs in `EntityCache`: write-through makes this implicit (write → cache holds new hash → fs-notify → `refresh_from_disk` sees hash match → no event).
  2. **Field-level diff.** `diff_fields(old, new)` produces `FieldChange[]` for the `entity-field-changed` Tauri event. → Belongs in `EntityCache::diff`: the cache is the only thing that holds both pre-change and post-change state. Diffing raw YAML in the app layer is the wrong level.
  3. **Attachment watching.** Watches `.attachments/` and emits `attachment-changed`. → Belongs in `EntityWatcher`: watching files for entity side-effects is what that watcher is for.
  4. **`flush_and_emit` synchronous post-write.** Rescans + diffs + emits immediately after a command to beat the 200ms debounce. → Disappears entirely: when writes go through `EntityCache::write`, the event is emitted synchronously from the write call. No rescan needed.
  5. **Board scoping.** Wraps events in `BoardWatchEvent { event, board_path }`. → Stays in the app. Genuinely an app concern.
  6. **Computed-field enrichment.** `enrich_computed_fields` (`kanban-app/src/commands.rs:2137`) appends derived fields to events. → Moves to `EntityCache::diff`: the cache stores already-enriched entities (compute runs on `EntityContext::read`/`list`), so the diff naturally includes computed-field deltas.

Cache B exists because Cache A didn't exist when the watcher was written. Fix: move the work to where it belongs; leave only (5) in the app.

## Design

- [ ] **`EntityContext` gains `Option<Arc<EntityCache>>`.** Builder `with_cache(cache)`. `list`/`read` consult the cache first when attached; `write` flows through `EntityCache::write` so the write-through pattern gives us implicit dedupe.
- [ ] **`KanbanContext` owns exactly one `Arc<EntityCache>`.** Constructed in `entity_context()`'s init block, calls `load_all(type)` for every entity type in the fields context (iterate `register_entity_stores` at `context.rs:407-429`), attaches the cache to the `EntityContext`. Exposes `KanbanContext::entity_cache()`.
- [ ] **`EntityEvent::EntityChanged` carries `Vec<FieldChange>`.** Replace `{ entity_type, id, version }` with `{ entity_type, id, version, changes: Vec<FieldChange> }`. `FieldChange { field, value }` moves from `kanban-app/src/watcher.rs:109-113` to `swissarmyhammer-entity/src/events.rs`. The diff is computed inside `EntityCache::write` (before-image from cache, after-image from the write) and `refresh_from_disk` (before-image from cache, after-image from disk re-read). The frontend's `entity-field-changed` Tauri payload shape is unchanged.
- [ ] **`EntityWatcher` absorbs attachment watching.** Add `EntityEvent::AttachmentChanged { entity_type, filename, removed }`. Extend `parse_entity_path` at `swissarmyhammer-entity/src/watcher.rs:123` to recognize `.attachments/**` paths; extend `handle_file_event` at `:155` to emit `AttachmentChanged` without touching the entity cache map.
- [ ] **`kanban-app/src/watcher.rs` collapses to a bridge.** Delete: `EntityCache` type (`:174`), `CachedEntity` (`:167`), `new_entity_cache` (`:181`), `cache_file`, `update_cache`, `resolve_change`, `resolve_removal`, `flush_and_emit`, `diff_fields`, `read_entity_fields_from_disk`, `is_entity_file`, `parse_entity_file`, the full `start_watching` fs-notify implementation. Keep: `BoardWatchEvent`, `WatchEvent` (it's the Tauri payload type — matches `EntityEvent` shape), `sync_search_index` (moves to the bridge subscriber). Add a bridge task that subscribes to `EntityCache::subscribe()`, maps `EntityEvent` → `WatchEvent`, wraps in `BoardWatchEvent { board_path }`, emits via Tauri. Target: from ~1200 lines to &lt;300.
- [ ] **Delete `flush_and_emit` call sites in `kanban-app/src/commands.rs`.** The write-through cache fires events synchronously; the app never needs to "catch up" the debounced watcher.
- [ ] **Delete `enrich_computed_fields` in `kanban-app/src/commands.rs:2137`.** Computed fields are already in cached entities via `EntityContext::read`'s compute step. When `EntityCache::diff` produces `Vec<FieldChange>`, computed-field changes are naturally included.

## Expected shape after

```
swissarmyhammer-entity/
  cache.rs       ← EntityCache: sole in-memory store; hash dedupe; FieldChange diff; event broadcast
  watcher.rs     ← EntityWatcher: sole fs watcher; drives refresh/evict; attachment events
  events.rs      ← EntityEvent { Changed { changes }, Deleted, AttachmentChanged }; FieldChange
  context.rs     ← EntityContext with optional cache; list/read/write go through it

swissarmyhammer-kanban/
  context.rs     ← KanbanContext constructs one EntityCache, load_all on init, shares with EntityContext

kanban-app/
  state.rs       ← holds Arc<EntityCache>, Arc<EntityWatcher>, bridge task
  watcher.rs     ← BoardWatchEvent + bridge only (board-scope + Tauri emit); ~250 lines
  commands.rs    ← flush_and_emit and enrich_computed_fields call sites removed
```

## Acceptance Criteria

- [ ] Exactly one `EntityCache` type in the workspace. `grep -R 'struct EntityCache\|type EntityCache' kanban-app swissarmyhammer-entity swissarmyhammer-kanban` returns the single definition in `swissarmyhammer-entity/src/cache.rs`.
- [ ] `kanban-app` contains no `HashMap<PathBuf, _>`, no hash-of-file logic, no fs-notify `Watcher`, no `diff_fields`, no frontmatter parsing. All of it is in `swissarmyhammer-entity`.
- [ ] `EntityContext::list` and `::read` consult `EntityCache::get_all` / `::get` when a cache is attached — verified by a read-counter test that never increments past the initial `load_all` over 100 `list` calls.
- [ ] Writing an entity with no real field changes emits zero events (hash match). Writing then fs-notify echo also emits zero events (implicit dedupe via write-through).
- [ ] `EntityEvent::EntityChanged` carries `Vec<FieldChange>` including computed-field deltas. The frontend's `entity-field-changed` Tauri payload shape is byte-compatible with today (same JSON schema).
- [ ] `flush_and_emit` and `enrich_computed_fields` and their call sites no longer exist.
- [ ] `MoveTask::execute` on a seeded 2000-task board runs in &lt;20ms in a bench and full drag-drop hits &lt;300ms wall-clock (supersedes `01KP63Z8GGSY3DPRZ4N37PDY0D`).
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` green. `cd kanban-app/ui && bun run test` green.

## Tests

- [ ] `swissarmyhammer-entity/src/cache.rs` — `test_entity_changed_carries_field_diff`: write {a:1, b:2}; write {a:1, b:3, c:4}; subscriber sees `EntityChanged { changes: [{b, 3}, {c, 4}] }`, no entry for `a`.
- [ ] `swissarmyhammer-entity/src/cache.rs` — `test_write_then_fs_notify_echo_dedupes`: call `cache.write(e)`, then `cache.refresh_from_disk("task", e.id)` (simulating the watcher firing on our own write); assert the second call returns `changed=false` and emits no event.
- [ ] `swissarmyhammer-entity/src/watcher.rs` — `test_attachment_event_emitted`: touch `{root}/tasks/.attachments/01ABC-foo.png`, assert `AttachmentChanged` on the cache channel.
- [ ] `swissarmyhammer-kanban/src/context.rs` — `test_list_goes_through_cache`: build `KanbanContext`, one `entity_context()` init (which `load_all`s), 100 `ectx.list("task")` calls, assert the `read_entity_dir` counter is 1.
- [ ] `kanban-app/src/watcher.rs` — `test_bridge_scopes_events_to_board`: send an `EntityEvent::EntityChanged` on a mock channel, assert the Tauri emit receives a `BoardWatchEvent` with the right `board_path` and the `changes` payload passes through unchanged.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` — all green.
- [ ] `cd kanban-app/ui && bun run test` — event contract unchanged, frontend tests green.

## Workflow & size

~7 files, ~1000+ lines after counting deletions. One concern ("the entity layer owns entity state; the app is a bridge") but fans across three crates. Use `/plan` to split into implementation sub-cards:

1. **Event shape migration**: `EntityEvent::EntityChanged` gains `Vec<FieldChange>`; `EntityCache::diff` in the entity crate.
2. **Context wiring**: `EntityContext::with_cache`; `KanbanContext` builds and attaches the cache with `load_all`.
3. **Attachment watching absorbed by `EntityWatcher`**.
4. **Kanban-app watcher collapses to bridge**; delete path-hash cache, `flush_and_emit`, `enrich_computed_fields`.

Each sub-card uses `/tdd`: start with the failing read-counter / dedupe test, then implement. Close `01KP63Z8GGSY3DPRZ4N37PDY0D` (drag-perf card) as subsumed by sub-card 2 — its bench becomes a verification criterion there.
#entity-cache