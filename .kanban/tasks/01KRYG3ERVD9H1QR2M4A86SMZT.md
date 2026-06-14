---
assignees:
- claude-code
depends_on:
- 01KRYG1VWTF16P6FQCX1ZRTZZX
- 01KRYG2ET5SXTTKQSRNSFTQXTM
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8d80
project: plugin-examples
title: 'Example plugin: multi-module (relative sibling-module imports)'
---
## What

Add a committed example proving the sandboxed module loader resolves **relative sibling imports** inside a real plugin bundle — a multi-file plugin, not a single `entry.ts`.

- Create `crates/swissarmyhammer-plugin/examples/plugins/multi-module/plugin.json` — `id: "multi-module"`, `entry: "entry.ts"`, `provides: ["board"]`.
- Create `crates/swissarmyhammer-plugin/examples/plugins/multi-module/board-helpers.ts` — a sibling module exporting one or more pure helper functions (e.g. a function that builds a normalized task title, and an `async` function that, given a server dispatcher, adds a tagged task). No SDK import of its own beyond shared types.
- Create `crates/swissarmyhammer-plugin/examples/plugins/multi-module/entry.ts` — imports `./board-helpers.ts` with a relative specifier, subclasses `Plugin`, registers `{ rust: "kanban" }` as `"board"`, and in `load()` calls the imported helper(s) to add a task. Document that the relative import is the point of the example.
- Create `crates/swissarmyhammer-plugin/tests/multi_module_e2e.rs` (`mod support;`) — reuse `expose_kanban_module` (added in the kanban-tasks task), `stage_example("multi-module", project_root)` (which must copy ALL files in the bundle, including `board-helpers.ts`), `discover_and_load_all`, and assert the temp board contains the task the helper module produced. The observable effect can only occur if the relative import resolved and the helper ran.

## Acceptance Criteria
- [ ] `examples/plugins/multi-module/{plugin.json,entry.ts,board-helpers.ts}` exist; `entry.ts` imports `./board-helpers.ts`.
- [ ] `stage_example` is confirmed to copy every file in a bundle directory (not just `plugin.json`/`entry.ts`) — adjust it if it does not.
- [ ] The e2e test loads the COMMITTED multi-file bundle and asserts the helper-produced task is on the temp board.
- [ ] README in `examples/plugins/` updated to describe `multi-module`.

## Tests
- [ ] New: `tests/multi_module_e2e.rs::multi_module_plugin_loads_sibling_module` — asserts board state after load.
- [ ] Run `cargo nextest run -p swissarmyhammer-plugin --test multi_module_e2e` — passes.
- [ ] Break-verify: temporarily rename `board-helpers.ts`; confirm the test fails at module resolution; revert.

## Workflow
- Use `/tdd` — write the failing test first, then the multi-file example bundle.