---
assignees:
- claude-code
depends_on:
- 01KRYG2ET5SXTTKQSRNSFTQXTM
- 01KRYG2ZJQAH20VS15NHQH5186
- 01KRYG40E5NB93KM0CPZJE38SQ
- 01KS01B6QGA5MWJFYSWERB51H1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8f80
project: plugin-examples
title: 'E2E test: discovery + layering across the committed examples'
---
## What

Add one capstone integration test, `crates/swissarmyhammer-plugin/tests/example_layering_e2e.rs` (`mod support;`), proving the committed example plugins are discovered and loaded through the real layer-stacking machinery and dispose cleanly.

**Scope note (rescoped 2026-05-19):** The platform deliberately makes a `{ rust }` module *single-activation* — `activate_rust_module` (`src/host.rs`) `remove`s the module, and its doc comment states activation is one-shot. So three `{ rust: "kanban" }` plugins cannot co-load. The capstone is therefore split into two honest tests, loading every bundle EXACTLY as committed (no `{rust}`/server-name rewriting):

### Test 1 — `committed_examples_coload_across_layers`
The two bundles that genuinely coexist (distinct modules, distinct server names):
- stage the real repo bundle `builtin/plugins/kanban-builtin-probe` into a temp **builtin** layer root (consumes `rust: kanban`, server `kanban-builtin-probe`);
- stage the committed `file-notes` example into a temp **project** layer root (consumes `rust: files`, server `fs`);
- expose the `kanban` and `files` Rust modules via the support helpers;
- run `discover_and_load_all` once; assert both plugins are discovered, each with the `FileSource` of the layer it was staged in;
- assert each effect occurred (the kanban board has the probe's effect / note files written);
- `host.unload(...)` both, and assert every server each registered is gone from the live registry afterward.

### Test 2 — `each_committed_example_loads_from_its_layer`
Each remaining committed example — `kanban-tasks`, `multi-module`, `cli-echo` — loaded individually, **fresh `PluginHost` per example**, each staged into a different layer (user / project / builtin respectively) to exercise discovery from every layer source. Assert each is discovered with the correct `FileSource` and loads successfully (its observable effect occurs). `cli-echo` still uses `stage_example_with` for the `__CLI_ECHO_COMMAND__` fixture-path token — that substitution is the fixture binary path, not a scope mutation.

Both tests must run under the `swissarmyhammer_common::test_utils::CurrentDirGuard` temp-CWD guard and be `#[serial_test::serial]`, since the `file-notes` bundle writes via the process CWD.

Update `examples/plugins/README.md` with a closing note that the example suite is exercised together by `example_layering_e2e.rs`.

## Acceptance Criteria
- [ ] `tests/example_layering_e2e.rs` has both tests; no example bundle's `{rust}` id or server name is rewritten (only the cli-echo fixture-path token is substituted).
- [ ] Test 1 co-loads `kanban-builtin-probe` + `file-notes` in one `discover_and_load_all`; each plugin's `FileSource` matches its layer; both effects asserted; after unload neither plugin's servers remain callable.
- [ ] Test 2 loads `kanban-tasks`, `multi-module`, `cli-echo` each from its own layer with a fresh host; each discovered with the correct `FileSource` and loaded.
- [ ] Both tests are `#[serial_test::serial]` and temp-CWD isolated; `git status` clean after running.
- [ ] README in `examples/plugins/` updated with the closing note.

## Tests
- [ ] New: `tests/example_layering_e2e.rs::committed_examples_coload_across_layers`.
- [ ] New: `tests/example_layering_e2e.rs::each_committed_example_loads_from_its_layer`.
- [ ] Run `cargo nextest run -p swissarmyhammer-plugin --test example_layering_e2e` — passes.
- [ ] Run the full crate suite `cargo nextest run -p swissarmyhammer-plugin` — all green, no regressions.

## Workflow
- Use `/tdd` — write the failing tests first, then wire any missing harness pieces.