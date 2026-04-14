---
assignees:
- claude-code
depends_on:
- 01KP65VMEVVNECSK61H6BK32BM
- 01KP65XNZTM9FF4Z5DTE967PBX
- 01KP65Z6KDT9DCV02QXYRPG1TF
position_column: todo
position_ordinal: d180
title: 'entity-cache 4/4: kanban-app watcher collapses to a bridge; delete path-hash cache, flush_and_emit, enrich_computed_fields'
---
#entity-cache

Parent design: `01KP65FJHDQ5R2N5C5BJHVHFBF`. Depends on `entity-cache 1/4` (event shape), `entity-cache 2/4` (context wiring + cache ownership), `entity-cache 3/4` (attachment events). This is the last sub-card — once it lands, there is exactly one `EntityCache` in the workspace and the kanban-app is a thin bridge that subscribes to it, scopes events to a board, and forwards via Tauri.

## What

Today `kanban-app/src/watcher.rs` (~1200 lines) redundantly re-implements entity caching, file watching, path-hash dedupe, and field-level diffing — all concerns that the preceding sub-cards moved into `swissarmyhammer-entity`. Delete that machinery and replace it with a subscriber that receives `EntityEvent` from `EntityCache::subscribe()`, wraps it in `BoardWatchEvent { board_path }`, and emits via Tauri.

Also delete the "synchronously rescan after our own command" pattern (`flush_and_emit_for_handle` in `kanban-app/src/commands.rs:1914`) and the event-payload computed-field enrichment (`enrich_computed_fields` at `:2137`): both become unnecessary once writes go through the cache (synchronous event emission) and cached entities are already compute-enriched by `EntityContext::read/list` in sub-card 2.

### Files

- [ ] `kanban-app/src/watcher.rs` — **delete**: `type EntityCache = Arc<Mutex<HashMap<...>>>` (`:174`), `struct CachedEntity` (`:167`), `new_entity_cache` (`:181`), `cache_file`, `update_cache`, `resolve_change`, `resolve_removal`, `flush_and_emit` (`:320`), `diff_fields`, `read_entity_fields_from_disk` (`:235`), `is_entity_file`, `parse_entity_file`, `start_watching` (`:396`), `dedup_events` (`:270`), `FsAction` (`:381`). Keep: `WatchEvent` enum (`:54-94`) — it's the Tauri payload shape; `BoardWatchEvent` (`:101-106`) — board-scope wrapper; `FieldChange` struct (`:109-113`) — repurposed as the Tauri payload field change (identical to `swissarmyhammer_entity::events::FieldChange`, re-export it instead of duplicating); `sync_search_index` (`:120-157`) — moves into the bridge.
- [ ] `kanban-app/src/watcher.rs` — **add** `pub async fn run_bridge(cache: Arc<EntityCache>, app: tauri::AppHandle, board_path: String, search_index: Arc<RwLock<EntitySearchIndex>>)`: subscribes to `cache.subscribe()`, for each `EntityEvent` maps to `WatchEvent` (1:1 — the shapes already align after sub-cards 1 & 3), updates `search_index` via `sync_search_index`, wraps in `BoardWatchEvent { event, board_path }`, emits via `app.emit`. Target: the file drops from ~1200 lines to &lt;300 lines after deletions + new bridge.
- [ ] `kanban-app/src/state.rs` — replace `pub(crate) entity_cache: EntityCache` (`:78`) with `pub(crate) entity_cache: Arc<swissarmyhammer_entity::EntityCache>` sourced from `KanbanContext::entity_cache()` (sub-card 2 added this accessor). Delete the `watcher::new_entity_cache` call at `:149`. Delete `_watcher: Option<BoardWatcher>` field and its lifecycle (`:213-261`) in favor of spawning the bridge task; an `EntityWatcher` (from sub-card 3) is already running inside the entity crate, driven by the same cache.
- [ ] `kanban-app/src/state.rs::start_watcher` (`:259-263+`) — rewrite to spawn `watcher::run_bridge` instead of `watcher::start_watching`. The entity-crate `EntityWatcher` is constructed in `KanbanContext::entity_context` as part of sub-card 2 (or add it there if sub-card 2 did not).
- [ ] `kanban-app/src/commands.rs` — delete `flush_and_emit_for_handle` (`:1914`) and every call site: `:1658`, `:1754`, `:1756`, `:1811-1818`. Delete `enrich_computed_fields` (`:2137-~2310`) and its sole call site at `:1957`. Delete any imports that become unused.
- [ ] `kanban-app/src/commands.rs` — remove `watcher::update_cache` call if any remain (search the file after deletions).

Subtasks:

- [ ] Delete path-hash `EntityCache`, `CachedEntity`, `new_entity_cache`, `cache_file`, `update_cache`, and all raw-YAML diff/parse helpers from `watcher.rs`.
- [ ] Delete `flush_and_emit` and `start_watching` from `watcher.rs`; delete their call sites in `commands.rs`.
- [ ] Delete `enrich_computed_fields` and its call site.
- [ ] Add `run_bridge` subscriber that maps `EntityEvent` → `WatchEvent` → `BoardWatchEvent`, updates search index, emits via Tauri.
- [ ] Update `kanban-app/src/state.rs` to hold `Arc<swissarmyhammer_entity::EntityCache>` and spawn the bridge in `start_watcher`.

## Interaction with the entity watcher

The `EntityWatcher` (sub-card 3) runs inside `KanbanContext`'s entity crate and pushes changes into the cache via `refresh_from_disk`/`evict`. The kanban-app bridge is a **pure observer** — it does not own the watcher, does not own the cache, does not touch the filesystem. It only relays events.

## Deletion budget

This is a deletion-heavy card. Line counts are approximate but reflect the scale:
- `kanban-app/src/watcher.rs`: ~1200 → &lt;300 (~900 lines deleted, ~100 added for bridge)
- `kanban-app/src/commands.rs`: ~170 lines deleted (`flush_and_emit_for_handle` + `enrich_computed_fields` + call sites)
- `kanban-app/src/state.rs`: ~20 lines changed

Files touched: 3. Net line delta: about −1000. Still one concern; stays within the card budget because deletions are cheap.

## Acceptance Criteria

- [ ] `kanban-app/src/watcher.rs` is the bridge only: no `HashMap<PathBuf, _>`, no hash-of-file logic, no `notify::Watcher` construction, no `diff_fields`, no frontmatter parsing, no `flush_and_emit`.
- [ ] `flush_and_emit_for_handle` and `enrich_computed_fields` are deleted. No call sites remain.
- [ ] `AppState.entity_cache` is `Arc<swissarmyhammer_entity::EntityCache>` shared with `KanbanContext::entity_cache()`.
- [ ] The frontend continues to receive `entity-created`, `entity-field-changed`, `entity-removed`, and `attachment-changed` Tauri events with the same JSON shape as today — verified by an unchanged frontend test suite.
- [ ] `grep -R 'struct EntityCache\|type EntityCache' kanban-app swissarmyhammer-entity swissarmyhammer-kanban` returns exactly one match: `swissarmyhammer-entity/src/cache.rs`.
- [ ] End-to-end drag-drop of a card on a 2000-task board lands in &lt;300ms wall-clock (manual Chrome DevTools trace in the PR).
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` green. `cd kanban-app/ui && bun run test` green.

## Tests

- [ ] `kanban-app/src/watcher.rs` — `test_bridge_relays_entity_changed`: build a real `EntityCache`, spawn `run_bridge` with a channel-backed fake Tauri emitter, `cache.write(entity)`, assert the emitter received `BoardWatchEvent { event: WatchEvent::EntityFieldChanged { changes }, board_path }` with the expected changes and correct board path.
- [ ] `kanban-app/src/watcher.rs` — `test_bridge_relays_attachment_changed`: push `EntityEvent::AttachmentChanged` on the cache channel, assert the bridge emits `WatchEvent::AttachmentChanged` with the same fields and the board_path wrapper.
- [ ] `kanban-app/src/watcher.rs` — `test_bridge_updates_search_index`: send an `EntityChanged`, assert the shared `EntitySearchIndex` is patched with the new field values (moves the coverage of today's `sync_search_index` tests).
- [ ] `kanban-app/src/commands.rs` — remove the tests that exercised `flush_and_emit_for_handle` / `enrich_computed_fields` directly; they have no replacement because those functions are gone.
- [ ] `cargo nextest run -p kanban-app` — full green.
- [ ] `cd kanban-app/ui && bun run test` — event contract unchanged, frontend tests pass.
- [ ] Manual smoke test: build the app, load a 2000-task board, drag a card, confirm visible landing &lt;300ms.

## Workflow
- Use `/tdd` — start with the bridge tests (`test_bridge_relays_entity_changed` and `test_bridge_relays_attachment_changed`) before deleting. They fail initially because `run_bridge` doesn't exist. Implement `run_bridge`, confirm green, then begin the deletion sweep. Keep `cargo nextest run -p kanban-app` green at every checkpoint during deletion so it's obvious when something still depends on a doomed helper.

## Scope / depends_on
- depends_on: `01KP65VMEVVNECSK61H6BK32BM` (sub-card 1 — event shape with `changes`), `01KP65XNZTM9FF4Z5DTE967PBX` (sub-card 2 — cache wired into context, `KanbanContext::entity_cache`), `01KP65Z6KDT9DCV02QXYRPG1TF` (sub-card 3 — `AttachmentChanged` event exists).
- Blocks: nothing — this is the last sub-card. After it lands, the parent design card `01KP65FJHDQ5R2N5C5BJHVHFBF` can be closed.
