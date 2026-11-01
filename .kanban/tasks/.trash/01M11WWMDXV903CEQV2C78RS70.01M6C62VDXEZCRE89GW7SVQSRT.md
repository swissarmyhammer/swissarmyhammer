---
assignees:
- claude-code
position_column: todo
position_ordinal: fff580
title: Remove project support from the kanban engine; tags replace it
---
Remove the `project` entity and the `project` field from the kanban engine. Tags do the same work. A task with a tag is a task in a project. The two mechanisms are redundant, and `project` is the weaker one: it holds one value, tags hold many.

Old boards must continue to load. A task file that still has a `project:` key must open without an error. The engine must ignore extra keys in the YAML front matter.

## What to remove

Engine crate `crates/swissarmyhammer-kanban`:

- `src/project/` — the module and its five commands: `add.rs`, `get.rs`, `update.rs`, `delete.rs`, `list.rs`, `mod.rs`.
- `src/commands/paste_handlers/task_into_project.rs` — the paste handler.
- `src/schema.rs` — the five project operations in `kanban_operations()`, the optional `project` parameter on `list tasks`, and the two tests `test_project_param_absent_from_signatures_but_in_full_schema` and the `add project` / `list projects` assertions near line 558.
- `src/types/operation.rs` — the project operation variants.
- `src/dispatch.rs` — the project dispatch arms.
- `src/task/list.rs` — the project filter. Also `add.rs`, `update.rs`, `next.rs`, `search.rs`, and `task_helpers.rs`, which all carry a project parameter.
- `src/defaults.rs`, `src/error.rs`, `src/lib.rs`, `src/entity/add.rs`, `src/commands/ui_commands.rs`, `src/commands/clipboard_commands.rs`, `src/commands/paste_handlers/task_into_column.rs`.
- Built-in data: `builtin/entities/project.yaml`, `builtin/definitions/project.yaml`, `builtin/views/projects-grid.yaml`. Remove the `- project` line from the field list in `builtin/entities/task.yaml`. Check `builtin/commands/perspective.yaml`.
- Tests: `src/dispatch/tests/tasks.rs`, `src/scope_commands/tests/entity_add.rs`, `src/scope_commands/tests/dynamic.rs`, `src/scope_commands/tests/templates.rs`.

MCP tool `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`: the tests `test_add_project`, `test_get_project`, `test_list_projects`, `test_delete_project` (near line 1600).

Documentation: `doc/src/reference/kanban-cli.md`.

## The GUI

The engine is the scope of this card. Change `apps/kanban-app` only as much as it takes to keep the build and the tests green — it calls the removed operations from `src/state.rs`, `src/commands.rs`, and `src/deeplink.rs`. Do not start new GUI work here. If the GUI needs more than a removal, make a second card.

## Backward compatibility

`parse_frontmatter_body` in `crates/swissarmyhammer-entity/src/io.rs` reads the front matter into a field map. It does not compare the keys against a schema. The field pipeline in `crates/swissarmyhammer-entity/src/context.rs` walks `field_defs` and touches only the fields it knows. So an extra key already passes through today. Prove it, do not assume it.

Requirements:

1. A task file with `project: 01ABC...` in its front matter loads without an error, and the task is complete in every other way.
2. A task file with any other unknown key (for example `junk: 42`) also loads without an error.
3. An old board directory that still holds a `projects/` folder opens without an error.
4. No `deny_unknown_fields` attribute is added to any entity or task type.

## One decision to record

An unknown key is ignored on load. Say what happens to it on the next write: does the engine keep the key in the file, or drop it? Recommendation: drop it. The field is gone, and a board on disk is easier to reason about when it holds only live fields. Write the choice in a code comment at the place that makes it.

## Acceptance

- No `project` operation is in the schema, and `kanban_operations()` proves it.
- `list tasks` with a `project` parameter is ignored, not an error.
- A regression test loads a fixture task file that carries `project:` and one more unknown key, and it asserts the load is good. The test must fail before the change.
- `cargo build` and the full test suite are green across the workspace, the GUI app included.
#kanban #refactor