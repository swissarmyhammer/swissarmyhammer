---
assignees:
- claude-code
depends_on:
- 01KNVSGJ7X7A95T8XV80E9M7Y3
- 01KNVSK9V84X79TKGPP9XCYKFH
position_column: done
position_ordinal: ffffffffffffffffffffffae80
project: expr-filter
title: locate and remove redundant project filtering field
---
## Resolution: no-op — satisfied by the DSL work

Marked done by user decision after investigation confirmed there is no dedicated redundant project filter field in the current tree.

## Investigation Summary

Searched all six candidate locations from the original card description:

| # | Location | Finding |
|---|---|---|
| 1 | Tauri/backend command parameters (`list_entities`, `ListTasks`, `NextTask`, `dispatch.rs`) | CLEAN. No `project` filter param. Only setter-path `project` usage in AddTask/UpdateTask. |
| 2 | Perspective struct (`swissarmyhammer-perspectives/src/types.rs`) | CLEAN. Fields are `id`, `name`, `view`, `fields`, `filter`, `group`, `sort` only. Zero hits for `project` in the entire crate. |
| 3 | Frontend filter editor UI (`kanban-app/ui/src/`) | CLEAN. No `projectFilter` / `project_filter` / `filterByProject` / `setProjectFilter`. All `project` references are grouping tests, `$project` DSL autocomplete tests, or unrelated path strings. |
| 4 | `refreshBoards` / entity loading | CLEAN. Signature is `(boardPath?, taskFilter?)`; no project param. |
| 5 | Grid column filter UI | CLEAN. No column-filter UI exists. The filter DSL expression bar is the only filter affordance. |
| 6 | Operation schemas | CLEAN. `project` only appears as `Noun::Project`/`Noun::Projects` for CRUD routing. |

## Conclusion

Every `project`-related code path falls into a do-not-remove category:
- Storage field on the task entity
- Setter on `AddTask`/`UpdateTask` + dispatch routing
- The `project` entity type and CRUD commands
- The new `$project` DSL path (cards 1-4)

The $project filter DSL added by cards 1-4 is the first and only project-filtering path. There was nothing pre-existing to remove. No code changes required.