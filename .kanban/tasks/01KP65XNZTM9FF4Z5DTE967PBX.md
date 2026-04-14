---
assignees:
- claude-code
depends_on:
- 01KP65VMEVVNECSK61H6BK32BM
position_column: todo
position_ordinal: cf80
title: 'entity-cache 2/4: wire EntityCache into EntityContext::list/read/write; KanbanContext preloads on init; supersedes drag-perf card'
---
#entity-cache

Parent design: `01KP65FJHDQ5R2N5C5BJHVHFBF`. Depends on `entity-cache 1/4` (event shape). This sub-card wires the real cache into the actual data path so `ectx.list("task")` stops hitting disk and drag-drop on a 2000-task board hits the &lt;300ms budget.

## What

`EntityContext` at `swissarmyhammer-entity/src/context.rs:28-41` owns no cache today. Attach an optional `EntityCache` and route reads/writes through it. `KanbanContext` at `swissarmyhammer-kanban/src/context.rs:381-392` constructs exactly one cache per kanban root, `load_all`'s every registered entity type on first `entity_context()` call, and attaches it to the `EntityContext` before handing out the `Arc`.

### Avoiding the write-through recursion

`EntityCache::write` at `swissarmyhammer-entity/src/cache.rs:152-203` today calls `self.inner.write(entity)` on the wrapped `EntityContext`. If we make `EntityContext::write` also delegate to an attached cache, we recurse. Split the write path:

- Keep `EntityContext::write_internal` (renamed from the existing `write`) — pure disk/store write, no cache, no recursion.
- `EntityContext::write(entity)` public method: if `cache` is attached, delegate to `cache.write(entity)`; otherwise call `write_internal` directly.
- `EntityCache::write` stays as-is structurally, but calls `self.inner.write_internal(entity)` instead of the public `write`.

`read` and `list` don't have the recursion problem (cache methods only do `get`/`get_all`, not calls back into context), so:
- `EntityContext::read(type, id)` — if cache attached, try `cache.get(type, id).await`; fall through to disk on miss. (Misses shouldn't happen once preloaded but are possible for lazily-added types.)
- `EntityContext::list(type)` — if cache attached, return `cache.get_all(type)`.

Files:

- [ ] `swissarmyhammer-entity/src/context.rs` — add `cache: OnceLock<Arc<EntityCache>>` field to `EntityContext` (`:28-41`) plus builder `pub fn with_cache(self, cache: Arc<EntityCache>) -> Self` that panics if called twice (OnceLock semantics). Rename current `write` impl to `write_internal` (keep signature), add new public `write` that checks cache and dispatches. Modify `read` (find it; it's alongside `list` around `:581`) and `list` (`:581-594`) to consult the cache.
- [ ] `swissarmyhammer-entity/src/cache.rs` — change `EntityCache::write` at `:160` from `self.inner.write(entity)` to `self.inner.write_internal(entity)` so the cache's own write doesn't loop back through the cache.
- [ ] `swissarmyhammer-kanban/src/context.rs` — in `entity_context()` at `:381-392`'s `get_or_try_init` block, after `build_entity_context` and before `register_entity_stores`, construct the cache and preload. Pattern:
  ```rust
  let cache = Arc::new(EntityCache::new(entities_inner));
  for entity_def in fields_ctx.all_entities() {
      cache.load_all(&entity_def.name).await?;
  }
  let entities = Arc::new(
      entities_inner_again.with_cache(Arc::clone(&cache))
  );
  ```
  Since `EntityCache::new` consumes the `EntityContext` by value, either (a) build two `EntityContext` instances — one wrapped in the cache, one attached to the cache as the public-facing one — or (b) restructure so `EntityCache::new` takes `Arc<EntityContext>` and `EntityContext::with_cache` accepts `Arc<EntityCache>`. Option (b) is the right design — fix `EntityCache::new` signature at `swissarmyhammer-entity/src/cache.rs:67` to accept `Arc<EntityContext>` so both the cache and `KanbanContext` can hold the same Arc.
- [ ] `swissarmyhammer-kanban/src/context.rs` — add `pub fn entity_cache(&self) -> Option<Arc<EntityCache>>` accessor next to `entity_context`.
- [ ] `swissarmyhammer-kanban/benches/move_task_bench.rs` (new) — Criterion bench: seed a board with 2000 tasks across 4 columns (50/500/1000/450 distribution is fine), hold the `KanbanContext` open, measure 100 `MoveTask::to_column(id, "doing").with_before(neighbor).execute(&ctx)` iterations. Target median &lt;20ms per iteration.

Subtasks:

- [ ] Change `EntityCache::new` signature to take `Arc<EntityContext>`.
- [ ] Rename `EntityContext::write` → `write_internal`; add cache-aware `write` wrapper; modify `read` and `list` to consult cache.
- [ ] Add `EntityContext::with_cache` builder.
- [ ] Wire cache construction + preload into `KanbanContext::entity_context`.
- [ ] Add `swissarmyhammer-kanban/benches/move_task_bench.rs` and verify target.

## Subsumption

This sub-card **supersedes `01KP63Z8GGSY3DPRZ4N37PDY0D`** ("Perf: wire swissarmyhammer_entity::EntityCache into KanbanContext"). When this sub-card is implemented, close `01KP63Z8GGSY3DPRZ4N37PDY0D` as subsumed — the drag-perf bench in that card's "Tests" section is carried forward verbatim here.

## Acceptance Criteria

- [ ] `EntityContext` has an `Option<Arc<EntityCache>>` slot and a `with_cache` builder.
- [ ] `EntityContext::list` returns from the cache's in-memory map when attached — asserted by a test that runs `list` 100 times and counts at most **one** call to `io::read_entity_dir` (the startup `load_all`).
- [ ] `EntityContext::write` goes through `EntityCache::write` when a cache is attached — asserted by a subscribe-before-write test seeing the `EntityChanged` event with the right `changes` (relies on `entity-cache 1/4`).
- [ ] `KanbanContext::entity_context()` preloads every registered entity type on first call via `cache.load_all(type)`.
- [ ] `KanbanContext::entity_cache()` returns the same `Arc<EntityCache>` shared with the attached `EntityContext`.
- [ ] New `swissarmyhammer-kanban/benches/move_task_bench.rs`: median &lt;20ms per `MoveTask::execute` on a 2000-task seeded board.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban` stays green.
- [ ] Close `01KP63Z8GGSY3DPRZ4N37PDY0D` as subsumed.

## Tests

- [ ] `swissarmyhammer-entity/src/context.rs` — `test_list_hits_cache_not_disk`: use a counting wrapper around `io::read_entity_dir` (or a new `read_counter` test hook), build `EntityContext::with_cache`, call `load_all("task")` once, then `list("task")` 100 times; assert the counter is exactly 1.
- [ ] `swissarmyhammer-entity/src/context.rs` — `test_write_goes_through_cache_when_attached`: subscribe to cache events, call `ectx.write(&entity)`, assert exactly one `EntityChanged` event is received (uses sub-card 1's `changes` shape).
- [ ] `swissarmyhammer-kanban/src/context.rs` — `test_entity_cache_preloads_all_types`: build `KanbanContext`, call `entity_context()`, then `entity_cache().unwrap().get_all("task")` / `get_all("column")` / `get_all("tag")`; assert all on-disk entities are present without any additional `read_entity_dir` calls beyond the preload.
- [ ] `swissarmyhammer-kanban/benches/move_task_bench.rs` — seed 2000 tasks, iterate `MoveTask::execute`, report median. `cargo bench -p swissarmyhammer-kanban --bench move_task_bench` prints median &lt;20ms.
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban` — full green.

## Workflow
- Use `/tdd` — start with `test_list_hits_cache_not_disk` (fails today because `EntityContext::list` goes straight to disk), then the write-event test, then the preload test. Implement cache wiring to make them pass. Bench comes last as a verification artifact.

## Scope / depends_on
- depends_on: `01KP65VMEVVNECSK61H6BK32BM` (entity-cache 1/4 — event shape).
- Blocks: `entity-cache 4/4` (kanban-app bridge collapse).
