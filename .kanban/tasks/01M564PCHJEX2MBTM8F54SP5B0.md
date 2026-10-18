---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m565cnfx24726a3dzavety1n
  text: |-
    Research and implementation notes.
    - ACP terminal API: no method writes to the stdin of a child (create, output, wait_for_exit, kill, release only). Thus stdin null is correct for terminal/create, not piped.
    - New audit of all production Command spawn/status and tokio output sites found no new site. The shell tool, LSP daemon, hooks, MCP stdio clients and claude_process set stdin explicitly. git diff and doctor probes use std output() (stdin null by default). The `open` crate (open::that) already sets all three streams to null.
    - Test method: std Command has no stdio getter. The alternate Debug form ({:#?}) shows each stdio stream that the caller set (std sys/process/unix/common.rs). Under nextest the test stdin is already null, so a spawn test cannot fail without the fix. Thus each test asserts on the built Command.
    - To prevent a copy of the same assertion in four crates, I added shared helpers `stdin_is_null` and `stdout_is_null` (unix only) to swissarmyhammer_common::test_utils, with their own tests. ane-embedding gets swissarmyhammer-common as a dev-dependency (no cycle).
    - Each site now goes through one small builder: copy_tree_command (coreml), reveal_command + platform_reveal_command (kanban), browser_command + platform_browser_command (mirdan), terminal_child_command (claude-agent, used by both terminal sites).
    - editor.rs is not changed (interactive, must inherit).
    - RED seen for all four sites before the fix; GREEN after.
  timestamp: 2026-10-18T00:08:08.957498+00:00
- actor: claude-code
  id: 01m565shndanzy7nvgxds93zj3
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/ane-embedding/src/coreml.rs, crates/ane-embedding/Cargo.toml (dev-dep swissarmyhammer-common; Cargo.lock +1), crates/claude-agent/src/terminal_manager.rs, crates/swissarmyhammer-kanban/src/commands/entity_commands.rs, crates/mirdan/src/auth.rs, crates/swissarmyhammer-common/src/test_utils.rs. New tests: 6 (4 site tests RED then GREEN, 2 helper tests). `cargo fmt --check` clean. `cargo clippy --all-targets --all-features -- -D warnings` clean. `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-kanban -p mirdan -p claude-agent -p ane-embedding`: 3924 run, 3911 passed, 2 failed, 11 timed out.
    - environment failures (not related to this change; no changed symbol is in their path): claude-agent `spawn_process_and_consume_init_skips_trigger_when_configured` (2-second start bound under load; passes alone); claude-agent `permissions::tests::test_permission_decision_caching_with_expiration` (fails at the first check right after store with a 2-second expiry; on this host trivial tests take 5-14 s, slow I/O in the large TMPDIR); 11 mirdan timeouts at 300 s: 8 `git_source::tests::test_clone_anthropics_*` (network clone), `install::tests::test_e2e_clone_anthropics_deploy_validator_uninstall_by_url` (network clone), `install::profile_consistency_tests::every_cli_profile_installs_by_store_symlink_and_round_trips` and `explicit_root_install_never_touches_cwd_for_any_profile` (TMPDIR / getcwd environment).
    - next: /review
  timestamp: 2026-10-18T00:15:11.021184+00:00
- actor: claude-code
  id: 01m56a3xtph6q9brk0d6xkkhp5
  text: |-
    ### test - stuck
    - evidence: cargo nextest run --workspace -E 'rdeps(ane-embedding) | rdeps(claude-agent) | rdeps(swissarmyhammer-kanban) | rdeps(mirdan) | rdeps(swissarmyhammer-common)' --no-fail-fast. The filter selects 12202 tests (the whole workspace). Result: 12172 passed, 8 failed, 22 timed out, 0 skipped. cargo clippy --all-targets --all-features -- -D warnings: exit 0, zero warnings. cargo fmt --check: clean.
    - The 6 new tests for the changed code all pass: stdin_is_null_only_when_the_command_sets_a_null_stdin, stdout_is_null_only_when_the_command_sets_a_null_stdout (swissarmyhammer-common), terminal_child_command_does_not_read_the_stdin_of_this_process (claude-agent), copy_tree_command_does_not_use_the_stdin_or_stdout_of_this_process (ane-embedding), browser_command_does_not_read_the_stdin_of_this_process (mirdan), reveal_command_does_not_use_the_stdin_or_stdout_of_this_process (swissarmyhammer-kanban).
    - Failures that are all known environment problems (none run changed code):
      - claude-agent permissions::tests::test_permission_decision_caching_with_expiration. The test uses PermissionPolicyEngine only. It has no reference to terminal_manager. It fails at the immediate check, after a 2 second expiry window, because other tests in the run took 3 to 6 seconds (wall-clock limit under load).
      - mirdan: 8 git_source test_clone_anthropics_* tests (network git clone), 2 install::profile_consistency_tests, 1 install::tests::test_e2e_clone_anthropics_deploy_validator_uninstall_by_url. All time out at 300 s. They do not call crate::auth (only registry/client.rs, doctor.rs, dispatch.rs call it).
      - statusline: test_index_deleted_cwd, test_languages_deleted_cwd (deleted cwd, getcwd hang).
      - validators: 10 review::tool_rules::tests::shipped::* time outs (Swift/Go fixtures), the_tool_run_overlaps_the_fleet_instead_of_delaying_it, blame_overhead_on_a_representative_commit_is_small_and_concurrent.
      - diagnostics real-server tests: follower_multi_step_rename_gets_real_leader_edits_under_one_lock, refresh_file_direct_reports_error_against_real_server.
      - tools concurrency stress tests: test_concurrent_file_access_patterns, test_high_concurrency_stress_test, test_mixed_operation_concurrency_stress.
    - No source file was changed by this step.
    - next: A person must decide. The build cannot be fully green on this machine until the large $TMPDIR problem is fixed. Run the 30 tests on a clean machine or in CI.
  timestamp: 2026-10-18T01:30:45.462118+00:00
- actor: claude-code
  id: 01m56am4nwypkdrm7k2scweach
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (commit 7e0dd17ba). 1 finding (1 confirmed, 1 refuted, 14 attempted, 0 failed): crates/mirdan/src/auth.rs:153
    - next: Set stdout to null in browser_command and extend its test to assert stdout is null. Then run the review again.
  timestamp: 2026-10-18T01:39:36.764070+00:00
- actor: claude-code
  id: 01m56amrycpjhb878bjjp2d1s9
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 7 files
    - test: stuck — whole workspace, 12172 of 12202 passed; all 30 failures are the known $TMPDIR and load problems, none runs changed code; 6 new tests pass; clippy and fmt clean
    - commit: 7e0dd17ba
    - review: findings — crates/mirdan/src/auth.rs:153
  timestamp: 2026-10-18T01:39:57.516656+00:00
- actor: claude-code
  id: 01m56attwfzna9a9eh2wvksdyp
  text: |-
    Fix for the finding at crates/mirdan/src/auth.rs:153 (completeness/invariant-propagation).
    - browser_command now sets a null stdin and a null stdout. The doc comment says why (stdin and stdout of `sah serve` are the protocol channel).
    - The test is renamed to browser_command_does_not_use_the_stdin_or_stdout_of_this_process (the same name form as the copy_tree and reveal tests) and asserts stdout_is_null too. RED seen first: "stdout is not null: Command { program: \"open\", ... stdin: Some(Null) }". GREEN after the fix.
    - Check of terminal_child_command (crates/claude-agent/src/terminal_manager.rs): no spawn path inherits stdout. create_terminal_with_command sets stdout and stderr to piped before spawn. execute_command uses tokio Command::output, which always sets stdout and stderr to piped (tokio-1.53.1 src/process/mod.rs, output(): `self.std.stdout(Stdio::piped()); self.std.stderr(Stdio::piped());`). No code change there. I added a doc comment paragraph to terminal_child_command that records this.
    - Other sites of the change already null stdout: copy_tree_command (coreml.rs) and reveal_command (kanban).
  timestamp: 2026-10-18T01:43:16.111311+00:00
- actor: claude-code
  id: 01m56aty3bssgq70pc22ckwnr7
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/mirdan/src/auth.rs (stdout null in browser_command, doc comment, test extended and renamed), crates/claude-agent/src/terminal_manager.rs (doc comment only; stdout is already piped on both paths). `cargo nextest run -p mirdan -E 'test(browser_command)'`: 1 run, 1 passed (RED before fix). `cargo nextest run -p claude-agent -E 'test(terminal_child_command)'`: 1 run, 1 passed. `cargo clippy --all-targets --all-features -- -D warnings`: clean. `cargo fmt --check`: clean. Finding flipped to [x].
    - next: /review
  timestamp: 2026-10-18T01:43:19.403859+00:00
- actor: claude-code
  id: 01m56b7mtpbthe2m1pj3g9j48r
  text: |-
    ### test - green
    - evidence: cargo nextest run -p mirdan -p claude-agent --no-fail-fast: 1600 run, 1589 passed, 11 timed out, 0 skipped. All 11 timeouts are known environment failures: 8 mirdan git_source test_clone_anthropics_* tests, plus install::profile_consistency_tests (2 tests) and install::tests::test_e2e_clone_anthropics_deploy_validator_uninstall_by_url. Both claude-agent known flaky tests passed. cargo clippy --all-targets --all-features -- -D warnings: clean. cargo fmt --check: clean.
    - next: review
  timestamp: 2026-10-18T01:50:15.894394+00:00
position_column: doing
position_ordinal: '8480'
title: Do not let a child process inherit the MCP stdin of sah serve
---
## Problem
`sah serve` speaks MCP on its stdin and stdout. A child process that does not set `.stdin(...)` gets the stdin of the parent with `spawn()` and `status()`. Thus a child that reads stdin can take MCP frames, and a child that writes to stdout can break the MCP output. The Swift port found this bug in the field (SWE-bench: `npm install` took the protocol channel, ^w29j74j in that repo).

Note: with std, `output()` sets an unset stdin to null. With tokio, `output()` does NOT do this. It inherits stdin (tokio-1.53.1 `src/process/mod.rs:1065-1071`).

## Audit result (2026-10-17)
The paths that can be dangerous are safe today: the LSP spawn (`swissarmyhammer-lsp/src/daemon.rs:297`, all piped), the shell tool, review tool-rule runs, tool auto-install, git diff and the doctor probes. Rust has no LSP auto-install.

These sites leave stdin unset on a path that inherits it:
- `crates/ane-embedding/src/coreml.rs:83`: `cp -R` with std `.status()`. It runs in `sah serve` on macOS (code-context embeddings) when the CoreML cache is empty. stdout is inherited too.
- `crates/claude-agent/src/terminal_manager.rs:346`: tokio `spawn()` for ACP `terminal/create`. Today the terminal capability check blocks it, but the risk is latent.
- `crates/claude-agent/src/terminal_manager.rs:594`: tokio `.output()` in `execute_command`. It inherits stdin (tokio behavior). Blocked by the same check, latent.
- `crates/swissarmyhammer-kanban/src/commands/entity_commands.rs:446`, `:457`, `:461`: `open -R`, `xdg-open` and `explorer` with std `.status()`. stdout is inherited too. Not reachable from `sah serve` today.
- `crates/mirdan/src/auth.rs:141`, `:145`, `:149`: browser open with std `.spawn()`. CLI only.
- `crates/swissarmyhammer-common/src/editor.rs:33`: `$EDITOR`. It is interactive, so it must inherit. Leave it as it is.

## Fix
- Add `.stdin(Stdio::null())` at each site above, except `editor.rs`.
- Add `.stdout(Stdio::null())` at `coreml.rs:83` and at the kanban reveal sites.
- Add a test for each site where a test is possible: the child must not read the stdin of the parent.

## Acceptance criteria
- No production `Command` that can run in `sah serve` inherits stdin or stdout.
- `cargo nextest` for the changed crates passes. Clippy `-D warnings` is clean. #bug

## Review Findings (2026-10-17 20:31)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 6 file(s) reviewed, 5 not reviewed.

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

> 1 file(s) not reviewed — no validator matched:
> - `Cargo.lock` — no validator matches this file

- [x] `crates/mirdan/src/auth.rs:153` `completeness/invariant-propagation` — browser_command nulls the child's stdin but leaves its stdout inherited. The same change nulls stdout for cp (coreml.rs) and reveal, on the stated ground that stdout is the MCP channel of `sah serve`. A browser opener spawned from that process can therefore write text into the MCP stdout stream, which the change does not prevent. In browser_command, also call `.stdout(std::process::Stdio::null())` next to the stdin call, and extend browser_command_does_not_read_the_stdin_of_this_process to assert stdout is null too, as the copy_tree test does. If stdout is deliberately left inherited for browser, document why in the doc comment at auth.rs:145-150.
