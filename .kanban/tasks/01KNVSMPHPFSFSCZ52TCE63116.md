---
assignees:
- claude-code
depends_on:
- 01KNVSGJ7X7A95T8XV80E9M7Y3
- 01KNVSK9V84X79TKGPP9XCYKFH
position_column: todo
position_ordinal: b180
project: expr-filter
title: locate and remove redundant project filtering field
---
## What

Once `$project-slug` is live in the filter DSL (parser, FilterContext adapters, Lezer grammar, and autocomplete — see sibling cards in this project), locate and remove the pre-existing project-specific filter path that is now redundant.

**Investigation checklist — do this FIRST before editing anything.** The user has stated there is a `project` filter affordance somewhere that will become redundant. Prior research did NOT conclusively locate it. Search these candidate locations in order:

1. **Tauri/backend command parameters** — grep for a `project: Option<String>` parameter being used for filtering (not for setting, which is legitimate in `AddTask` / `UpdateTask`):
   - `kanban-app/src/commands.rs::list_entities` — does a later revision take a `project` param?
   - `swissarmyhammer-kanban/src/task/list.rs::ListTasks` and `next.rs::NextTask` — do they have a `project: Option<String>` field?
   - Any other Tauri `#[tauri::command]` that takes a `project` filter
   - Dispatch layer: `swissarmyhammer-kanban/src/dispatch.rs` passing a `project` param that isn't already routed to set-semantics

2. **Perspective definition** — `swissarmyhammer-perspectives/src/types.rs::Perspective` — check for any dedicated `project: Option<String>` field that was added beyond `filter`, `group`, `sort`, `fields`. Grep for `project` under `swissarmyhammer-perspectives/`.

3. **Frontend filter editor UI** — search `kanban-app/ui/src/components/` for project-specific filter controls:
   - A project dropdown/select in the formula bar, toolbar, or filter editor shell
   - A `projectFilter` state variable in perspective context or store
   - A URL query parameter or hash fragment for filtering by project
   - Dedicated props or context values with "project" in the name passed to filter editor components
   - Grep for `project_filter`, `projectFilter`, `filterByProject`, `setProjectFilter`

4. **Refresh / entity loading path** — `kanban-app/ui/src/lib/refresh.ts::refreshBoards` currently takes `taskFilter`. Check git log / recent diffs for any variant that also took `project` or `projectId`.

5. **Grid column filter UI** — if the grid view has an implicit per-column filter (click a header to add a filter chip), check whether `project` has a bespoke path distinct from the generic column filter.

6. **Operation schemas** — `swissarmyhammer-kanban/src/types/operation.rs` and adjacent schema files — does any operation schema advertise a `project` param for filtering that isn't routed through `filter`?

Document what you found with exact file paths and line ranges in a comment on this card before making any changes. If you find nothing after a thorough search, ASK the user via AskUserQuestion before deleting code — do not guess or force a removal.

**Once located**, remove it and migrate any caller to use `$project-slug` in the filter DSL instead:
- Delete the struct field / parameter / UI control
- Delete the test coverage for the removed path
- Replace call sites that set the old filter with equivalent DSL strings (e.g. `"$myproj"` or appending `" && $myproj"` to an existing filter)
- If the removal is in the Perspective YAML schema, migrate the built-in perspectives that use the field
- Update any snapshot tests that capture the old schema

**Do NOT remove:**
- `project` as a regular field on the task entity (`swissarmyhammer-kanban/builtin/entities/task.yaml`) — this is the storage field, still needed
- `project` parameter on `AddTask` / `UpdateTask` commands and their dispatch — these set the project, not filter
- `project` as a groupable field in the default perspective (grouping is separate from filtering)
- The `project` entity type itself or any of its CRUD commands

## Acceptance Criteria

- [ ] A comment on this card documents what was found (exact file paths, symbol names) or a note that nothing was found after searching all six candidate locations above
- [ ] If something was found: it is removed, including its tests
- [ ] If something was found: all previous call sites using it are migrated to `$project` DSL syntax and still work
- [ ] `cargo build --workspace` passes
- [ ] `pnpm test` in `kanban-app/ui/` passes
- [ ] Opening a perspective that used to filter by project (if any default perspective did) still filters the same tasks, but via a DSL `filter: "$slug"` stored in the perspective
- [ ] No dead imports, dead fields, or unused methods left behind

## Tests

- [ ] Before removing: write or identify a regression test that exercises the current behavior via `$project` DSL. This test must pass before deletion so you know the DSL path covers the old affordance.
- [ ] After removing: all previously-passing tests still pass; the deleted path's tests are either removed or converted to use the DSL
- [ ] If a built-in perspective was migrated, add a round-trip test asserting the YAML serialisation of the migrated perspective (field order, filter string) matches expectation
- [ ] Full workspace: `cargo test --workspace` and `pnpm test` in `kanban-app/ui/`

## Workflow
- Use `/explore` to hunt for the redundant field first — this card is investigation-heavy and wants articulation of what you found before any code changes. If after thorough search nothing fits the user's description, escalate rather than force a removal.