---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcd80
project: task-card-fields
title: Re-enrich dependent tasks when a task's computed-tag inputs change (stale BLOCKED/READY/BLOCKING tags)
---
## What

Virtual tags (BLOCKED, READY, BLOCKING) are computed on-the-fly by `enrich_task_entity()` in `swissarmyhammer-kanban/src/task_helpers.rs`. They depend on cross-entity state — e.g., BLOCKED checks whether each `depends_on` target is in the terminal column. But the post-command event pipeline in `kanban-app/src/commands.rs` only enriches entities whose files changed on disk. When task A moves to \"done\", task B (which depends on A) has no disk change, gets no enrichment pass, and its BLOCKED tag stays stale until a full refresh.

This is a generic problem: any mutation that changes the inputs to another task's computed tags (column changes, dependency edits, tag changes) should trigger re-enrichment of affected tasks.

### Approach — expand `enrich_computed_fields()` with a dependency fan-out pass

In `kanban-app/src/commands.rs`, the `enrich_computed_fields()` function (around line 1611) currently only processes entities present in the `events` list. Add a second pass:

1. After the initial enrichment loop, collect the set of entity IDs that had their `position_column` or `depends_on` fields change (from the events).
2. For each such entity, find all tasks whose `depends_on` includes that entity ID (reverse dependency lookup). Also find all tasks that the changed entity `depends_on` (forward lookup — these may gain/lose BLOCKING status).
3. Run `enrich_task_entity()` on each affected task.
4. Diff the new computed fields against the previously-emitted values. If any changed, emit synthetic `EntityFieldChanged` events for `virtual_tags`, `filter_tags`, `ready`, `blocked_by`, and `blocks`.

This is generic: it doesn't hard-code BLOCKED or any specific tag. It re-runs the full enrichment pipeline on any task whose computed-tag inputs may have changed, and only emits events when actual values differ.

### Files modified

1. **`kanban-app/src/enrichment.rs`** (new module) — fan-out logic, `EnrichmentCache` (Arc<Mutex<HashMap<String, TaskEnrichmentSnapshot>>>), `TaskEnrichmentSnapshot`, `collect_trigger_task_ids`, `snapshot_previous_depends_on`, `compute_fanout_targets`, `fan_out_synthetic_events`, `record_primary_enrichment`. 22 unit tests + 4 end-to-end tests covering all acceptance criteria.

2. **`kanban-app/src/main.rs`** — registers `mod enrichment;`.

3. **`kanban-app/src/state.rs`** — adds `enrichment_cache: EnrichmentCache` to `BoardHandle`; initialized via `new_enrichment_cache()` in `open()`.

4. **`kanban-app/src/commands.rs`** — new `enrich_computed_fields_with_fanout()` wrapper that runs the primary enrichment pass, primes the cache for triggers, computes fan-out targets, and appends synthetic events. `flush_and_emit_for_handle` calls this wrapper.

5. **`swissarmyhammer-kanban/src/task_helpers.rs`** — adds `find_dependent_task_ids(task_id: &str, all_tasks: &[Entity]) -> Vec<String>` (reverse-dependency lookup that accepts a bare `&str` so it works when the trigger is absent from `all_tasks`). 3 unit tests.

### Design notes

- `enrich_task_entity()` already takes `all_tasks: &[Value]` and `terminal_column_id` — the fan-out pass reuses the same data.
- The fan-out is bounded: only tasks directly linked via `depends_on` / `blocks` need re-enrichment. No transitive closure needed.
- This naturally handles all computed-tag staleness: adding/removing dependencies, moving tasks into/out of terminal columns, and any future computed tags that depend on cross-entity state.

## Acceptance Criteria

- [x] Moving a blocking task to the terminal column emits `entity-field-changed` events for all tasks that depended on it, with updated `virtual_tags` (BLOCKED removed), `filter_tags`, and `ready` fields — covered by `end_to_end_moving_a_to_done_emits_blocked_removal_for_b`.
- [x] Moving a task OUT of the terminal column (e.g., back to \"doing\") re-emits BLOCKED on dependent tasks that are not yet done — covered by `end_to_end_moving_a_back_out_of_done_re_emits_blocked`.
- [x] Editing `depends_on` on task A triggers re-enrichment of both the old and new dependency targets (their BLOCKING status may change) — covered by `end_to_end_editing_depends_on_refreshes_old_and_new_targets`.
- [x] No spurious events: if re-enrichment produces the same values, no event is emitted — covered by `end_to_end_no_spurious_events_when_column_move_does_not_flip_blocked`, `fan_out_emits_nothing_when_state_unchanged`, and `fan_out_updates_cache_even_when_no_event_emitted`.

## Tests

- [x] `kanban-app/src/enrichment.rs` — integration test: create tasks A→B (B depends on A), move A to done, assert that the emitted events include B with BLOCKED removed from `virtual_tags`.
- [x] `cargo test -p swissarmyhammer-kanban task_helpers` — passes (49 tests including 3 new `find_dependent_task_ids` tests).
- [x] `cargo test -p kanban-app` — passes (161 tests including 27 enrichment module tests).

## Workflow

- Used TDD — full test suite (27 tests in `enrichment.rs`, 3 in `task_helpers.rs`) verified before wiring the integration, then integration tests confirmed no regressions.

## Review Findings (2026-04-13 18:41)

### Warnings

- [x] `kanban-app/src/enrichment.rs:128` (`collect_trigger_task_ids`) — Only `EntityFieldChanged` events are considered triggers. `EntityCreated` and `EntityRemoved` for tasks carrying a `depends_on` list do not trigger fan-out, so their forward/reverse dependents get stale BLOCKING/BLOCKED/READY state on the frontend. E.g. creating B with `depends_on: [a]` leaves A's `blocks` and BLOCKING tag stale; deleting the last blocker of B leaves B stuck as BLOCKED. The card's acceptance criteria explicitly scoped the trigger set to `EntityFieldChanged`, so the tests pass — but the card description frames this as a \"generic problem\" and the implementation doesn't cover creates/removes. Suggest extending `collect_trigger_task_ids` to include `EntityCreated` (use the payload's `depends_on` as forward triggers) and `EntityRemoved` (use the reverse-dependency lookup as-is; current_deps is empty so only the reverse branch matters).
- [x] `kanban-app/src/enrichment.rs:115` — The `EnrichmentCache` `HashMap<String, TaskEnrichmentSnapshot>` is never pruned. When a task is deleted, its entry lingers forever. For long-running sessions this is a slow memory leak bounded by total tasks ever created. Suggest a cleanup hook on `EntityRemoved` that does `cache.lock().unwrap().remove(&id)` — easy to add in the fan-out wrapper alongside the proposed trigger-set extension above.

### Nits

- [x] `kanban-app/src/commands.rs:2253-2259` — The fan-out wrapper re-enriches every trigger (clones the entity, calls `enrich_task_entity`, records snapshot) even though the primary `enrich_computed_fields` path already did the same work a moment earlier via `enrich_task_from_context`. If the primary loop could populate the cache as a side effect (or return the enriched entity), this second pass could be skipped. Current code is correct but doubles the per-trigger enrichment cost — acceptable given that only tasks with column or depends_on changes hit this path.
- [x] `swissarmyhammer-kanban/src/task_helpers.rs:202` (`find_dependent_task_ids`) — Body is nearly identical to the existing `task_blocks` function a few lines up; the only differences are accepting `&str` instead of `&Entity` and using `.any(|d| d == task_id)` vs `.contains(&entity.id.to_string())`. The docstring explains why the input type differs (trigger may be absent from `all_tasks`), but the two implementations could share code: `task_blocks` could delegate to `find_dependent_task_ids(entity.id.as_str(), all_tasks)`. Minor DRY cleanup.
- [x] `kanban-app/src/enrichment.rs` — Several doc comments contain unquoted proper-noun tokens (e.g. `FieldChange`, `BoardHandle`) that clippy's pedantic `doc_markdown` lint flags. Not a correctness issue; backticks would silence the warnings.

## Review Follow-up (2026-04-13)

Addressed all five items from the review findings above.

### Warning 1 — `collect_trigger_task_ids` now covers creates and removes

Extended `collect_trigger_task_ids` to include task `EntityCreated` and `EntityRemoved` events. Creating a new task routes through the normal forward-lookup path (the new entity appears in `all_tasks`, so `compute_fanout_targets` discovers any tasks it `depends_on`). Removing a task routes through the reverse-lookup path (other tasks' `depends_on` still references the deleted id) plus the cached-previous-`depends_on` forward path (captured before the prune).

Added tests covering both directions: `trigger_collects_task_create_events`, `trigger_collects_task_remove_events`, `trigger_ignores_non_task_create_and_remove_events`, `end_to_end_creating_b_depending_on_a_updates_a_blocks`, `end_to_end_removing_last_blocker_unblocks_dependent`, `end_to_end_removing_blocker_with_multi_dep_unblocks_dependent`.

### Warning 2 — `EnrichmentCache` is pruned on task deletion

Added `collect_removed_task_ids` + `prune_cache_for_removed` helpers. The fan-out wrapper calls `prune_cache_for_removed` on every return path AFTER `snapshot_previous_depends_on` has captured pre-delete state AND after the fan-out pass itself (so deleted entries still service forward-dependency lookups during fan-out). Tests: `removed_ids_collects_task_deletes`, `removed_ids_ignores_non_task_deletes`, `removed_ids_ignores_create_and_change_events`, `prune_cache_removes_named_entries`, `prune_cache_is_noop_for_empty_set`, `prune_cache_tolerates_missing_ids`.

### Nit 3 — Primary loop primes the cache, no double enrichment

Split `enrich_computed_fields` into a public test-only wrapper and `enrich_computed_fields_inner(ctx, events, cache: Option<&EnrichmentCache>)`. `enrich_one_watch_event` now takes `cache: Option<&EnrichmentCache>` and, after `enrich_task_from_context` fully enriches a task entity, records the snapshot. The fan-out wrapper simply calls `enrich_computed_fields_inner(..., Some(cache))` and drops the old manual trigger-prime loop entirely. Every task event's enrichment now populates the cache exactly once, and no event goes through a second enrichment pass.

### Nit 4 — `task_blocks` delegates to `find_dependent_task_ids`

`task_blocks` is now a one-line wrapper that calls `find_dependent_task_ids(entity.id.as_ref(), all_tasks)`. Doc comment points readers at the underlying helper for the bare-id case. All 49 `task_helpers` tests still pass.

### Nit 5 — Doc comments use backticks around type names

Wrapped `FieldChange`, `BoardHandle`, `Vec<String>`, and `bool` in backticks in the enrichment module doc comments. `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.

### Verification

- `cargo test -p swissarmyhammer-kanban`: 956 passed.
- `cargo test -p kanban-app`: 172 passed (was 161; added 11 new tests covering create/remove triggers + cache pruning + new end-to-end scenarios).
- `cargo clippy -p swissarmyhammer-kanban -p kanban-app --all-targets -- -D warnings`: clean.
