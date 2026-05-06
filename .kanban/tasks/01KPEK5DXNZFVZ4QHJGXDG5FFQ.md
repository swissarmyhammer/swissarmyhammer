---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
title: 'Fix: `#<tag> !$<project>` filter still shows tasks in the excluded project'
---
## What

Tasks belonging to a project whose slugified display-name appears in the filter as `!$<slug>` are still visible on the board. Reproducer in this repo: the perspective `.kanban/perspectives/01KNAGBM78EMM4ZWK5EDTRNPRF.yaml` has `filter: '#READY !$spatial-focus-navigation'`. Tasks with `project: spatial-nav` (display name "Spatial Focus Navigation") still appear when that perspective is active.

### What the research ruled out

The filter DSL itself is correct for this input:
- Parser (`swissarmyhammer-filter-expr/src/parser.rs`): `#READY !$spatial-focus-navigation` parses to `And(Tag("READY"), Not(Project("spatial-focus-navigation")))` — verified by writing a probe binary.
- Evaluator (`swissarmyhammer-filter-expr/src/eval.rs`): `Not` flips `has_project` correctly.
- Adapter (`kanban-app/src/commands.rs` `EntityFilterAdapter::has_project` ~line 423, and `swissarmyhammer-kanban/src/task_helpers.rs` `TaskFilterAdapter::has_project` ~line 697): resolves the display-name slug to the project id via `EntitySlugRegistry::project_id_for_slug`, so `"spatial-focus-navigation"` → `"spatial-nav"` and matches the task's stored `project: spatial-nav`. A probe with the real data confirms `has_project("spatial-focus-navigation")` returns `true` and `expr.matches(&adapter)` returns `false` — i.e. the task IS correctly excluded at the adapter layer.

### Likely root cause (investigate first)

The bug is almost certainly in the UI data pipeline, not the filter. Two suspect paths in `kanban-app/ui/src/components/rust-engine-container.tsx`:

1. **Event handlers bypass the filter.** `handleEntityCreated` (~line 283) and `handleEntityFieldChanged` patch `entitiesByType["task"]` directly from the event payload — they never re-evaluate the active perspective filter. A task created/updated while a filtered perspective is active will be added to the store even if it would have been excluded by the filter. Same problem for field changes that newly put a task into the excluded project.

2. **Initial load race.** `window-container.tsx` calls `refreshEntities(path)` without a `taskFilter` on board switch (lines 275/287/288/384/396). `PerspectiveContainer` (`perspective-container.tsx:102-105`) then fires a second `refreshEntities(boardPath, activeFilter)` in a `useEffect` when `activeFilter` becomes available. If the unfiltered result arrives after the filtered one, the board shows the unfiltered list.

Investigate both. The fix is likely a combination of:
- Have the event handlers check the active filter before inserting/updating a task in `entitiesByType["task"]`, OR drop the filter-aware responsibility onto a single code path that always re-applies the filter client-side on top of the server-delivered base list.
- Plumb the active perspective filter through every `refreshEntities` call path (including `window-container.tsx` board-switch paths) so no call ever runs without a filter when one is active.

Prefer the second (always pass the filter) because the backend DSL evaluator already matches the one used for the filter editor's validation. Re-implementing matching client-side would duplicate `EntityFilterAdapter` logic and the `EntitySlugRegistry` in TypeScript.

### Files to read / modify

- `kanban-app/ui/src/components/rust-engine-container.tsx` — `handleEntityCreated`, `handleEntityFieldChanged`, `useGuardedRefreshEntities`.
- `kanban-app/ui/src/components/window-container.tsx` — every `deps.refreshEntities(...)` call that omits the filter.
- `kanban-app/ui/src/components/perspective-container.tsx` — already passes the filter; make sure it's the sole source of truth for "what filter is active".
- `kanban-app/ui/src/lib/refresh.ts` — signature already supports `taskFilter`; no change expected.
- `kanban-app/ui/src/components/rust-engine-container.test.tsx` and `refresh.test.ts` — add the failing scenarios here.

### Subtasks

- [ ] Reproduce the bug in a test before changing any product code. Write a failing test in `kanban-app/ui/src/components/rust-engine-container.test.tsx` that: mounts the container with an active perspective whose filter is `#READY !$spatial-focus-navigation`, seeds entities via `list_entities` so the initial filtered list is correct, then dispatches an `entity-created` event for a task with `project: spatial-nav` and `filter_tags: ["READY"]`, and asserts that the task is NOT added to `entitiesByType["task"]`.
- [ ] Write a second failing test for the board-switch race: trigger `refreshEntities(path)` without a filter followed by `refreshEntities(path, filter)` and assert the final store only contains filtered tasks.
- [ ] Investigate the event handlers and pick ONE of the two architectural options above (event-handler-aware filter vs. "always pass filter + no direct event patching of filtered lists"). Document the choice in a one-line code comment explaining why.
- [ ] Apply the fix in the chosen path(s). Keep the diff focused on the filter-bypass — do NOT refactor unrelated event-handling code.
- [ ] Add a backend integration test in `swissarmyhammer-kanban/tests/filter_integration.rs` that exercises `#READY !$<project-name-slug>` end-to-end through `ListTasks` with a project whose id differs from `slug(name)` (mirror the structure of `test_list_tasks_filter_by_project_slug_of_name` in `swissarmyhammer-kanban/src/task/list.rs:394`, plus a NOT-case). This locks down the backend path even though the bug appears to be in the UI.
- [ ] Run `cd kanban-app/ui && npm test` and `cargo test -p swissarmyhammer-kanban -p swissarmyhammer-filter-expr` — all green.
- [ ] Manual: launch `kanban-app`, activate the "Ready" perspective, confirm no tasks with `project: spatial-nav` are visible; create a new task, set its project to "Spatial Focus Navigation", confirm it disappears from the view as soon as the project field is saved.

## Acceptance Criteria

- [ ] With perspective filter `#READY !$spatial-focus-navigation` active, no task whose `project` field resolves (by id OR display-name slug) to `spatial-nav` is visible in the kanban-app UI.
- [ ] The guarantee holds across: (a) initial load, (b) board switch, (c) perspective switch, (d) a task being created via `entity-created`, (e) an existing task's project being changed to the excluded project via `entity-field-changed`.
- [ ] The filter editor continues to accept `#<tag> !$<project>` as a valid expression with no parse error.
- [ ] Existing filter tests (backend + UI) still pass; the new failing tests added in this task now pass.

## Tests

- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — new test: `entity-created does not bypass active perspective filter`. Mount with filter `#READY !$spatial-focus-navigation`, emit `entity-created` for a task in project `spatial-nav` with tag READY; assert task is NOT in `entitiesByType["task"]`.
- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — new test: `entity-field-changed removes task when new project value matches active !$project filter`. Seed a visible task, emit field change setting `project: spatial-nav`, assert task is removed from the store.
- [ ] `kanban-app/ui/src/lib/refresh.test.ts` — new test asserting every `refreshEntities` call path passes `taskFilter` when a perspective filter is active (exercise the window-container board-switch call sites).
- [ ] `swissarmyhammer-kanban/src/task/list.rs` — new unit test `test_list_tasks_filter_not_project_by_slug_of_name`: create a project with id `spatial-nav` and name "Spatial Focus Navigation", a task in that project with tag READY, and an unrelated READY task; assert `ListTasks::with_filter("#READY !$spatial-focus-navigation")` returns only the unrelated task.
- [ ] `swissarmyhammer-kanban/tests/filter_integration.rs` — mirror the above at the integration layer.
- [ ] Command: `cd kanban-app/ui && npm test` — exit 0.
- [ ] Command: `cargo test -p swissarmyhammer-kanban -p swissarmyhammer-filter-expr` — exit 0.

## Workflow

- Use `/tdd` — write the failing UI tests first, confirm they fail for the reasons hypothesized above (NOT for wiring reasons), then fix. Run the test suite between steps.