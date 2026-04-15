---
assignees:
- claude-code
position_column: todo
position_ordinal: cc80
title: 'Perf: wire swissarmyhammer_entity::EntityCache into KanbanContext so task.move stops reading all task files from disk on every drag-drop'
---
## What

Dragging and dropping a task on a board with 2000 cards takes seconds; the target is &lt;300ms end-to-end so it feels human-instant.

### Root cause — the entity cache is built but not wired up

`swissarmyhammer-entity/src/cache.rs` defines `EntityCache`, an in-memory store keyed by `(entity_type, id)` with:
- `load_all(entity_type)` — bulk-populate from disk on startup (`cache.rs:100`)
- `get(type, id)` / `get_all(type)` — O(1) reads, no disk (`cache.rs:122`, `:137`)
- `write(entity)` — writes through to `EntityContext::write` and updates the cache in lock-step, bumping version + emitting `EntityEvent::EntityChanged` (`cache.rs:152`)
- `refresh_from_disk` / `evict` — points the file watcher already calls on external changes (`cache.rs:228, 244`)
- `EntityWatcher` in `swissarmyhammer-entity/src/watcher.rs:33` — wraps an `Arc<EntityCache>` and routes fs-notify events through it

Everything for "cache all entities in memory, refresh on write or file change" is **implemented** in the entity crate. It just is not plugged into the kanban data path.

`KanbanContext::entity_context()` at `swissarmyhammer-kanban/src/context.rs:381-392` returns a bare `Arc<EntityContext>`, and every `Execute` impl (including `MoveTask::execute` at `swissarmyhammer-kanban/src/task/mv.rs:89`) calls `ectx.list("task")` → `EntityContext::list` at `swissarmyhammer-entity/src/context.rs:581-594` → `io::read_entity_dir` at `swissarmyhammer-entity/src/io.rs:145`. For 2000 files this is tens-to-hundreds of ms of disk I/O plus YAML parse, on every drag-drop.

Meanwhile `kanban-app/src/state.rs:20` imports an *unrelated* type also named `EntityCache` — `kanban_app::watcher::EntityCache` at `kanban-app/src/watcher.rs:174`, which is `Arc<Mutex<HashMap<PathBuf, CachedEntity>>>`, a hash-tracking cache for the notify layer only. It does not store entity content and cannot answer `list`. The collision of names has kept the real cache hidden: the app thinks it has a cache; it only has a change-detector.

### The fix — make `swissarmyhammer_entity::EntityCache` the read/write path

Plug the existing cache in where operations actually execute.

- [ ] Extend `swissarmyhammer-kanban/src/context.rs` so `KanbanContext` holds an `Arc<EntityCache>` alongside (or wrapping) the `EntityContext`. Initialize it in `KanbanContext::entity_context`'s `get_or_try_init` block: construct the `EntityContext`, construct the `EntityCache` around it, and call `cache.load_all("task")`, `cache.load_all("column")`, `cache.load_all("board")`, etc. for every entity type registered in the fields context (reuse the iteration already in `register_entity_stores` at `context.rs:407-429`). Expose a new `KanbanContext::entity_cache() -> Arc<EntityCache>` accessor.
- [ ] Route `EntityContext::list` and `EntityContext::read` through the cache when one is attached. The minimally invasive way: add an `Option<Arc<EntityCache>>` slot on `EntityContext`, a `pub fn with_cache(self, cache: Arc<EntityCache>) -> Self` builder method, and branch in `list`/`read` to call `cache.get_all` / `cache.get` first. Write path continues through `EntityContext::write` → `EntityCache::write` wraps it and updates the map. **Do not** duplicate the cache state — `EntityContext` owns its cache reference, no second copy in `KanbanContext`.
- [ ] Replace the kanban-app's current `watcher::EntityCache` (path→hash map) with the real `swissarmyhammer_entity::EntityCache` + `EntityWatcher` pair. Delete `kanban-app/src/watcher.rs`'s hash-only `EntityCache` type and its `new_entity_cache` helper; update `kanban-app/src/state.rs:20, 78, 149, 212, 261` to hold `Arc<swissarmyhammer_entity::EntityCache>` and start `EntityWatcher::start(kanban_root, cache)` in `AppState::start_watcher`. The existing entity-watcher already drives `refresh_from_disk` / `evict` on external writes.
- [ ] Wire the cache's broadcast channel (`EntityCache::subscribe`, `cache.rs:86-88`) to the existing `WatchEvent` → frontend bridge so the frontend still receives `entity-created` / `entity-field-changed` / `entity-removed`. The app-side watcher currently does `diff_fields` directly from file content (`kanban-app/src/watcher.rs`); with the entity cache in the middle, that diff moves into `EntityCache::refresh_from_disk` (which already compares hashes). Make sure field-level diffs still reach the frontend — likely by having the bridge subscribe to `EntityEvent::EntityChanged` and consult the cache to diff old→new field values. This is the tricky part; budget for it.
- [ ] Add a Criterion benchmark in `swissarmyhammer-kanban/benches/move_task_bench.rs` (new) that seeds a board with 2000 tasks across 4 columns and measures `MoveTask::execute(...).with_before(...)` latency. Before the cache wiring the expected reading is several hundred ms; after wiring it must be &lt;20ms on CI hardware (no disk `list`).
- [ ] Add a targeted test in `swissarmyhammer-kanban/src/task/mv.rs` that asserts `MoveTask::execute` issues **zero** calls to `io::read_entity_dir` on the cached path — use a counting test double on `EntityContext` (or instrument with a tracing span + assertion) so a future regression cannot silently restore the disk scan.

### Out of scope — file follow-up cards

- React-side `cardClaimPredicates` at `kanban-app/ui/src/components/column-view.tsx:299-377` is O(tasks_in_col × tasks_in_adjacent_col). With 500 cards/col that is ~250K iterations per recompute. Not the drag bottleneck today, but track it once the backend is cached.
- Optimistic frontend move in `persistMove` (`kanban-app/ui/src/components/board-view.tsx:333-353`) — separate concern from the disk scan fix.
- Computed-fields re-evaluation on `list`: `EntityContext::list` at `context.rs:586-591` applies compute for every returned entity. The cache should store already-computed entities so repeated reads skip that work too; verify this is what `load_all` ends up holding or add a post-processing step.

## Acceptance Criteria

- [ ] On a seeded 2000-task board, `MoveTask::execute(...).with_before(...)` runs in &lt;20ms in the new benchmark (down from hundreds of ms).
- [ ] `MoveTask::execute` (and any other `Execute` on the drag-drop path) performs zero calls to `io::read_entity_dir` once the cache is populated — asserted in a test that instruments the disk read path.
- [ ] End-to-end: dragging a card on a 2000-task board lands the card in &lt;300ms measured from drop event to UI repaint (manual Chrome DevTools performance trace noted in the PR).
- [ ] `KanbanContext` owns a single `Arc<EntityCache>` that is shared with `EntityContext::list`/`read` and with the `EntityWatcher` file watcher. No duplicate entity state.
- [ ] The kanban-app's local path-hash `EntityCache` type is gone; the app uses `swissarmyhammer_entity::EntityCache` exclusively. The frontend still receives `entity-created` / `entity-field-changed` / `entity-removed` events with the same payload shape.
- [ ] All existing kanban + entity tests stay green: `cargo nextest run -p swissarmyhammer-kanban` and `cargo nextest run -p swissarmyhammer-entity`.

## Tests

- [ ] `swissarmyhammer-kanban/src/task/mv.rs` — add `test_move_task_uses_cache_not_disk`: wrap `EntityContext` in a test double that counts `read_entity_dir` calls, perform 10 moves in sequence, assert the counter stays at the startup `load_all` value (i.e. moves add zero extra disk scans).
- [ ] `swissarmyhammer-kanban/src/context.rs` — add `test_entity_cache_preloads_on_first_use`: construct a `KanbanContext`, call `entity_context()`, then `entity_cache().get_all("task")` and assert all on-disk tasks are returned without any further disk I/O.
- [ ] `swissarmyhammer-kanban/benches/move_task_bench.rs` (new) — Criterion bench seeding 2000 tasks across 4 columns. `cargo bench -p swissarmyhammer-kanban --bench move_task_bench` reports median &lt;20ms.
- [ ] `kanban-app/src/watcher.rs` / its tests — update to exercise the real `EntityCache` event path (cache hash detects change → emits `EntityChanged` → bridge relays as `entity-field-changed`). Existing `diff_fields` tests either move to the bridge layer or are replaced.
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-entity -p kanban-app` — full green across the three affected crates.
- [ ] Manual regression: build the app, load a 2000-task board, drag a card between two others, record a Chrome DevTools performance trace, confirm total drop-to-paint &lt;300ms and no disk activity in fs-usage / Instruments during the drag.

## Workflow
- Use `/tdd` — begin with the failing "no disk scan on move" test in `mv.rs` (it fails against today's code because `ectx.list("task")` reads disk), then write the failing bench, then implement the `EntityContext`-with-cache wiring. The frontend-watcher bridge refactor comes last.

## Size note
This card is larger than typical (~5 files, ~400 lines). It is one concern — "make the existing cache the read path" — but the watcher bridge alone could be a follow-up. If implementation finds the watcher bridge is &gt;100 lines of substantive change, split it into a dependent card `depends_on` this one.
#entity-cache