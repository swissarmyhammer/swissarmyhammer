---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
- 01KRREBGRC9WTBRRXB7KS8WQT8
position_column: todo
position_ordinal: '9380'
project: plugin-arch
title: 'plugin: files_dispatch_e2e.rs — reference integration test'
---
## What
Write the reference integration test that exercises the whole plugin pipeline through the real `files` MCP server — the canonical example every other capability test follows. `files` is the right target: real in-process rmcp, observable state (the filesystem), verification = "did files land on disk."

`crates/swissarmyhammer-plugin/tests/integration/files_dispatch_e2e.rs`:
1. Create a `TempDir`, run from it; build `PluginHost::for_tests` pointing at `<tempdir>/plugins/` as the project plugin root.
2. Host wraps the REAL `FilesTool` in an `InProcessServer` and exposes/registers it — NO mocks.
3. Test writes a real probe plugin to `<tempdir>/plugins/probe/`: a real `plugin.json` + real entry `.ts`. Its `load()` uses only the registered `files` server to: write a probe file in cwd, read it back, then write the readback content into a second probe file. (Reporting via a second written file means no special host-side reporter hook is needed.)
4. Trigger discovery; host transpiles, creates a fresh isolate, runs `load()`.
5. Assert BOTH probe files exist with expected contents — first proves `op` dispatch reached the real `files` handler; second proves the return value crossed back through the dispatcher into the isolate.

This is also where the test layout convention is set: `tests/integration/`, one `*_e2e.rs` file per capability, matching the existing code-context / skill `*_e2e.rs` pattern. Set up per-test isolation: `TempDir` project root, fresh `PluginHost` (no `static`), fresh `ServerRegistry`, watcher scoped to the temp `plugins/` dir.

## Acceptance Criteria
- [ ] `files_dispatch_e2e.rs` exists under `tests/integration/`, uses the real `FilesTool` via `InProcessServer`, no mocks.
- [ ] The test runs a real probe plugin through transpile → isolate → `op` dispatch → return marshalling.
- [ ] Both probe files are asserted on disk with expected contents.
- [ ] Each test owns its `TempDir` and a fresh `PluginHost`; nothing is shared/`static`.

## Tests
- [ ] The test IS the deliverable. Run: `cargo test -p swissarmyhammer-plugin --test files_dispatch_e2e` (or the integration harness path) — green.
- [ ] Run the full `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- This task delivers a test directly; no `/tdd` cycle — but the test must genuinely fail if any pipeline stage is broken (verify by temporarily breaking one stage locally).

## Depends on
PluginHost lifecycle + the `expose_rust_module` wiring for `files`.