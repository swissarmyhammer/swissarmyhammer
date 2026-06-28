---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
- 01KRREBGRC9WTBRRXB7KS8WQT8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8280
project: plugin-arch
title: 'plugin: files_dispatch_e2e.rs — reference integration test'
---
## What
Write the reference integration test that exercises the whole plugin pipeline through the real `files` MCP server — the canonical example every other capability test follows. `files` is the right target: real in-process rmcp, observable state (the filesystem), verification = "did files land on disk."

`crates/swissarmyhammer-plugin/tests/files_dispatch_e2e.rs` (the crate's integration tests are flat files in `tests/`, not a `tests/integration/` subdir — the new file follows that established convention):
1. Create a `TempDir`, run from it; build `PluginHost::for_tests` pointing at `<tempdir>/plugins/` as the project plugin root.
2. Host wraps the REAL `FilesTool` via the `ToolModuleServer` exposure path (`McpServer::expose_tools_to_plugin_host`) and exposes/registers it — NO mocks.
3. Test writes a real probe plugin to `<tempdir>/plugins/probe/`: a real `plugin.json` + real entry `.ts`. Its `load()` uses only the registered `files` server to: write a probe file, read it back, then write the readback content into a second probe file. (Reporting via a second written file means no special host-side reporter hook is needed.)
4. Trigger discovery; host transpiles, creates a fresh isolate, runs `load()`.
5. Assert BOTH probe files exist with expected contents — first proves `op` dispatch reached the real `files` handler; second proves the return value crossed back through the dispatcher into the isolate.

This is also where the test layout convention is set: one `*_e2e.rs` file per capability. Set up per-test isolation: `TempDir` project root, fresh `PluginHost` (no `static`), watcher scoped to the temp `plugins/` dir.

## Acceptance Criteria
- [x] `files_dispatch_e2e.rs` exists in `tests/`, uses the real `FilesTool` via the `ToolModuleServer` exposure path, no mocks.
- [x] The test runs a real probe plugin through transpile → isolate → `op` dispatch → return marshalling.
- [x] Both probe files are asserted on disk with expected contents.
- [x] Each test owns its `TempDir` and a fresh `PluginHost`; nothing is shared/`static`.

## Tests
- [x] The test IS the deliverable. Run: `cargo test -p swissarmyhammer-plugin --test files_dispatch_e2e` — green.
- [x] Run the full `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- [x] This task delivers a test directly; no `/tdd` cycle — but the test must genuinely fail if any pipeline stage is broken (verified by temporarily corrupting the probe's `op` string: the test failed with `test result: FAILED. 0 passed; 1 failed`, then restored).

## Depends on
PluginHost lifecycle + the `expose_rust_module` wiring for `files`.

## Review Findings (2026-05-18 12:15)

### Nits
- [x] `crates/swissarmyhammer-plugin/tests/files_dispatch_e2e.rs:100` — `write_probe_plugin` hard-codes the literal `"plugins"` in `project_root.join("plugins").join("probe")`. The crate already exports `swissarmyhammer_plugin::PLUGINS_SUBDIR` (re-exported from `discovery.rs` via `lib.rs:43`) precisely so the discovery layout is named in one place — `discovery.rs`'s own tests use the constant. The literal is a silent structural coupling: if the subdir name ever changes, this test breaks without a compile error pointing here. Suggest `project_root.join(swissarmyhammer_plugin::PLUGINS_SUBDIR).join("probe")`.