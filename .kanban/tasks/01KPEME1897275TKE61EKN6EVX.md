---
assignees:
- claude-code
depends_on:
- 01KPEN0JMTVSCW8PZW6RRD0NC3
position_column: todo
position_ordinal: d580
title: 'Commands: retire DeleteProjectCmd and project.delete registration (superseded by entity.delete auto-emit)'
---
## What

Once the auto-emit migration lands (card 01KPEN0JMTVSCW8PZW6RRD0NC3), `entity.delete` surfaces on project cards automatically and correctly dispatches to `project::DeleteProject` via `DeleteEntityCmd`. The legacy type-specific `project.delete` command + its Rust impl become dead code. This card removes them cleanly.

### Why this is its own card

- The YAML cleanup on `project.yaml` happens inside the migration card (deletes cross-cutting opt-ins). What remains is the Rust + registry cleanup:
  - Retire `project.delete` from `swissarmyhammer-commands/builtin/commands/entity.yaml` if it's still declared there (it was originally planned to move to a `project.yaml` command file in card 01KPEM93Z47JSME10BY1JBGTFM — do not create one; just let `project.delete` disappear).
  - Delete `DeleteProjectCmd` struct, impl, and tests from `swissarmyhammer-kanban/src/commands/project_commands.rs`.
  - Remove `project.delete` registration from `swissarmyhammer-kanban/src/commands/mod.rs::register_commands()`.
  - Update `register_commands_returns_expected_count` expected total.
  - Audit `AddProjectCmd` — the code comments say `project.add` is retired in favor of dynamic `entity.add:project`, so `AddProjectCmd` is likely dead too. Delete if unreferenced; leave if still registered.
  - Grep `kanban-app/ui/src/**` and `kanban-app/src/**` for string `"project.delete"` — migrate any dispatch sites to `entity.delete` with `target: "project:<id>"`.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml` (or the yet-to-be-created `project.yaml` — verify in card 01KPEM93Z47JSME10BY1JBGTFM's output).
- `swissarmyhammer-kanban/src/commands/project_commands.rs` — delete `DeleteProjectCmd`; audit `AddProjectCmd`.
- `swissarmyhammer-kanban/src/commands/mod.rs` — unregister; update count test.
- Frontend: `kanban-app/ui/src/**`, `kanban-app/src/**` — grep for `"project.delete"`.

### Subtasks

- [ ] Delete `DeleteProjectCmd` struct, `impl Command`, and tests.
- [ ] Remove `project.delete` registration from `register_commands`.
- [ ] Audit `AddProjectCmd` — delete if unreferenced.
- [ ] Update `register_commands_returns_expected_count` total.
- [ ] Grep frontend for `"project.delete"`; migrate dispatches.

## Acceptance Criteria

- [ ] Right-click on a project card shows **Delete Project** and deletes the project (verified by matrix test in card 01KPEMFBBFRE1JWRJ9AXQFVSEB).
- [ ] `DeleteProjectCmd` no longer exists.
- [ ] `register_commands()` does not register `project.delete`.
- [ ] `grep -r '"project.delete"' kanban-app/` returns no dispatch sites.
- [ ] `test_all_yaml_commands_have_rust_implementations` still passes.

## Tests

- [ ] Add `delete_entity_deletes_project` in `swissarmyhammer-kanban/src/commands/entity_commands.rs` tests (mirror `delete_entity_deletes_tag`): create a project, call `DeleteEntityCmd::execute` with `target = "project:<id>"`, assert the project is gone.
- [ ] Remove the `DeleteProjectCmd` test block from `project_commands.rs`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban` — all green.

## Workflow

- Use `/tdd` — add `delete_entity_deletes_project` first; it passes as soon as the prior cards land. Then delete the dead code and re-run tests.

#commands

Depends on: 01KPEN0JMTVSCW8PZW6RRD0NC3 (auto-emit must surface entity.delete on projects first)