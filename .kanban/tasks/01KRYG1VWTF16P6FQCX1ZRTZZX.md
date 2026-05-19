---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8980
project: plugin-examples
title: Scaffold examples/plugins/ tree + shared e2e test harness
---
## What

Establish the home for committed example plugins and the shared test-support module every `*_e2e.rs` example test will use.

- Create directory `crates/swissarmyhammer-plugin/examples/plugins/` with a `README.md` that documents the plugin-authoring model (the `plugin.json` manifest fields — `id`, `name`, `version`, `entry`, `provides`; the `entry.ts` exporting an async `load()`; the `@swissarmyhammer/plugin` SDK `Plugin` base class + `makePluginThis`) and indexes the example set this project will add. The README is plugin-author documentation, not a changelog.
- Create test-support module `crates/swissarmyhammer-plugin/tests/support/mod.rs` (consumed by each example test via `mod support;` — Cargo does not compile `tests/support/mod.rs` as its own test binary, only top-level `tests/*.rs`). It provides:
  - `examples_root() -> std::path::PathBuf` — `Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plugins")`.
  - `stage_example(name: &str, layer_root: &Path)` — recursively copies the committed bundle `examples_root()/<name>/` into `<layer_root>/plugins/<name>/` so a real bundle can be discovered from a temp layer root.
  - `build_mcp_server(work_dir: &Path) -> McpServer` — the real MCP-server bootstrap, lifted verbatim from `tests/files_dispatch_e2e.rs::build_mcp_server`, so example tests share one definition.
  - A `TIMEOUT` const mirroring the existing e2e tests.
  - Mark helpers `#[allow(dead_code)]` — each test file uses only a subset.
- Create `crates/swissarmyhammer-plugin/tests/example_plugins_e2e.rs` containing `mod support;` and a smoke test that asserts `examples_root()` is an existing directory and that the README is present.

## Acceptance Criteria
- [x] `crates/swissarmyhammer-plugin/examples/plugins/README.md` exists and documents manifest fields, the `entry.ts`/`load()` contract, and the SDK surface, with a section listing the planned examples.
- [x] `tests/support/mod.rs` exposes `examples_root`, `stage_example`, `build_mcp_server`, and `TIMEOUT`, each with a doc comment.
- [x] `cargo build -p swissarmyhammer-plugin --all-targets` is clean — the `examples/plugins/` tree (no `.rs` files) is not picked up as a Cargo example target.
- [x] `tests/example_plugins_e2e.rs` smoke test passes.

## Tests
- [x] New: `tests/example_plugins_e2e.rs::examples_root_is_present` — asserts `support::examples_root()` is a directory and `README.md` exists inside it.
- [x] Run `cargo nextest run -p swissarmyhammer-plugin --test example_plugins_e2e` — passes.
- [x] Run `cargo build -p swissarmyhammer-plugin --all-targets` — clean, no new warnings.

## Workflow
- Use `/tdd` — write the failing smoke test first, then create the directory/README/harness to make it pass.