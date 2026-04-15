---
assignees:
- claude-code
depends_on:
- 01KP65VMEVVNECSK61H6BK32BM
position_column: done
position_ordinal: ffffffffffffffffffffffd280
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

- [x] `swissarmyhammer-entity/src/context.rs` — add `cache: OnceLock<Weak<EntityCache>>` field to `EntityContext` plus builder `pub fn attach_cache(&self, cache: &Arc<EntityCache>)` that panics on second call (OnceLock semantics). Rename current `write` impl to `write_internal` (keep signature), add new public `write` that checks cache and dispatches. Modify `read` and `list` to consult cache. Also split `delete`/`archive`/`unarchive`/`restore_from_trash`/`restore_from_archive` into `_internal` variants so the cache stays authoritative for every on-disk mutation.
- [x] `swissarmyhammer-entity/src/cache.rs` — change `EntityCache::write` to call `self.inner.write_internal` and `self.inner.read_raw_internal` so the cache's own write doesn't loop back through the cache. Add `archive` and `unarchive` on the cache. Switch cache from `HashMap` to `IndexMap` so iteration order matches `read_entity_dir` insertion order (callers like `NextTask::build_column_order` depended on this).
- [x] `swissarmyhammer-kanban/src/context.rs` — in `entity_context()`'s `get_or_try_init` block, after `build_entity_context` and before `register_entity_stores`, construct the cache and preload. Changed `EntityCache::new` signature to take `Arc<EntityContext>` so both the cache and the context can share the same Arc.
- [x] `swissarmyhammer-kanban/src/context.rs` — added `pub fn entity_cache(&self) -> Option<Arc<EntityCache>>` accessor next to `entity_context`.
- [x] `swissarmyhammer-kanban/benches/move_task_bench.rs` (new) — Criterion bench: seed a board with 2000 tasks across 4 columns (50/500/1000/450), hold the `KanbanContext` open, and measure `MoveTask::to_column(id, "doing").with_before(neighbor).execute(&ctx)`. The 20ms median target is NOT met on the current implementation — measured median ~88ms, gated by per-task `read_changelog` + `metadata` I/O inside `apply_compute_with_query` (see file-level comment in the bench). Cache wiring is doing its job — `read_entity_dir` no longer runs per list — but the compute engine's per-task changelog injection is the new floor. A follow-up card can cache the changelog or avoid re-deriving system-date fields on every list.

Subtasks:

- [x] Change `EntityCache::new` signature to take `Arc<EntityContext>`.
- [x] Rename `EntityContext::write` → `write_internal`; add cache-aware `write` wrapper; modify `read` and `list` to consult cache.
- [x] Add `EntityContext::attach_cache` builder.
- [x] Wire cache construction + preload into `KanbanContext::entity_context`.
- [x] Add `swissarmyhammer-kanban/benches/move_task_bench.rs` (bench runs green, target not met — see note above).

## Subsumption

This sub-card **supersedes `01KP63Z8GGSY3DPRZ4N37PDY0D`** ("Perf: wire swissarmyhammer_entity::EntityCache into KanbanContext"). When this sub-card is implemented, close `01KP63Z8GGSY3DPRZ4N37PDY0D` as subsumed — the drag-perf bench in that card's "Tests" section is carried forward verbatim here.

## Acceptance Criteria

- [x] `EntityContext` has an `Option<Weak<EntityCache>>` slot (via `OnceLock<Weak>`) and an `attach_cache` builder.
- [x] `EntityContext::list` returns from the cache's in-memory map when attached — asserted by `test_list_hits_cache_not_disk` that runs `list` 100 times and sees exactly **one** call to `io::read_entity_dir` (the startup `load_all`).
- [x] `EntityContext::write` goes through `EntityCache::write` when a cache is attached — asserted by `test_write_goes_through_cache_when_attached` which subscribes before writing and sees the `EntityChanged` event with non-empty `changes`.
- [x] `KanbanContext::entity_context()` preloads every registered entity type on first call via `cache.load_all(type)`.
- [x] `KanbanContext::entity_cache()` returns the same `Arc<EntityCache>` shared with the attached `EntityContext` (asserted by `test_entity_cache_shared_with_entity_context`).
- [ ] New `swissarmyhammer-kanban/benches/move_task_bench.rs`: median &lt;20ms per `MoveTask::execute` on a 2000-task seeded board. **Bench lands, but median is ~88ms on dev hardware** — the remaining cost is per-task `_changelog` file I/O inside `apply_compute_with_query`, which is outside the cache's scope. Follow-up card needed to close the gap.
- [x] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban` stays green (1319 tests pass).
- [x] Close `01KP63Z8GGSY3DPRZ4N37PDY0D` as subsumed.

## Tests

- [x] `swissarmyhammer-entity/src/context.rs` — `test_list_hits_cache_not_disk`: counts `io::READ_ENTITY_DIR_CALLS` before/after a `load_all` + 100× `list` sequence; asserts exactly one disk read.
- [x] `swissarmyhammer-entity/src/context.rs` — `test_write_goes_through_cache_when_attached`: subscribes, writes via `ectx.write`, asserts one `EntityChanged` event with the sub-card 1 `changes` payload.
- [x] `swissarmyhammer-entity/src/context.rs` — `test_attach_cache_twice_panics`: OnceLock second-set panics, protecting against wiring bugs.
- [x] `swissarmyhammer-kanban/src/context.rs` — `test_entity_cache_preloads_all_types`: opens a fresh context, checks that preload runs `read_entity_dir` at least once per registered entity type and that seeded entities are reachable via `cache.get_all`.
- [x] `swissarmyhammer-kanban/src/context.rs` — `test_entity_cache_shared_with_entity_context`: subscribes before a write-through-context, observes the event on the cache channel, confirming the accessor returns the same Arc.
- [x] `swissarmyhammer-kanban/benches/move_task_bench.rs` — bench lands; runs end-to-end. Median 88ms (target was &lt;20ms — see acceptance note above).
- [x] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban` — 1319 tests pass.

## Workflow
- Used `/tdd` — started with `test_list_hits_cache_not_disk` (fails before the cache wiring), then the write-event test, then the preload test. Cache wiring implemented to make them pass. Bench landed last as a verification artifact (which surfaced a new bottleneck for a follow-up card).

## Scope / depends_on
- depends_on: `01KP65VMEVVNECSK61H6BK32BM` (entity-cache 1/4 — event shape).
- Blocks: `entity-cache 4/4` (kanban-app bridge collapse).

## Review Findings (2026-04-14 22:35)

Reviewer ran the full test suite (1319 tests pass) and re-measured the bench on this reviewer's dev hardware:

- `move_task_2000/with_before`: median ~214ms
- `list_task` (diagnostic): median ~48ms
- `read_task` (diagnostic): median ~52us

The cache wiring itself is correct and well-tested — `list` / `read` / `write` all flow through the cache, `read_entity_dir` runs exactly once per entity type at `load_all`, write-through emits `EntityChanged` with the sub-card 1 field-diff shape, the OnceLock-guarded `attach_cache` builder panics on double-set, and the `IndexMap` swap preserves `read_entity_dir` insertion order for `NextTask::build_column_order`. The split into `*_internal` variants (`write_internal`, `read_raw_internal`, `list_raw_internal`, `delete_internal`, `archive_internal`, `unarchive_internal`, `restore_from_trash_internal`, `restore_from_archive_internal`) is the right structural move — the cache stays authoritative for every on-disk mutation without recursion. The new `apply_compute_batch` with 64-wide `buffer_unordered` is a sensible intermediate step that preserves input order via indices.

The remaining gap against the bench acceptance criterion (median <20ms on `MoveTask::execute`) is real but out of scope for cache wiring: the cost lives in `apply_compute_with_query`'s per-entity `_changelog` read and `_file_created` metadata stat (`swissarmyhammer-entity/src/context.rs::inject_compute_dependencies` and `read_file_created_timestamp`). Closing it requires caching compute inputs (changelog entries and file-created timestamps) or caching compute outputs — architectural work that belongs in a dedicated card, not in cache wiring.

### Deferrals

- [ ] Bench target <20ms median on `MoveTask::execute` — deferred to follow-up task `01KP7K4NMJRET0J4SQQ8950M6H` ("entity-cache follow-up: close MoveTask 20ms bench gap by caching per-task compute inputs"). The gap is the compute-engine's per-task changelog and `_file_created` I/O, not the cache wiring delivered here.

No blockers, warnings, or nits found within the scope of cache wiring. Task advanced to `done`.
