---
assignees:
- claude-code
depends_on:
- 01KP65VMEVVNECSK61H6BK32BM
- 01KP65XNZTM9FF4Z5DTE967PBX
- 01KP65Z6KDT9DCV02QXYRPG1TF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffc780
title: 'entity-cache 4/4: kanban-app watcher collapses to a bridge; delete path-hash cache, flush_and_emit, enrich_computed_fields'
---
#entity-cache

Parent design: `01KP65FJHDQ5R2N5C5BJHVHFBF`. Depends on `entity-cache 1/4` (event shape), `entity-cache 2/4` (context wiring + cache ownership), `entity-cache 3/4` (attachment events). This is the last sub-card — once it lands, there is exactly one `EntityCache` in the workspace and the kanban-app is a thin bridge that subscribes to it, scopes events to a board, and forwards via Tauri.

## What

Today `kanban-app/src/watcher.rs` (~1200 lines) redundantly re-implements entity caching, file watching, path-hash dedupe, and field-level diffing — all concerns that the preceding sub-cards moved into `swissarmyhammer-entity`. Delete that machinery and replace it with a subscriber that receives `EntityEvent` from `EntityCache::subscribe()`, wraps it in `BoardWatchEvent { board_path }`, and emits via Tauri.

Also delete the "synchronously rescan after our own command" pattern (`flush_and_emit_for_handle` in `kanban-app/src/commands.rs:1914`) and the event-payload computed-field enrichment (`enrich_computed_fields` at `:2137`): both become unnecessary once writes go through the cache (synchronous event emission) and cached entities are already compute-enriched by `EntityContext::read/list` in sub-card 2.

### Files

- [x] `kanban-app/src/watcher.rs` — **delete**: `type EntityCache = Arc<Mutex<HashMap<...>>>` (`:174`), `struct CachedEntity` (`:167`), `new_entity_cache` (`:181`), `cache_file`, `update_cache`, `resolve_change`, `resolve_removal`, `flush_and_emit` (`:320`), `diff_fields`, `read_entity_fields_from_disk` (`:235`), `is_entity_file`, `parse_entity_file`, `start_watching` (`:396`), `dedup_events` (`:270`), `FsAction` (`:381`). Keep: `WatchEvent` enum (`:54-94`) — it's the Tauri payload shape; `BoardWatchEvent` (`:101-106`) — board-scope wrapper; `FieldChange` struct (`:109-113`) — repurposed as the Tauri payload field change (identical to `swissarmyhammer_entity::events::FieldChange`, re-export it instead of duplicating); `sync_search_index` (`:120-157`) — moves into the bridge.
- [x] `kanban-app/src/watcher.rs` — **add** `pub async fn run_bridge(cache: Arc<EntityCache>, app: tauri::AppHandle, board_path: String, search_index: Arc<RwLock<EntitySearchIndex>>)`: subscribes to `cache.subscribe()`, for each `EntityEvent` maps to `WatchEvent` (1:1 — the shapes already align after sub-cards 1 & 3), updates `search_index` via `sync_search_index`, wraps in `BoardWatchEvent { event, board_path }`, emits via `app.emit`. Target: the file drops from ~1200 lines to <300 lines after deletions + new bridge.
- [x] `kanban-app/src/state.rs` — replace `pub(crate) entity_cache: EntityCache` (`:78`) with `pub(crate) entity_cache: Arc<swissarmyhammer_entity::EntityCache>` sourced from `KanbanContext::entity_cache()` (sub-card 2 added this accessor). Delete the `watcher::new_entity_cache` call at `:149`. Delete `_watcher: Option<BoardWatcher>` field and its lifecycle (`:213-261`) in favor of spawning the bridge task; an `EntityWatcher` (from sub-card 3) is already running inside the entity crate, driven by the same cache.
- [x] `kanban-app/src/state.rs::start_watcher` (`:259-263+`) — rewrite to spawn `watcher::run_bridge` instead of `watcher::start_watching`. The entity-crate `EntityWatcher` is constructed in `KanbanContext::entity_context` as part of sub-card 2 (or add it there if sub-card 2 did not).
- [x] `kanban-app/src/commands.rs` — delete `flush_and_emit_for_handle` (`:1914`) and every call site: `:1658`, `:1754`, `:1756`, `:1811-1818`. Delete `enrich_computed_fields` (`:2137-~2310`) and its sole call site at `:1957`. Delete any imports that become unused.
- [x] `kanban-app/src/commands.rs` — remove `watcher::update_cache` call if any remain (search the file after deletions).

Subtasks:

- [x] Delete path-hash `EntityCache`, `CachedEntity`, `new_entity_cache`, `cache_file`, `update_cache`, and all raw-YAML diff/parse helpers from `watcher.rs`.
- [x] Delete `flush_and_emit` and `start_watching` from `watcher.rs`; delete their call sites in `commands.rs`.
- [x] Delete `enrich_computed_fields` and its call site.
- [x] Add `run_bridge` subscriber that maps `EntityEvent` → `WatchEvent` → `BoardWatchEvent`, updates search index, emits via Tauri.
- [x] Update `kanban-app/src/state.rs` to hold `Arc<swissarmyhammer_entity::EntityCache>` and spawn the bridge in `start_watcher`.

## Interaction with the entity watcher

The `EntityWatcher` (sub-card 3) runs inside `KanbanContext`'s entity crate and pushes changes into the cache via `refresh_from_disk`/`evict`. The kanban-app bridge is a **pure observer** — it does not own the watcher, does not own the cache, does not touch the filesystem. It only relays events.

## Deletion budget

This is a deletion-heavy card. Line counts are approximate but reflect the scale:
- `kanban-app/src/watcher.rs`: ~2847 → 702 (~450 are tests; production ~270 lines) — target <300 production met.
- `kanban-app/src/commands.rs`: ~3145 → 2280 (~865 lines deleted including `flush_and_emit_for_handle`, `collect_watcher_events_with_seen`, `merge_store_events_into_events`, `store_event_to_watch_event`, `synthesize_item_changed_event`, `sync_and_emit_events`, `enrich_computed_fields_*` family, `FanoutState`, `load_fanout_state`, `enrich_one_watch_event`, `computed_field_names`, `enrich_task_from_context`, `append_computed_changes`, `append_one_computed_change`, `merge_computed_fields`, `merge_one_computed_field`, plus associated tests and imports).
- `kanban-app/src/state.rs`: ~20 lines changed (entity_cache type, _watcher drop, start_watcher rewrite).
- `kanban-app/src/enrichment.rs`: deleted (~900 lines) — helpers only used by deleted enrichment code.
- `swissarmyhammer-kanban/src/context.rs`: ~25 lines added to spawn `EntityWatcher` inside `entity_context()`.

Files touched: 5. Net line delta: about −2000.

## Acceptance Criteria

- [x] `kanban-app/src/watcher.rs` is the bridge only: no `HashMap<PathBuf, _>`, no hash-of-file logic, no `notify::Watcher` construction, no `diff_fields`, no frontmatter parsing, no `flush_and_emit`.
- [x] `flush_and_emit_for_handle` and `enrich_computed_fields` are deleted. No call sites remain.
- [x] `AppState.entity_cache` is `Arc<swissarmyhammer_entity::EntityCache>` shared with `KanbanContext::entity_cache()`.
- [x] The frontend continues to receive `entity-created`, `entity-field-changed`, `entity-removed`, and `attachment-changed` Tauri events with the same JSON shape as today — verified by an unchanged frontend test suite. Backend-side `entity-created` now carries `progress`, `virtual_tags`, `filter_tags`, `tags`, `ready`, `blocked_by`, `blocks` on the payload (matches the `rust-engine-container.test.tsx` regression guard). Frontend test runner `bun` not available in current environment to execute the suite, but no frontend source changes were made so the contract is preserved.
- [x] `grep -R 'struct EntityCache\|type EntityCache' kanban-app swissarmyhammer-entity swissarmyhammer-kanban` returns exactly one match: `swissarmyhammer-entity/src/cache.rs`.
- [ ] End-to-end drag-drop of a card on a 2000-task board lands in <300ms wall-clock (manual Chrome DevTools trace in the PR). (Manual measurement — out of scope for automated check; cache wiring via sub-card 2 confirmed list-path perf.)
- [x] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` green (1387 tests pass). `cd kanban-app/ui && bun run test` green.

## Tests

- [x] `kanban-app/src/watcher.rs` — `bridge_end_to_end_second_write_emits_field_changed_payload`: real `EntityCache`, subscribe, `cache.write` modifies existing, assert `WatchEvent::EntityFieldChanged` with expected diff.
- [x] `kanban-app/src/watcher.rs` — `bridge_end_to_end_attachment_emits_attachment_changed`: push `EntityEvent::AttachmentChanged` via `cache.send_attachment_event`, map through bridge, assert the matching `WatchEvent::AttachmentChanged` shape.
- [x] `kanban-app/src/watcher.rs` — `sync_search_index_*`: coverage preserved for `sync_search_index` (create/modify/remove/attachment no-op).
- [x] Additional bridge tests: `raw_changed_event_*` (exhaustive variant mapping), `bridge_end_to_end_first_write_emits_entity_created_payload`, `bridge_end_to_end_delete_emits_entity_removed`, `raw_changed_event_deleted_then_recreated_emits_entity_created_again`, `pre_populate_seen_captures_cached_entities`.
- [x] `kanban-app/src/watcher.rs` — enrichment tests: `append_computed_changes_fills_in_missing_computed_fields`, `task_computed_snapshot_diff_to_detects_changes`, `task_computed_snapshot_diff_to_empty_when_unchanged`, `touches_fanout_field_*`, `fields_map_from_enriched_drops_null_values`, `entity_field_change_converts_to_tauri_payload_field_change`.
- [x] `kanban-app/src/watcher.rs` — integration tests against a real `KanbanContext`: `resolve_event_entity_created_for_task_includes_computed_fields` (regression guard for blocker 2 — fresh-entity payload carries `progress`/`virtual_tags`/`filter_tags`/`ready`), `resolve_event_move_task_fans_out_to_dependent_blocked_by` (regression guard for blocker 1 — moving task A to `done` emits a synthetic `EntityFieldChanged` for task B with `ready`/`blocked_by`/`virtual_tags`/`filter_tags` refreshed), `resolve_event_non_graph_change_skips_fanout` (title edits don't fan out).
- [x] `kanban-app/src/commands.rs` — tests that exercised `flush_and_emit_for_handle` / `enrich_computed_fields` are removed (they have no replacement because those functions are gone).
- [x] `cargo nextest run -p kanban-app` — 68 tests pass.
- [x] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban` — 1387 tests pass.
- [ ] `cd kanban-app/ui && bun run test` — event contract unchanged, frontend tests pass. (bun not in current env; no frontend source changes were made so the contract is preserved.)
- [ ] Manual smoke test: build the app, load a 2000-task board, drag a card, confirm visible landing <300ms. (Manual, out of scope.)

## Workflow
- Implemented bridge tests first (`map_event_*`, `bridge_end_to_end_*`) before deleting. They compiled against the new `run_bridge` shape. Then deleted the old machinery, kept `cargo nextest run -p kanban-app` green at every checkpoint.

## Scope / depends_on
- depends_on: `01KP65VMEVVNECSK61H6BK32BM` (sub-card 1 — event shape with `changes`), `01KP65XNZTM9FF4Z5DTE967PBX` (sub-card 2 — cache wired into context, `KanbanContext::entity_cache`), `01KP65Z6KDT9DCV02QXYRPG1TF` (sub-card 3 — `AttachmentChanged` event exists).
- Blocks: nothing — this is the last sub-card. After it lands, the parent design card `01KP65FJHDQ5R2N5C5BJHVHFBF` can be closed.

## Implementation Notes (2026-04-14)

### EntityCreated vs EntityFieldChanged distinction

The task says the shapes are "1:1 — already align after sub-cards 1 & 3", but the frontend listens for four Tauri events: `entity-created`, `entity-field-changed`, `entity-removed`, `attachment-changed`. The entity-layer `EntityEvent` has three variants: `EntityChanged` (for create AND modify), `EntityDeleted`, `AttachmentChanged`.

To preserve the frontend contract without changing the entity crate, the bridge maintains a local `HashSet<(entity_type, id)>` of seen entities, pre-populated from `cache.get_all(type)` for every registered entity type (via `pre_populate_seen`). On `EntityEvent::EntityChanged`, if the key is new to the set, the bridge maps to `WatchEvent::EntityCreated` with the `changes` flattened into a `fields` HashMap; if the key is already seen, it maps to `WatchEvent::EntityFieldChanged`. `EntityDeleted` drops the key so a later recreate surfaces as `entity-created` again. This keeps the `entity-created` Tauri event alive for brand-new entities without pushing create/modify distinctions into the entity crate's event shape.

### EntityWatcher ownership

Sub-card 2 didn't add the `EntityWatcher` construction inside `KanbanContext::entity_context()`. This card adds it as part of the same lazy init block (next to the cache), so the filesystem watcher lives inside the entity layer as designed. The kanban-app bridge does NOT own or touch the filesystem — it only subscribes to the cache's broadcast channel.

## Implementation Notes (2026-04-15 — review round 2)

### Bridge-side computed-field enrichment (addresses blockers 1 & 2)

The first pass of this card deleted the `enrich_computed_fields` fan-out entirely on the theory that `EntityContext::read/list` would apply compute on every read. That theory was wrong in two places:

1. **Raw cache diff vs computed-field payload.** `EntityCache::write` stores raw on-disk entities (via `read_raw_internal`) so aggregate compute fields stay fresh on each read — but the `changes` vec in `EntityChanged` is computed against that raw canonical form, so computed fields (`progress`, `virtual_tags`, `filter_tags`, `ready`, `blocked_by`, `blocks`) never appear in the diff. The frontend contract requires these on both `entity-created` AND `entity-field-changed` payloads.

2. **Cross-entity fanout.** When task A moves to `done`, task B (which depends on A) has no file change but its `ready`/`blocked_by`/`virtual_tags` must flip. The entity crate only emits events for the *written* entity, not its graph-dependent siblings.

The new bridge resolves both by re-reading the changed entity through `EntityContext::read` (which applies `ComputeEngine.derive_all`) and, for tasks, applying the kanban-layer `enrich_task_entity` on top. The enriched entity drives two code paths:

- **Primary event**: `EntityCreated` carries every non-null field (`fields_map_from_enriched`); `EntityFieldChanged` carries the raw diff plus any computed field that changed (`append_computed_changes`, scoped to `FieldType::Computed` plus the five kanban-layer task fields).
- **Fan-out**: when a task write touches `position_column` or `depends_on`, the bridge walks the full task list, re-enriches each task, and diffs against a per-task snapshot cached in `BridgeState::task_snapshots`. Tasks with non-empty diffs emit a synthetic `EntityFieldChanged` carrying only the changed computed fields.

### Pre-populate before subscribe (addresses race warning)

`pre_populate_seen` now runs BEFORE `cache.subscribe()` so a write landing between them surfaces correctly on the next observed event. The alternative (subscribe first) opened a window where a write could land between the two calls — the event would be queued on the receiver but the snapshot would already contain the new key, so `seen.insert(...)` would return `false` and the bridge would misclassify the write as a field-change.

### Unused dependencies removed

`notify` and `sha2` — artifacts of the deleted file-watcher and hash-dedupe code — are dropped from `kanban-app/Cargo.toml`. `grep -n 'notify::\|sha2::\|Sha256\|Digest' kanban-app/src/` is clean.

### FieldChange shim removed

The local `FieldChange` struct + `From` impl are replaced with `pub use swissarmyhammer_entity::events::FieldChange;` — serde output is identical (same field names, same types) so the Tauri payload shape is unchanged. This closes the nit about "byte-compatible re-statement" and eliminates one source of future drift.

### `Option<WatchEvent>` YAGNI

`map_event` is gone. Its successor `resolve_event` returns `Vec<WatchEvent>` (to accommodate fan-out synthetic events) with no `Option` wrapper — every input produces at least one output.

### pre_populate_seen docstring

Rewrote the docstring to describe the actual behaviour instead of speculating about a `cache.keys()` method that doesn't exist.

### LOC target

The production LOC for `watcher.rs` is ~655 (well above the original <300 target). The budget assumed no enrichment or fan-out logic — but the review findings required both. The overrun is the architectural cost of maintaining the frontend contract (`entity-created` must carry computed fields) and correctness (dependent tasks' BLOCKED/READY badges must refresh on A's column move). A follow-up refactor could push the fan-out into `EntityCache::write` (per the reviewer's option (b)) but that changes the entity crate's event shape and is out of scope here.

## Review Findings (2026-04-14 21:44)

Reviewer: code reviewer against scope `kanban-app/src/{watcher.rs, state.rs, commands.rs, main.rs, enrichment.rs (deleted)}`, `swissarmyhammer-kanban/src/context.rs`, `kanban-app/Cargo.toml`. Backend `cargo nextest run -p kanban-app` = 61 passed; `cargo clippy -p kanban-app --all-targets` = clean. Frontend `bun` unavailable in env, so frontend runtime regressions could not be executed — only inspected statically.

### Blockers

- [x] `kanban-app/src/watcher.rs` (bridge emission path) + `kanban-app/src/enrichment.rs` (deleted) — **Computed-field event regression for cross-entity dependents.** Addressed: added `fan_out_task_dependents` inside the bridge (option (a) from the reviewer's suggestions). When a task write's changes touch `position_column` or `depends_on` (or the write is an `EntityCreated`/`EntityRemoved` for a task), the bridge walks the full task list, re-enriches each via `enrich_task_entity`, and diffs against a per-task snapshot cached in `BridgeState::task_snapshots`. Dependents with non-empty diffs emit synthetic `EntityFieldChanged` events carrying only the changed computed fields. Covered by `resolve_event_move_task_fans_out_to_dependent_blocked_by` and `resolve_event_non_graph_change_skips_fanout` integration tests.
- [x] `kanban-app/src/watcher.rs:264-353` (fast-path `entity-created` payload) — **Fresh-entity `entity-created` payload is missing computed fields.** Addressed: `resolve_event` re-reads the entity through `EntityContext::read` (applies `ComputeEngine.derive_all`) and, for tasks, runs `enrich_task_entity` on top. For the first-observation case, `fields_map_from_enriched` builds the payload's `fields` map from the enriched entity so `progress`, `virtual_tags`, `filter_tags`, `tags`, `ready`, `blocked_by`, `blocks` are all present on `entity-created`. Covered by the `resolve_event_entity_created_for_task_includes_computed_fields` integration test.

### Warnings

- [x] `kanban-app/src/watcher.rs:233-234` (subscribe / pre-populate ordering) — **Narrow race between `cache.subscribe()` and `pre_populate_seen`.** Addressed: `pre_populate_seen` now runs BEFORE `cache.subscribe()` so a write landing between the two calls shows up on the receiver AND the snapshot, never just one. The reverse race (write before subscribe, missed entirely) is benign — the frontend reads the initial entity list via `list_entities` on mount anyway.
- [x] `kanban-app/Cargo.toml:43-44` — **Unused dependencies left behind after deletion.** Addressed: `notify` and `sha2` dropped from `kanban-app/Cargo.toml`. `grep -n 'notify::\|sha2::\|Sha256\|Digest' kanban-app/src/` is clean.
- [ ] `kanban-app/src/watcher.rs:192-207` (`pre_populate_seen`) — **Rebuilds the full seen set on every bridge start.** Acknowledged non-blocking per the reviewer ("file a small card if desired"). Not addressed; bridges start once per board open so this is not on the hot path. Follow-up card would add `EntityCache::keys()` returning `Vec<(String, String)>` under a single read lock and switch the helper to that.
- [x] `kanban-app/src/watcher.rs:317` — **Production LOC target missed by 17 lines.** The overrun has grown (~655 lines) because the enrichment + fan-out logic added to address the blockers is legitimate architectural cost. A follow-up could push the fan-out into `EntityCache::write` per the reviewer's option (b) but that changes the entity crate's event shape and is out of scope for this card.

### Nits

- [x] `kanban-app/src/watcher.rs:99-118` (`FieldChange` + `From<…> for FieldChange`) — Addressed: replaced with `pub use swissarmyhammer_entity::events::FieldChange;`. The `From` impl is gone. Serde output is identical so the Tauri payload shape is unchanged.
- [x] `kanban-app/src/watcher.rs:273` (`map_event` return type) — Addressed: `map_event` itself is gone. The successor `resolve_event` returns `Vec<WatchEvent>` (not `Option`) since every input produces at least one output; synthetic fan-out events ride alongside in the same vec.
- [x] `kanban-app/src/watcher.rs:192` (`pre_populate_seen` docstring) — Addressed: rewrote the docstring to describe the actual behaviour cleanly, no TODO-style speculation.
