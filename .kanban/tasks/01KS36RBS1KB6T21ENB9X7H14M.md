---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd080
project: builtin-commands
title: 'Builtin plugin: task commands (port task.yaml)'
---
## What

Port `crates/swissarmyhammer-kanban/builtin/commands/task.yaml` (3 commands: `task.move`, `task.untag`, `task.doThisNext`) to a builtin TypeScript plugin.

Files:
- `builtin/plugins/task-commands/index.ts` — entry; in `load()`, calls `ensureServices(this, ["commands"])` then `registerCommands(this, [...])` with the three commands (see SDK helpers task for the convention). Each command's `available` and `execute` callbacks call into the kanban MCP server (`this.kanban.task.move(...)`, etc.) — the plugin contains no business logic, only the wiring.

The class-prop fields (`name`, `description` per b13d2f3) sit on the `Plugin` subclass. The id is the directory name (`task-commands`) per the plugin-tsonly migration.

Each registration carries the full UI metadata from the original YAML: `keys` (vim/cua bindings), `scope` (`entity:task`), `undoable`, `context_menu`, `params` (with `from: scope_chain`, `entity_type`, etc.). Behavior must be identical to the YAML-driven version.

`available` checks: e.g., `task.move` requires a task in scope and a column target; the callback returns `{ ok: false, reason: "Select a task first" }` otherwise.

`execute` impl: typically one MCP call into the kanban server (`task.move`, `task.untag`, `task.doThisNext` — those operations already exist in `swissarmyhammer-kanban`'s MCP surface).

## Acceptance Criteria
- [ ] `builtin/plugins/task-commands/` is discoverable by the platform's builtin layer
- [ ] After plugin load, `list command { scope: "entity:task" }` returns all 3 task commands with their full metadata
- [ ] `execute command { id: "task.move", ctx: {...} }` end-to-end moves a task in the underlying kanban store
- [ ] `available command` reflects the original YAML's preconditions
- [ ] `load()` calls `ensureServices` before `registerCommands` (the convention)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_task_commands_e2e.rs` — load a real `PluginHost::for_tests` with the builtin layer including `task-commands`; assert `list` returns the 3 commands with matching metadata; execute `task.move` and observe the kanban store mutation
- [ ] Each command has a regression test that locks its `keys`, `scope`, `params`, and `undoable` flag against the YAML baseline
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_task_commands_e2e` passes

## Workflow
- Use `/tdd` — write the metadata-fidelity tests first (they fail if any field is dropped). The integration test is the headline acceptance.

Depends on the Command service being live and the SDK helpers (`ensureServices`, `registerCommands`) being available.