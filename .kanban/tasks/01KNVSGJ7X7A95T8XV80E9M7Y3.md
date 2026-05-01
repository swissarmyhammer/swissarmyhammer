---
assignees:
- claude-code
depends_on:
- 01KNVSFHMR89NG65Y8EESWF3E7
position_column: done
position_ordinal: fffffffffffffffffffffffffffff580
project: expr-filter
title: 'kanban adapters: implement has_project for task filter contexts'
---
## What

Now that `FilterContext` has a `has_project` method (added in card `01KNVSFHMR89NG65Y8EESWF3E7`), all downstream implementations must provide it. There are **two** adapter copies in the tree — both must be updated for the Rust build to pass.

**Files to modify:**

1. **`swissarmyhammer-kanban/src/task_helpers.rs`** — `impl<'a> swissarmyhammer_filter_expr::FilterContext for TaskFilterAdapter<'a>`. Add:
   ```rust
   fn has_project(&self, project: &str) -> bool {
       self.entity
           .get_str("project")
           .map(|p| p.eq_ignore_ascii_case(project))
           .unwrap_or(false)
   }
   ```
   The task `project` field is a single-reference (not a list) per `swissarmyhammer-kanban/builtin/definitions/project.yaml` (`multiple: false`), so we read it with `get_str`, not `get_string_list`. Matching is case-insensitive to mirror `has_tag` / `has_assignee`. Update the adapter's doc comment (just above the struct) to mention the `$` project atom.

2. **`kanban-app/src/commands.rs`** — `impl<'a> FilterContext for EntityFilterAdapter<'a>`. Add the same `has_project` implementation reading `entity.get_str("project")`. Update the doc comment on the `EntityFilterAdapter` struct to mention the `$` atom.

**Context:**
- These two adapter implementations are near-identical duplicates of each other (they diverged because `task_helpers.rs` is used by `list tasks` / `next task` command paths while `kanban-app/src/commands.rs::list_entities` is the Tauri read path for the UI). Do NOT merge them in this card — the duplication is pre-existing and out of scope.
- The `TaskFilterAdapter` caller in `swissarmyhammer-kanban/src/task/list.rs` and `next.rs` already passes enriched task entities, so `entity.get_str("project")` will work without additional enrichment.

## Acceptance Criteria

- [ ] Both `TaskFilterAdapter` (in `swissarmyhammer-kanban/src/task_helpers.rs`) and `EntityFilterAdapter` (in `kanban-app/src/commands.rs`) implement `has_project` via `entity.get_str("project").map(|p| p.eq_ignore_ascii_case(...)).unwrap_or(false)`
- [ ] `cargo build --workspace` compiles with no errors
- [ ] A task with `project = "auth-migration"` matches the filter `$auth-migration`
- [ ] A task with no project set (field absent or null) does NOT match any `$project` filter
- [ ] Matching is case-insensitive (`$AUTH-MIGRATION` matches a task with `project = "auth-migration"`)

## Tests

Add to `swissarmyhammer-kanban/src/task/list.rs` tests module (mirror `test_list_tasks_filter_by_tag` if present, or add alongside existing list tests):
- [ ] `test_list_tasks_filter_by_project` — create a project, add two tasks (one with project set, one without), call `ListTasks::new().with_filter("$myproj")`, assert only the tagged task is returned
- [ ] `test_list_tasks_filter_by_project_case_insensitive` — assert `$MYPROJ` matches a task with `project = "myproj"`

Add to `swissarmyhammer-kanban/src/task/next.rs` tests module:
- [ ] `test_next_task_filter_by_project` — mirror `test_next_task_filter_by_tag` structure, using `with_filter("$myproj")` and asserting the project task is returned

Add to `kanban-app/src/commands.rs` tests (wherever `list_entities` or `EntityFilterAdapter` is exercised):
- [ ] Integration test that calls `list_entities` with `filter = "$myproj"` and asserts only tasks with that project are returned. If no existing test module exists here, add one following the surrounding file's test conventions.

Test commands:
- [ ] `cargo test -p swissarmyhammer-kanban task::list`
- [ ] `cargo test -p swissarmyhammer-kanban task::next`
- [ ] `cargo test -p kanban-app` (or whatever the kanban-app backend test alias is)

## Workflow
- Use `/tdd` — write the failing `test_list_tasks_filter_by_project` first. It should compile (card 1 added `Project` to the Expr enum) but fail at runtime because `has_project` is not yet implemented. Then implement to make it pass. #expr-filter