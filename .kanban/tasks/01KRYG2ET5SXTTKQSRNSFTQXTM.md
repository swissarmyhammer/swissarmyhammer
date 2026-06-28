---
assignees:
- claude-code
depends_on:
- 01KRYG1VWTF16P6FQCX1ZRTZZX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8a80
project: plugin-examples
title: 'Example plugin: kanban-tasks (operation-tool noun.verb dispatch)'
---
## What

Add the flagship example: a committed plugin that drives the real `kanban` operation tool through the SDK's `_meta`-driven `noun.verb` path dispatch. This proves operation-`_meta` resolution against a real operation tool, with the kanban board as observable state — no filesystem paths involved.

- Create `crates/swissarmyhammer-plugin/examples/plugins/kanban-tasks/plugin.json` — `id: "kanban-tasks"`, `entry: "entry.ts"`, `provides: ["board"]`.
- Create `crates/swissarmyhammer-plugin/examples/plugins/kanban-tasks/entry.ts` — a `Plugin` subclass whose `load()`:
  1. `this.register("board", { rust: "kanban" })`;
  2. adds two tasks via the path form `await this.board.kanban.task.add({ title: "..." })` (this is the form that exercises `io.swissarmyhammer/operations` `_meta` lookup — distinct from the direct `{ op: "..." }` form used by `files_dispatch_e2e`);
  3. lists tasks via `await this.board.kanban.task.list({})` and `this.log.info(...)`s the count.
  Heavily comment the file as the canonical operation-tool example.
- Add an `expose_kanban_module(host)` helper to `tests/support/mod.rs` that exposes the in-process `kanban` operation tool as the Rust module id `"kanban"`. Mirror the production wiring in `apps/kanban-app/src/plugins.rs` — `swissarmyhammer_tools::register_kanban_tools`, `build_tool_modules` (from `swissarmyhammer_tools::mcp::plugin_bridge`), a `ToolContext`/`ToolRegistry`/`ToolHandlers` pointed at a temp `KanbanConfig` board root — then `host.expose_rust_module("kanban", module)`.
- Create `crates/swissarmyhammer-plugin/tests/kanban_tasks_e2e.rs` (`mod support;`): create a temp `KanbanConfig` root with an initialized board, expose the kanban module, `stage_example("kanban-tasks", project_root)`, run `discover_and_load_all`, then read the temp board and assert both tasks the plugin added are present.

If the in-process `kanban` tool genuinely cannot be exposed from the plugin crate's existing dev-dependencies (`swissarmyhammer-tools`), stop and report — do not invent a mock.

## Acceptance Criteria
- [ ] `examples/plugins/kanban-tasks/{plugin.json,entry.ts}` exist; `entry.ts` uses the `this.board.kanban.task.add(...)` / `.task.list(...)` path form and is documented.
- [ ] `tests/support/mod.rs` gains a documented `expose_kanban_module` helper.
- [ ] The e2e test discovers and loads the COMMITTED bundle (via `stage_example`) — no inline plugin source in the test.
- [ ] After load, the temp kanban board contains exactly the two tasks the plugin added.
- [ ] README in `examples/plugins/` updated to describe `kanban-tasks`.

## Tests
- [ ] New: `tests/kanban_tasks_e2e.rs::kanban_tasks_plugin_adds_tasks_via_meta_path` — asserts board state after load.
- [ ] Run `cargo nextest run -p swissarmyhammer-plugin --test kanban_tasks_e2e` — passes.
- [ ] Break-verify: temporarily corrupt one verb name in `entry.ts` (e.g. `task.addd`); confirm the test fails with an `UnknownOperation`-rooted error; revert.

## Workflow
- Use `/tdd` — write the failing test first, then the example bundle + harness helper.