---
assignees:
- claude-code
depends_on:
- 01KMN0A2T6RHDE4DV7T6PABR0F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffc280
project: task-card-fields
title: Remove order field from project entity
---
## What

The `order` field on the project entity serves no purpose and should be removed. Unlike columns (which need a deliberate left-to-right workflow order), projects have no meaningful positional semantics — they are referenced by task assignment and should be presented alphabetically by name.

Remove the `order` field from the project entity definition, the `AddProject`/`UpdateProject` operations, the list-sort logic, the grid view, and the dispatcher. Update tests accordingly. Existing `.kanban/projects/*.yaml` files have a stale `order:` key that will simply be ignored after the field is removed from the schema (entities are generic key/value maps) — do NOT write a migration, do NOT rewrite the existing YAML files.

### Files to modify

- **`swissarmyhammer-kanban/src/project/add.rs`**
  - Remove the `order: Option<usize>` field from `AddProject` struct
  - Remove `with_order()` builder method
  - Remove the auto-increment order computation block (lines ~79-89)
  - Remove `entity.set("order", json!(order))` from `execute()`
  - Remove `"order": entity.get("order")...` from `project_entity_to_json()`
  - Delete `test_add_project_auto_order` test
  - Update `test_add_project` and `test_add_project_with_all_fields` — remove `order` assertions and `.with_order(5)` calls
- **`swissarmyhammer-kanban/src/project/update.rs`**
  - Remove the `order: Option<usize>` field from `UpdateProject` struct
  - Remove `with_order()` builder method
  - Remove the `if let Some(order) = self.order { ... }` block from `execute()`
  - Update `#[operation(description = ...)]` macro — change `"Update a project's name, description, color, or order"` to `"Update a project's name, description, or color"`
  - Update `test_update_project_all_fields` — remove `.with_order(42)` and `assert_eq!(result["order"], 42)`
- **`swissarmyhammer-kanban/src/project/list.rs`**
  - Change `projects.sort_by_key(|p| p.get("order")...)` to sort by `name` (ascending, case-insensitive)
  - Update doc comment from "List all projects ordered by their `order` field." to "List all projects sorted alphabetically by name."
  - Update `#[operation(description = ...)]` from `"List all projects ordered by position"` to `"List all projects sorted alphabetically by name"`
  - Rewrite `test_list_projects_sorted_by_order` as `test_list_projects_sorted_by_name` — add projects out of alphabetical order, assert result is sorted by name
- **`swissarmyhammer-kanban/src/dispatch.rs`**
  - In `dispatch_add_project()` (around line 504): remove the `if let Some(o) = parse_order(op) { cmd = cmd.with_order(o); }` block
  - In `dispatch_update_project()` (around line 522): remove the same block
  - Delete the `parse_order` helper function (around line 471) — it is only used by project dispatch; columns have their own inline parsing
- **`swissarmyhammer-kanban/builtin/entities/project.yaml`**
  - Remove `- order` from the `fields:` list
- **`swissarmyhammer-kanban/builtin/views/projects-grid.yaml`**
  - Remove `- order` from `card_fields:`

### Out of scope
- Do NOT touch `column`, `perspective`, or `tag` order — those entities genuinely need ordering
- Do NOT migrate or rewrite existing `.kanban/projects/*.yaml` files — stale `order:` keys are harmless
- Do NOT change sort behavior elsewhere (task list, board get, etc.) — none reference project order

## Acceptance Criteria

- [x] `AddProject` struct has no `order` field; MCP schema regenerates without `order` for `add project`
- [x] `UpdateProject` struct has no `order` field; MCP schema regenerates without `order` for `update project`
- [x] `ListProjects` returns projects sorted alphabetically by `name` (case-insensitive)
- [x] `project.yaml` entity definition lists only `name, description, color` in `fields:`
- [x] `projects-grid.yaml` view lists only `name, description, color` in `card_fields:`
- [x] `parse_order` helper is deleted from `dispatch.rs`
- [x] Existing `.kanban/projects/*.yaml` files still load without error (stale `order:` keys ignored)
- [x] `cargo build -p swissarmyhammer-kanban` succeeds with no warnings about unused imports or dead code

## Tests

- [x] Update `swissarmyhammer-kanban/src/project/add.rs` tests: delete `test_add_project_auto_order`; scrub `order` assertions from `test_add_project` and `test_add_project_with_all_fields`
- [x] Update `swissarmyhammer-kanban/src/project/update.rs` tests: scrub `order` from `test_update_project_all_fields`
- [x] Replace `test_list_projects_sorted_by_order` in `swissarmyhammer-kanban/src/project/list.rs` with `test_list_projects_sorted_by_name` — add projects in non-alphabetical insertion order (e.g. `zz-last`, `aa-first`, `mm-middle`), assert the returned `projects` array is `[aa-first, mm-middle, zz-last]` by name
- [x] Add a regression test in `swissarmyhammer-kanban/src/project/add.rs` asserting the returned JSON from `AddProject` does NOT contain an `order` key
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban project::` — all tests in the project module pass
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban` — full kanban test suite passes
- [x] Run: `cargo clippy -p swissarmyhammer-kanban -- -D warnings` — no new warnings (especially no dead-code warnings for `parse_order`)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #fields