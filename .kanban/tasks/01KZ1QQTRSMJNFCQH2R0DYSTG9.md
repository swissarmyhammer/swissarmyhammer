---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6qde56cjyqybx3dhh4a1nm
  text: |-
    ### implement — changed
    - evidence: `cargo nextest run -p swissarmyhammer-tools --lib` — 1286/1286 passed (1 pre-existing leaky, unrelated to shell). `cargo clippy -p swissarmyhammer-tools --lib --tests --no-deps` — zero warnings.
    - Files changed:
      - `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — the 3 named watcher tests plus `test_known_tool_names_matches_registry` (a 4th call site in the same file, found while verifying) now use `ShellExecuteTool::new_isolated()` instead of `ShellExecuteTool::new()`. Added regression test `test_new_isolated_does_not_create_shell_dir_in_crate_directory`.
      - `crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs` — `register_shell_tool` now pins CWD to a `tempfile::TempDir` via `CurrentDirGuard` before calling `ShellExecuteTool::new()` (can't reach `new_isolated()` from this external test crate). Added regression test `test_register_shell_tool_does_not_create_shell_dir_in_crate_directory`.
      - `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs` — 3 unit tests calling `create_fully_registered_tool_registry()` (which internally hits the same `ShellExecuteTool::new()` path) now wrap the call in a `CurrentDirGuard`-pinned temp dir.
      - `crates/swissarmyhammer-tools/tests/integration/mcp_server_paritys.rs` — `get_mcp_tools()` (same `create_fully_registered_tool_registry()` call) fixed the same way.
    - Verified each fixed test/module individually with targeted `cargo nextest run` + `ls` checks: no `.shell` dir appears after any of them.
    - Hit and fixed a self-inflicted deadlock: the first version of the `file_size_limits.rs` regression test wrapped an *outer* `CurrentDirGuard` around `register_shell_tool`, which takes its own guard internally — `std::sync::Mutex` isn't reentrant, so this hung the whole test binary for 300s until nextest killed it. Fixed by relying on Cargo's existing package-root CWD guarantee instead of chdir'ing a second time.
    - **Scope discovery**: `cargo nextest run -p swissarmyhammer-tools --lib` (fully green, 1286/1286) *still* recreates `.shell` in the crate directory. Root cause: `McpServer::new(...)` (~17 call sites in `server.rs`, 3 in `mcp/tests.rs`) registers the real shell tool via `register_all_tools -> register_shell_tools -> ShellExecuteTool::new()`. This is a distinct function/file set from what this card named, ~5x the call sites, and plausibly wants a design decision (test-only `McpServer::new_isolated()` vs. ~20 individual `CurrentDirGuard` wraps). Filed as new task ^2kqcswb with the full call-site list rather than silently expanding this card's scope.
    - Also discovered and restored: `rm -rf .shell` during investigation deleted a **git-tracked** `crates/swissarmyhammer-tools/.shell/.gitignore` (auto-generated self-ignoring gitignore that got accidentally committed during the `375d20b16` crates/ move). Restored via `git checkout -- crates/swissarmyhammer-tools/.shell/.gitignore` — untouched, not part of this task's scope.
    - Acceptance criterion "no test in the crate calls `ShellExecuteTool::new()`" is satisfied for every call site this card's scope covers. The broader "leaves no `.shell` directory" criterion is not yet true crate-wide — tracked in ^2kqcswb.

    step: implement
    outcome: changed
    evidence: 4 files changed — crates/swissarmyhammer-tools/src/mcp/tool_config.rs, crates/swissarmyhammer-tools/src/mcp/tool_registry.rs, crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs, crates/swissarmyhammer-tools/tests/integration/mcp_server_paritys.rs; new follow-up task ^2kqcswb filed for the remaining McpServer::new() call sites
    task: ^0dystg9
  timestamp: 2026-08-04T15:48:57.638424+00:00
- actor: claude-code
  id: 01kz6rncpx3bed3dd2bn4s10r9
  text: |-
    ### test — green

    - evidence: `cargo nextest run --workspace` — 13497 passed, 0 failed, 0 skipped; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.

    Verified the 4 named files (`tool_config.rs`, `tool_registry.rs`, `file_size_limits.rs`, `mcp_server_paritys.rs`) no longer create `.shell` in `crates/swissarmyhammer-tools/`.

    While re-verifying the task's own acceptance criterion ("no `.shell` dir in the crate directory"), found the fix was still incomplete for two files this card didn't touch — same bug class (`register_shell_tools`/`create_fully_registered_tool_registry` calling `ShellExecuteTool::new()`, unguarded CWD), reproduced empirically (removed `.shell`, ran the specific tests, watched it get recreated), then fixed and re-verified it stays gone:

    - `crates/swissarmyhammer-tools/src/health_registry.rs` — 5 `#[tokio::test]` fns (`test_collect_all_health_checks`, `test_web_search_health_check_included`, `test_all_tool_groups_enumerated`, `test_review_validators_health_check_included`, `test_prompt_health_checks_included`) called `collect_all_health_checks()`, which calls `register_shell_tools(&mut tool_registry)` internally, unguarded. Fixed: each now pins CWD to a fresh `tempfile::TempDir` via `CurrentDirGuard` before the call, with `#[serial_test::serial(cwd)]` added to match this file's existing CWD-mutating tests.
    - `apps/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs` — 4 tests (`test_mcp_tools_are_registered`, `test_cli_categories_are_available`, `test_tool_schemas_are_claude_api_compatible` call `register_shell_tools` directly; `test_operation_tools_split_wire_and_full_schemas` calls `create_fully_registered_tool_registry()`). Same fix pattern applied to all 4.

    Both re-verified: ran the specific tests standalone after deleting `.shell`, confirmed no recreation, then re-ran the full workspace suite + clippy — still green, and `git status --porcelain --ignored` on both crate dirs shows no new untracked `.shell/log` beyond the pre-existing `.gitignore`.

    Confirmed ^2kqcswb (the already-filed follow-up for the `McpServer::new()` call sites in `server.rs`/`tests.rs`, ~20 sites, correctly scoped out of this card as a design decision) still accurately describes that remaining gap. Added one more site to it: `crates/swissarmyhammer-mcp-proxy/tests/integration/end_to_end.rs::test_proxy_filters_tool_discovery`, which has no `CurrentDirGuard` and pollutes `crates/swissarmyhammer-mcp-proxy/.shell`. Checked `review/tests.rs`, `review_progress_stdio_test.rs`, and `mcp_integration.rs`'s `test_cli_tool_registry_matches_server_registry` — all already guard CWD correctly before their `McpServer::new*` calls, no action needed there.

    - next: this card's named scope is done and verified; ^2kqcswb remains open to close the wider `McpServer::new()` gap (now including the mcp-proxy site).
  timestamp: 2026-08-04T16:10:46.877839+00:00
position_column: doing
position_ordinal: '8380'
title: 'shell tests: stop `ShellExecuteTool::new()` in tests from making a `.shell` dir in the crate directory'
---
## What

`cargo nextest run -p swissarmyhammer-tools` creates a `.shell` directory in
`crates/swissarmyhammer-tools/`. Nextest runs each test binary with the crate
directory as the CWD, and `ShellExecuteTool::new()` calls `ShellState::new()`,
which makes `.shell` relative to the CWD.

Found while working ^mbran97. The directory is not tracked by git, but it is
litter in the source tree, and it makes test runs write outside their temp
sandbox.

Tests that call `ShellExecuteTool::new()` instead of the test-only
`ShellExecuteTool::new_isolated()`:

- `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — three tests in the
  watcher `mod tests` block.
- `crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs` — the
  `register_shell_tool` helper.

`new_isolated()` is `#[cfg(test)]` and `pub(crate)`, so the integration test in
`tests/` cannot reach it. That one needs a different route — a `CurrentDirGuard`
on a temp dir, or a crate-public test constructor.

### Subtasks

- [x] Move the `tool_config.rs` tests to `new_isolated()`. Done for the 3
      watcher tests AND a 4th call site in the same file
      (`test_known_tool_names_matches_registry`, which called the production
      `register_shell_tools(&mut registry)` — replaced with
      `ShellExecuteTool::new_isolated()` registered directly, since the test
      only checks tool *names*).
- [x] Give `file_size_limits.rs` an isolated state directory. Done via a
      `CurrentDirGuard` pinned to a fresh `tempfile::TempDir` inside
      `register_shell_tool`, since `new_isolated()` is unreachable from this
      external integration test crate.
- [x] Add a test that proves the fixed call sites don't create `.shell` in the
      crate directory. Added
      `test_new_isolated_does_not_create_shell_dir_in_crate_directory` in
      `tool_config.rs` and
      `test_register_shell_tool_does_not_create_shell_dir_in_crate_directory`
      in `file_size_limits.rs` — both chdir/check against the real
      `CARGO_MANIFEST_DIR` and pass.

### Additional sites fixed beyond the original scope (same bug class, same files/call chains)

- `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs` — 3 unit tests
  (`test_create_fully_registered_tool_registry_*`) called
  `create_fully_registered_tool_registry()`, which internally calls the real
  `register_shell_tools`. Fixed with a `CurrentDirGuard`-pinned temp dir.
- `crates/swissarmyhammer-tools/tests/integration/mcp_server_paritys.rs` —
  `get_mcp_tools()` called the same `create_fully_registered_tool_registry()`.
  Fixed the same way.

### Known remaining gap — filed as ^2kqcswb

Verified empirically: after all the fixes above, `cargo nextest run
-p swissarmyhammer-tools --lib` (1286/1286 tests green) **still** recreates
`.shell` in the crate directory. Root cause: `McpServer::new(...)` — used by
~17 tests in `server.rs` and 3 in `mcp/tests.rs` — calls `register_all_tools`
-> `register_shell_tools` -> `ShellExecuteTool::new()` internally. This is a
different function, in different files, with ~5x the call sites of this
card's named scope, and fixing it cleanly likely wants a design decision (a
test-only `McpServer::new_isolated()` vs. ~20 individual `CurrentDirGuard`
wraps) rather than a mechanical edit. Filed as ^2kqcswb with the exact call
site list.

## Acceptance Criteria

- [ ] `cargo nextest run -p swissarmyhammer-tools` leaves no `.shell` directory
      in `crates/swissarmyhammer-tools/`. **Partially met**: true for every
      test this card touched (`tool_config.rs`, `tool_registry.rs`,
      `file_size_limits.rs`, `mcp_server_paritys.rs`, verified with targeted
      nextest runs). **Not yet true crate-wide** — `server.rs`/`tests.rs`'s
      `McpServer::new()` call sites still recreate it (see ^2kqcswb).
- [x] No test in the crate calls `ShellExecuteTool::new()` directly anymore
      (the literal constructor named in this card's scope — `tool_config.rs`
      and `file_size_limits.rs` — now use `new_isolated()` / a
      `CurrentDirGuard`-pinned temp dir). #bug #shelltool #test-hygiene #tools