---
assignees:
- claude-code
position_column: todo
position_ordinal: b380
project: expr-filter
title: Profile list_entities for 2000-task boards and parallelize read_entity_dir disk I/O
---
## What

On a kanban board with ~2000 tasks, the filter refetch triggered by `PerspectiveContainer` (`kanban-app/ui/src/components/perspective-container.tsx`) feels unacceptably slow. The user sees a "long fucking time" between pressing Enter and the filtered board appearing. This card profiles the backend path, identifies the dominant cost, and applies the lowest-risk fix (almost certainly: make `read_entity_dir` concurrent instead of sequential).

### Current path

When the frontend calls `list_entities({entityType: "task", filter})` via Tauri IPC, the chain on the Rust side is:

1. `kanban-app/src/commands.rs::list_entities` → `handle.ctx.entity_context().await` (fast, cached)
2. `ectx.list("task").await` → `swissarmyhammer-entity/src/context.rs::EntityContext::list` → `io::read_entity_dir(dir, "task", def).await`
3. `swissarmyhammer-entity/src/io.rs::read_entity_dir` — **serial loop** at line 137: `while let Some(entry) = entries.next_entry().await? { ... read_entity(&path, ...).await }`. Each iteration awaits a disk read + YAML parse for one task file, then moves to the next. **This is the likely bottleneck for 2000 tasks** — N sequential async file reads at roughly 0.5–2ms each adds up to 1–4 seconds just on I/O.
4. `ectx.list` then loops again over the entities running `apply_compute_with_query` for each (computed fields like `ready`, `filter_tags`). Also O(N).
5. Back in `kanban-app/src/commands.rs::list_entities`, `enrich_and_sort_tasks` runs `enrich_all_task_entities` (O(N) with pre-built dep indexes — see `swissarmyhammer-kanban/src/task_helpers.rs` line 371) and sorts. Unlikely to be the bottleneck.
6. `apply_filter(&mut entities, filter_str)` parses the DSL and retains matching entries. O(N) with cheap per-entity evaluation.
7. Serializes each entity to JSON.

### Approach

**Step 1 — profile first, don't guess.** Add a `criterion` benchmark (the project already uses `criterion` in other crates — check for existing `benches/` directories before picking a harness) that:
- Sets up a temp `.kanban/` dir with 2000 synthetic task YAML files.
- Calls `list_entities` through the full Rust pipeline (either via `EntityContext::list` + `enrich_all_task_entities` + `apply_filter`, or by wiring up a minimal `AppState` and calling the Tauri command directly — prefer whichever maps closer to the real path).
- Reports per-phase timings: dir read, per-entity YAML parse, compute-engine derivation, enrich step, filter step, JSON serialize.

The benchmark is a **diagnostic artifact**, not a CI gate. It lives in `swissarmyhammer-entity/benches/list_entities.rs` (or `swissarmyhammer-kanban/benches/` if the enrich step turns out to matter) and is runnable via `cargo bench -p swissarmyhammer-entity --bench list_entities`. If `criterion` isn't already a dev-dep of the crate, add it; if another crate in the workspace already has a benchmark pattern, follow that prevailing pattern.

**Step 2 — parallelize `read_entity_dir` disk I/O.** The current serial loop is:

```rust
while let Some(entry) = entries.next_entry().await? {
    // ...collect path + id...
    match read_entity(&path, entity_type, &id, entity_def).await { ... }
}
```

Convert to a two-phase approach:

```rust
// Phase 1: collect paths from the dir listing (cheap, sequential is fine).
let mut jobs: Vec<(PathBuf, String)> = Vec::new();
while let Some(entry) = entries.next_entry().await? {
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some(ext) { continue; }
    let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else { continue };
    jobs.push((path, id));
}

// Phase 2: spawn bounded-concurrency reads.
use futures::stream::{self, StreamExt};
const CONCURRENCY: usize = 64; // avoid overwhelming the OS file table
let entities: Vec<Entity> = stream::iter(jobs)
    .map(|(path, id)| {
        let entity_type = entity_type.to_string();
        let entity_def = entity_def.clone();
        async move {
            match read_entity(&path, &entity_type, &id, &entity_def).await {
                Ok(e) => Some(Ok(e)),
                Err(EntityError::NotFound { .. }) => None,
                Err(e @ (EntityError::InvalidFrontmatter { .. } | EntityError::Yaml { .. })) => {
                    warn!(path = %path.display(), error = %e, "skipping unparseable entity file");
                    None
                }
                Err(e) => Some(Err(e)),
            }
        }
    })
    .buffer_unordered(CONCURRENCY)
    .filter_map(|r| async move { r })
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
```

Key constraints:
- **Bounded concurrency** (`buffer_unordered(64)`) — unbounded `join_all` risks exhausting the OS file handle budget and has quadratic memory cost at high N. 64 is a starting point; the benchmark validates whether to tune.
- **Preserve error semantics** exactly: `NotFound` and parse errors are skipped-with-warning; I/O and other errors propagate via `?`. The current sequential loop has this behavior; the parallel version must too.
- **Do not parallelize** phase 1 (the `next_entry().await` loop over the dir handle). Dir enumeration is already cheap, and tokio's `ReadDir` is not `Clone`-able.
- **`EntityDef` is `Clone`** (required to move into spawned futures) — verify before committing. If it isn't cheap to clone, wrap in `Arc` locally and share.

**Step 3 — re-run the benchmark.** Record the before/after delta for N=2000 tasks. If parallelization does not give at least a 3× speedup (my ballpark hypothesis is 5–10× since disk reads are the dominant cost), the profile was wrong about the bottleneck and this card should stop and report findings rather than shipping. Do not proceed to heavier changes (in-memory task cache, incremental filter evaluation, async streaming) in this card — those are follow-ups if step 2 isn't enough.

### Subtasks

- [ ] Add a `criterion` benchmark at `swissarmyhammer-entity/benches/list_entities.rs` (or follow the existing benchmark pattern in the workspace) that sets up a 2000-task temp `.kanban/` and times `EntityContext::list("task")`. If `criterion` isn't a dev-dep of `swissarmyhammer-entity` yet, add it. Record the baseline timing in the PR/commit body.
- [ ] Convert `read_entity_dir` in `swissarmyhammer-entity/src/io.rs` to the two-phase pattern above: serial dir enumeration → bounded-concurrency `buffer_unordered(64)` reads via `futures::stream`. Preserve the exact error-handling semantics (skip `NotFound`, warn-and-skip parse errors, propagate I/O).
- [ ] Re-run the benchmark, record the new timing. Fail the subtask if speedup is < 3× — investigate and branch rather than shipping an unprincipled fix.
- [ ] Add a unit test for `read_entity_dir` that exercises the concurrent path with a mix of valid, unparseable, and deleted-mid-read files to confirm error handling hasn't drifted.

## Acceptance Criteria

- [ ] A new criterion benchmark exists at `swissarmyhammer-entity/benches/list_entities.rs` (or the workspace-conventional location) and runs via `cargo bench -p swissarmyhammer-entity --bench list_entities` without manual setup.
- [ ] The benchmark's 2000-task `EntityContext::list("task")` wall time is at least 3× faster after the change than before (both numbers recorded in the commit body).
- [ ] `read_entity_dir` preserves its exact error semantics: `NotFound` mid-read skips the file silently (race-tolerant), `InvalidFrontmatter`/`Yaml` parse errors log a warning and skip, I/O and other errors propagate.
- [ ] Full test suite regression-clean: `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` passes with zero failures and zero warnings from `cargo clippy --all-targets -- -D warnings`.

## Tests

- [ ] **Benchmark (diagnostic, not CI gate)** — `swissarmyhammer-entity/benches/list_entities.rs`:
  - Build a temp `.kanban/` dir with 2000 task YAML files (reuse existing test fixtures or synthesize minimal valid task YAML).
  - Time `EntityContext::list("task")` with no compute engine attached (isolates disk I/O) and a second bench with compute attached (full path).
  - Run: `cargo bench -p swissarmyhammer-entity --bench list_entities`. Expected: the no-compute variant shows the dramatic speedup from parallelization; the with-compute variant shows a smaller but still measurable improvement.
- [ ] **Error-handling regression** — `swissarmyhammer-entity/src/io.rs` test module:
  - `read_entity_dir_skips_unparseable_files_concurrently`: set up a dir with one valid YAML and one intentionally-bad YAML; assert `read_entity_dir` returns the valid one and logs a warning for the bad one (warning assertion via `tracing_subscriber::fmt::TestWriter` if already used elsewhere, otherwise just assert the `Vec` has the right length).
  - `read_entity_dir_propagates_io_errors`: simulate via a dir path the process cannot read and assert `EntityError::Io` is returned. May need to use `tempfile` + `chmod` — if the test environment (e.g. CI on macOS) can't honor chmod, skip this subtask and rely on the existing test coverage for error paths.
  - Reuse the existing `read_entity_dir_reads_all` test at `swissarmyhammer-entity/src/io.rs` line 727 as the happy-path regression.
- [ ] **Regression run**: `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app` — all green. `cargo clippy --all-targets -- -D warnings` on the same crates — zero warnings.

## Workflow

Use `/tdd` loosely here — the benchmark is the test driving the change, not a traditional unit test. Order:

1. Write the benchmark first. Run it on the current serial implementation, record the baseline.
2. Make the parallelization change in `read_entity_dir`.
3. Re-run the benchmark, record the improved timing.
4. Add the error-handling unit tests to make sure the new concurrent path preserves the old semantics.
5. Run the full regression suite.

If step 3's speedup is under 3×, **stop**. The assumption (disk I/O is the bottleneck) was wrong. In that case: add a brief note to the card explaining what the profile actually showed, revert the parallelization, and open a follow-up card for the real bottleneck (likely `apply_compute_with_query` per-entity derivation, or `serde_yaml_ng` parse cost, or something downstream in `enrich_all_task_entities`). Do not ship parallelization just because it's "faster in theory."

## Notes / related

- This card pairs with the frontend card **01KNYEH1W0KGTED4380RSYVW9T** (decouple filter save from refresh + visible progress + latest-wins). That card makes the refetch **visible** and **cancellable**; this card makes it **fast**. Both ship independently, but the user-visible improvement is largest when both land.
- The filter DSL itself (`swissarmyhammer-filter-expr`) is not the bottleneck — its evaluation is cheap. `apply_filter` in `kanban-app/src/commands.rs` parses the expression once and then iterates; this is fine at N=2000.
- Out of scope for this card: in-memory task caching on the BoardHandle, incremental/streaming filter evaluation, AbortController plumbing across Tauri IPC, indexing by tag/assignee/project. These are bigger ideas worth a separate planning pass if parallel disk I/O isn't enough. Do NOT anticipate them here.
- The 64-thread concurrency limit in `buffer_unordered(64)` is a first guess. If the benchmark shows 64 is too low (reads haven't saturated) or too high (contention on the async runtime), tune with benchmark evidence only, not intuition.
