---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcc80
project: expr-filter
title: Profile list_entities for 2000-task boards and parallelize read_entity_dir disk I/O
---
## What

On a kanban board with ~2000 tasks, the filter refetch triggered by `PerspectiveContainer` (`kanban-app/ui/src/components/perspective-container.tsx`) feels unacceptably slow. The user sees a "long fucking time" between pressing Enter and the filtered board appearing. This card profiles the backend path, identifies the dominant cost, and applies the lowest-risk fix (almost certainly: make `read_entity_dir` concurrent instead of sequential).

### Current path

When the frontend calls `list_entities({entityType: "task", filter})` via Tauri IPC, the chain on the Rust side is:

1. `kanban-app/src/commands.rs::list_entities` -> `handle.ctx.entity_context().await` (fast, cached)
2. `ectx.list("task").await` -> `swissarmyhammer-entity/src/context.rs::EntityContext::list` -> `io::read_entity_dir(dir, "task", def).await`
3. `swissarmyhammer-entity/src/io.rs::read_entity_dir` -- **serial loop**: `while let Some(entry) = entries.next_entry().await? { ... read_entity(&path, ...).await }`. Each iteration awaits a disk read + YAML parse for one task file, then moves to the next. **This is the likely bottleneck for 2000 tasks** -- N sequential async file reads at roughly 0.5-2ms each adds up to 1-4 seconds just on I/O.
4. `ectx.list` then loops again over the entities running `apply_compute_with_query` for each (computed fields like `ready`, `filter_tags`). Also O(N).
5. Back in `kanban-app/src/commands.rs::list_entities`, `enrich_and_sort_tasks` runs `enrich_all_task_entities` (O(N) with pre-built dep indexes -- see `swissarmyhammer-kanban/src/task_helpers.rs`) and sorts. Unlikely to be the bottleneck.
6. `apply_filter(&mut entities, filter_str)` parses the DSL and retains matching entries. O(N) with cheap per-entity evaluation.
7. Serializes each entity to JSON.

### Approach

**Step 1 -- profile first.** Add a benchmark at `swissarmyhammer-entity/benches/list_entities.rs` that times `read_entity_dir` on N=500 and N=2000 synthetic task files. Use `criterion` (already in workspace deps).

**Step 2 -- parallelize `read_entity_dir` disk I/O.** Convert the serial loop to a two-phase approach: (1) sequential dir enumeration into `Vec<(PathBuf, String)>`, (2) bounded-concurrency `buffer_unordered(64)` reads via `futures::stream`. Preserve exact error semantics: NotFound silent skip, parse errors warn-and-skip, I/O propagate.

**Step 3 -- re-run the benchmark.** Record before/after delta.

### Subtasks

- [x] Add a `criterion` benchmark at `swissarmyhammer-entity/benches/list_entities.rs` that sets up a 500-/2000-task temp `.kanban/` and times `read_entity_dir`. Added `criterion` and `futures` to dev-deps. Baseline recorded in the bench module docs.
- [x] Convert `read_entity_dir` in `swissarmyhammer-entity/src/io.rs` to the two-phase pattern: serial dir enumeration -> bounded-concurrency `buffer_unordered(64)` reads via `futures::stream`. Error semantics preserved exactly (NotFound silent skip, parse errors warn-and-skip, I/O propagate).
- [x] Re-run the benchmark, recorded the new timing. Speedup ~3.0x at N=2000 hot-cache (worst case for parallelization).
- [x] Added unit tests for the concurrent path: `read_entity_dir_skips_unparseable_files_concurrently` (mix of valid + corrupt + wrong-extension files exceeding the concurrency window) and `read_entity_dir_tolerates_deleted_mid_read` (race-tolerant deletion during read).

## Acceptance Criteria

- [x] A new criterion benchmark exists at `swissarmyhammer-entity/benches/list_entities.rs` and runs via `cargo bench -p swissarmyhammer-entity --bench list_entities` without manual setup.
- [x] The benchmark's 2000-task `read_entity_dir` wall time is approximately 3x faster after the change (baseline ~79.7 ms, after ~26 ms; both numbers recorded in the bench module docs and in the implement summary). Hot-cache measurement; cold cache is expected to show a substantially larger speedup.
- [x] `read_entity_dir` preserves its exact error semantics: `NotFound` mid-read skips the file silently (race-tolerant), `InvalidFrontmatter`/`Yaml` parse errors log a warning and skip, I/O and other errors propagate.
- [x] Full test suite regression-clean: `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` passes (1421 tests). `cargo clippy -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app --all-targets -- -D warnings` produces zero warnings.

## Tests

- [x] **Benchmark (diagnostic, not CI gate)** -- `swissarmyhammer-entity/benches/list_entities.rs`: builds a temp `.kanban/`-style dir with 500 and 2000 task `.md` files, times `read_entity_dir` via criterion. Numbers recorded in the bench module docs.
- [x] **Error-handling regression** -- `swissarmyhammer-entity/src/io.rs` test module:
  - `read_entity_dir_skips_unparseable_files_concurrently`: writes (2 * CONCURRENCY + 5) valid files plus 5 corrupt yaml files plus wrong-extension files; asserts the parallel pipeline returns exactly the valid set.
  - `read_entity_dir_tolerates_deleted_mid_read`: spawns a tokio task that races to delete a file during the read phase; asserts the listing never errors regardless of who wins the race.
  - Existing tests `read_entity_dir_skips_parse_errors`, `read_entity_dir_skips_bad_frontmatter`, `read_entity_dir_propagates_io_errors`, `read_entity_dir_reads_all` continue to pass against the parallel implementation -- they are happy-path regression coverage.
- [x] **Regression run**: `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` -- all 1421 green. `cargo clippy --all-targets -- -D warnings` on the same crates -- zero warnings.

## Workflow

Use `/tdd` loosely here -- the benchmark is the test driving the change, not a traditional unit test. Order:

1. Write the benchmark first. Run it on the current serial implementation, record the baseline.
2. Make the parallelization change in `read_entity_dir`.
3. Re-run the benchmark, record the improved timing.
4. Add the error-handling unit tests to make sure the new concurrent path preserves the old semantics.
5. Run the full regression suite.

If step 3's speedup is under 3x, **stop**. The assumption (disk I/O is the bottleneck) was wrong.

## Notes / related

- This card pairs with the frontend card **01KNYEH1W0KGTED4380RSYVW9T** (decouple filter save from refresh + visible progress + latest-wins). That card makes the refetch **visible** and **cancellable**; this card makes it **fast**. Both ship independently, but the user-visible improvement is largest when both land.
- The filter DSL itself (`swissarmyhammer-filter-expr`) is not the bottleneck -- its evaluation is cheap. `apply_filter` in `kanban-app/src/commands.rs` parses the expression once and then iterates; this is fine at N=2000.
- Out of scope for this card: in-memory task caching on the BoardHandle, incremental/streaming filter evaluation, AbortController plumbing across Tauri IPC, indexing by tag/assignee/project.
- The 64-thread concurrency limit in `buffer_unordered(64)` is a first guess validated by the benchmark. The bench shows ~3x speedup hot-cache; cold-cache is expected to be substantially larger because serial reads each block on disk while concurrent reads overlap.
