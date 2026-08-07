---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzdjxjq1ek2425yhnntm9p3s
  text: |-
    ### test — verified, pre-existing, unrelated to ^cysg4xv

    Evidence the claim is real and unrelated:
    - `git diff --stat HEAD -- crates/mirdan apps/mirdan-app apps/mirdan-cli` is empty. `crates/mirdan` is byte-identical to the previous commit (HEAD `ede9b46ca`). The assertion-census change (^cysg4xv) touches only `swissarmyhammer-sem` and `swissarmyhammer-validators`. This bug pre-dates that change and cannot be caused by it.
    - `cargo nextest run --workspace` (13708 tests, 0 failed) is green and stays green across a full run. Nextest spawns one process per test, so the race below cannot manifest there.
    - Reproduced with `RUST_TEST_THREADS=32 cargo test -p mirdan --lib`, looped 15x: 6 of 15 runs failed, with a rotating cast of failing tests — not just the two named in this card:
      - `install::tests::test_deploy_plugin_creates_files`
      - `sync::tests::test_sync_validator_missing`, `test_sync_validator_present_in_project_dir`
      - `install::tests::test_install_tool_from_mcp_config_then_uninstall`, `test_deploy_and_uninstall_plugin`, `test_install_tool_from_tool_md_content`, `test_e2e_tool_install_list_uninstall`
      - `lockfile::tests::test_lockfile_root_for_scope_project`

    Root cause found:
    - `crates/mirdan/src/test_support.rs` documents the contract: any test mutating CWD, `HOME`, or `MIRDAN_AGENTS_CONFIG` (all process-global) must use `#[serial_test::serial(cwd)]` so they mutually exclude.
    - The crate actually uses three different serial_test lock groups that do NOT exclude each other: bare `#[serial]` (most of `install/tests.rs`, `new.rs`, `sync.rs`, `list.rs` — including both tests named in this card), `#[serial(cwd)]` (`install/tests.rs` lines 84/197/258, `tool_install.rs` lines 250/257/282), and `#[serial(home_env)]` (`lockfile.rs` line 335).
    - Because these are distinct named locks, a bare-`#[serial]` test that calls raw `std::env::set_current_dir` can run concurrently with a `#[serial(cwd)]` test that redirects `HOME`/`MIRDAN_AGENTS_CONFIG` via `IsolatedTestEnvironment`/`MirdanConfigGuard`. `test_deploy_plugin_creates_files` reads agent config via `agents::load_agents_config()` → `MIRDAN_AGENTS_CONFIG`/`HOME`; if a concurrently running `#[serial(cwd)]` test has pointed those at a fake single-agent config (no `claude-code` id), `deploy_plugin` gets an empty target list and returns `"no agents with plugin support detected"`. `test_new_validator_creates_structure` races on raw `set_current_dir` the same way, landing `run_new_validator` in the wrong directory.

    Fix direction for whoever picks this up: unify all CWD/HOME/`MIRDAN_AGENTS_CONFIG`-mutating tests onto the single `#[serial(cwd)]` group per the existing doc comment in `test_support.rs` — this is broader than the two tests named in the title, it affects `install/tests.rs`, `new.rs`, `sync.rs`, `list.rs`, and `lockfile.rs`'s `home_env` group too. Left as-is per instructions (unrelated to ^cysg4xv, no fix applied here).
  timestamp: 2026-08-07T07:45:04.737555+00:00
position_column: todo
position_ordinal: ff8e80
title: Two mirdan tests fail under the parallel suite but pass alone
---
`cargo test -p mirdan --lib` fails these two, and both pass when run alone with `--test-threads=1`:

- `install::tests::test_deploy_plugin_creates_files` — `Validation("no agents with plugin support detected. Plugins are currently supported by Claude Code")`
- `new::tests::test_new_validator_creates_structure` — `assertion failed: result.is_ok()`

The failure set changes between runs, so this is shared mutable state across threads — agent detection reading the process environment or the current directory, which another test changes underneath it. Found while running the suite for task ^cysg4xv, whose change touches neither crate.

Work:
- Find what the two tests read that another test writes (env var, current directory, or a home/config path).
- Give each test its own isolated state through the existing RAII guard (`CurrentDirGuard`) or `serial_test`, never by adding a production API to work around the test environment.
- Re-run the whole `-p mirdan` suite in parallel and confirm it is green over several runs.