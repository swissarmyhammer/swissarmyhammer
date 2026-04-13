---
assignees:
- claude-code
position_column: todo
position_ordinal: 9f80
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

### Files to modify

1. **`kanban-app/src/commands.rs`** — In `enrich_computed_fields()`: after the primary enrichment loop, add the fan-out pass described above. Use `all_tasks` (already loaded for enrichment) to find reverse/forward dependencies.

2. **`swissarmyhammer-kanban/src/task_helpers.rs`** — May need to expose a helper like `find_reverse_dependents(task_id, all_tasks) -> Vec<&EntityId>` to keep the lookup logic in the domain layer rather than the app layer. Check if this already exists.

### Design notes

- `enrich_task_entity()` already takes `all_tasks: &[Value]` and `terminal_column_id` — the fan-out pass can reuse the same data.
- The fan-out should be bounded: only tasks directly linked via `depends_on` / `blocks` need re-enrichment. No transitive closure needed — a task's BLOCKED status only depends on its direct dependencies' columns.
- This naturally handles all computed-tag staleness: adding/removing dependencies, moving tasks into/out of terminal columns, and any future computed tags that depend on cross-entity state.

## Acceptance Criteria

- [ ] Moving a blocking task to the terminal column emits `entity-field-changed` events for all tasks that depended on it, with updated `virtual_tags` (BLOCKED removed), `filter_tags`, and `ready` fields
- [ ] Moving a task OUT of the terminal column (e.g., back to \"doing\") re-emits BLOCKED on dependent tasks that are not yet done
- [ ] Editing `depends_on` on task A triggers re-enrichment of both the old and new dependency targets (their BLOCKING status may change)
- [ ] No spurious events: if re-enrichment produces the same values, no event is emitted

## Tests

- [ ] `kanban-app/src/commands.rs` or `swissarmyhammer-kanban/src/task_helpers.rs` — Integration test: create tasks A→B (B depends on A), move A to done, assert that the emitted events include B with BLOCKED removed from `virtual_tags`
- [ ] `cargo test -p swissarmyhammer-kanban task_helpers` — passes
- [ ] `cargo test -p kanban-app` — passes (if integration tests exist there)

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass."