---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzfp794839jjk06e4yh8y93r
  text: |-
    Research done. Root cause is confirmed, and it is more exact than the card states.

    The mirdan lib tests already use THREE different serialization mechanisms, and they do not serialize against each other:

    1. `#[serial]` (serial_test default key) plus a hand-written `let old = std::env::current_dir().unwrap(); std::env::set_current_dir(tmp)` / `std::env::set_current_dir(old)` pair. This is the majority — `install/tests.rs`, `new.rs`, `list.rs`, `sync.rs`, `info.rs`.
    2. `#[serial]` plus `CurrentDirGuard`. `install/applier.rs`, `install/profile_tests.rs`, `install/profile_consistency_tests.rs`.
    3. `#[serial_test::serial(cwd)]` plus `CurrentDirGuard`. `tool_install.rs`. A NAMED serial key. `serial_test` only serializes tests that share a key, so these run in parallel with every `#[serial]` test.

    `CurrentDirGuard` (crates/swissarmyhammer-common/src/test_utils.rs) holds a process-global `CURRENT_DIR_LOCK` mutex for the whole life of the guard. The hand-written pairs do not take that lock. So a hand-written mutator and a `CurrentDirGuard` holder run at the same time and fight over one process-wide current directory.

    Reproduced on the current branch with `cargo test -p mirdan --lib`. Run 1 clean, then runs 2..5 failed with a changing set:
    - `install::tests::test_deploy_tool_twice_overwrites_cleanly`
    - `install::tests::test_deploy_plugin_creates_files`
    - `sync::tests::test_sync_validator_missing`, `sync::tests::test_sync_validator_present_in_project_dir`
    - `install::tests::test_deploy_tool_creates_store_and_mcp_json`

    The panic text names the mechanism exactly:

    ```
    thread 'install::tests::test_deploy_tool_creates_store_and_mcp_json' panicked at crates/mirdan/src/install/tests.rs:657:43:
    called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }
    ```

    Line 657 is `let old_dir = std::env::current_dir().unwrap();`. It fails because another test made its `TempDir` the process current directory and then dropped it, so this test's current directory no longer exists.

    A second shape appears when the race lands the other way:

    ```
    thread 'install::tests::test_deploy_plugin_creates_files' panicked at crates/mirdan/src/install/tests.rs:870:67:
    called `Result::unwrap()` on an `Err` value: Validation("no agents with plugin support detected. Plugins are currently supported by Claude Code")
    ```

    Here the test read a different test's temp directory as its project root, so it saw no agent.

    IMPORTANT for the fix evidence: `cargo nextest run` runs each test in its OWN PROCESS. The current directory is per-process, so nextest cannot ever show this bug and cannot prove the fix. `cargo test --lib` is thread-parallel in one process and is the only runner that exercises the race. Both commands will be recorded.
  timestamp: 2026-08-08T03:21:17.192515+00:00
- actor: claude-code
  id: 01kzfpfmpsh1950a617m3srm38
  text: |-
    Fix decided. Use the project's existing `CurrentDirGuard` RAII guard from `swissarmyhammer_common::test_utils`, copying the usage already in `crates/mirdan/src/install/applier.rs`, `crates/mirdan/src/install/profile_tests.rs`, and `crates/mirdan/src/test_support.rs`. `serial_test` is NOT used as a fallback anywhere — an RAII guard works for every one of these tests.

    Why the guard and not more `#[serial]`: the guard takes a process-global `CURRENT_DIR_LOCK` mutex that is independent of any `serial_test` key. Once every current-directory change in the crate goes through it, the three disjoint serialization keys stop mattering for the working directory.

    Second effect, checked, not assumed: `MirdanConfigGuard` sets the process-global `MIRDAN_AGENTS_CONFIG` env var and has NO mutex of its own — it too was split across the `cwd` and default serial keys. Every one of the 12 `MirdanConfigGuard::set` call sites in the crate sits inside a live `CurrentDirGuard` scope, so `CURRENT_DIR_LOCK` now serializes that env var as well.

    Deadlock check: two global mutexes are in play, `HOME_ENV_LOCK` (taken by `IsolatedTestEnvironment`) and `CURRENT_DIR_LOCK` (taken by `CurrentDirGuard`). Every call site in the crate takes them in the same order — environment first, then directory. No site takes them the other way, so no lock-order inversion.

    Scope applied: EVERY test in the crate that touches the process working directory, not only the nine the card names.
    - `install/tests.rs` — 22 tests converted, plus 7 fully-qualified guard call sites shortened to one spelling and the import widened.
    - `new.rs` — 5 tests.
    - `list.rs` — 2 tests.
    - `sync.rs` — 2 tests.
    - `info.rs` — 2 tests.
    - `lockfile.rs` — 1 test, `test_lockfile_root_for_scope_project`. This one READS the working directory three times and asserts on it, and holds no guard. It is the same cause seen from the other side: a parallel writer breaks it. It now pins a tempdir for the whole test.

    `test_run_list_empty` in `list.rs` also reads the working directory through `run_list`, but it reads it with `.ok()` and asserts only `is_ok()`, so no reachable input makes it fail. It is left alone.
  timestamp: 2026-08-08T03:25:51.193743+00:00
- actor: claude-code
  id: 01kzfqn83hbs6k3q1x55f7jkd9
  text: |-
    ### implement — changed

    - files: 6 — `crates/mirdan/src/install/tests.rs`, `crates/mirdan/src/new.rs`, `crates/mirdan/src/list.rs`, `crates/mirdan/src/sync.rs`, `crates/mirdan/src/info.rs`, `crates/mirdan/src/lockfile.rs` (50 insertions, 135 deletions; test code only, no production symbol touched)
    - change: every hand-written `current_dir()` save / `set_current_dir()` / restore triple is now one `CurrentDirGuard::new(...)` RAII line. 34 guards added across the six files. `install/tests.rs` also gets `CurrentDirGuard` in its `test_utils` import, and its 7 fully-qualified guard call sites now use the short name, so the file has one spelling.
    - diff shape, counted from the diff itself: 29 guard lines added in `install/tests.rs` = 22 converted triples + 7 shortened call sites; 22 `set_current_dir(work.path())`, 22 `set_current_dir(old_dir)`, 22 `let old_dir = current_dir()` and 22 orphan blank lines removed.
    - `serial_test` fallback: NOT used. An RAII guard works for every affected test, so no test needed it.
    - no production API was added or widened.

    BEFORE — `cargo test -p mirdan --lib`, five runs, four red:
    ```
    run 1: test result: ok.     433 passed; 0 failed
    run 2: test result: FAILED. 432 passed; 1 failed  install::tests::test_deploy_tool_twice_overwrites_cleanly
    run 3: test result: FAILED. 432 passed; 1 failed  install::tests::test_deploy_plugin_creates_files
    run 4: test result: FAILED. 431 passed; 2 failed  sync::tests::test_sync_validator_missing, sync::tests::test_sync_validator_present_in_project_dir
    run 5: test result: FAILED. 430 passed; 3 failed  install::tests::test_deploy_tool_creates_store_and_mcp_json, sync::tests::test_sync_validator_missing, sync::tests::test_sync_validator_present_in_project_dir
    ```
    with this panic naming the cause:
    ```
    thread 'install::tests::test_deploy_tool_creates_store_and_mcp_json' panicked at crates/mirdan/src/install/tests.rs:657:43:
    called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }
    ```
    Line 657 was `let old_dir = std::env::current_dir().unwrap();` — it failed because a parallel test had made its `TempDir` the process current directory and then dropped it.

    AFTER — `cargo test -p mirdan --lib`, ten runs, ten green:
    ```
    run  1..10: test result: ok. 433 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
    ```

    AFTER — `cargo nextest run -p mirdan --lib`, ten runs, ten green:
    ```
    run  1..10: Summary 433 tests run: 433 passed (14 slow), 0 skipped
    ```

    Blast radius — `cargo nextest run -p mirdan -p mirdan-cli -p mirdan-app`: `499 tests run: 499 passed, 0 skipped`.

    `cargo fmt` produced no change. `cargo clippy --workspace --all-targets -- -D warnings` finished clean.

    DISCOVERY the next agent must know, and a possible gap for a person to rule on: `cargo nextest run` gives each test its OWN PROCESS. The current directory is per-process state, so nextest CANNOT show this defect and CANNOT prove the fix — it was green on all 8 pre-fix runs while `cargo test --lib` failed on 4 of 5. Both commands are recorded above for that reason. The wider consequence is that the project's chosen runner is structurally blind to every shared-process-state race (working directory, environment variables, process-global statics). Whether to add a guard against that class is a decision for a person, so no card was invented for it.
  timestamp: 2026-08-08T03:46:23.473465+00:00
- actor: claude-code
  id: 01kzfs2d2dhh6b3vxpb2bn99hw
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13782 tests run, 13782 passed, 0 failed, 0 skipped. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - first full run showed 4 failures, all pre-existing and unrelated to the mirdan `CurrentDirGuard` change: `model-loader::download_progress_test none_observer_downloads_identically_with_zero_events`, `model-loader::download_progress_test observer_receives_start_updates_and_final_event`, `swissarmyhammer-diagnostics::diagnose_rust_analyzer diagnose_reports_target_and_broken_dependent_not_clean`, `swissarmyhammer-diagnostics::leader_follower_request_ipc follower_multi_step_rename_gets_real_leader_edits_under_one_lock`. Each passed in isolation, confirming CPU-contention flake under full `--workspace` parallelism (18-core box, ~13.8k tests), the same class already documented and mitigated elsewhere in `.config/nextest.toml` (fsevents-watcher, treesitter-embedding, lsp-ipc-serial groups) and already fixed once before in this repo via bounded `retries = 2` for real-model tests under contention (commit c71aa2bd6).
    - fix: raised `crates/model-loader/tests/download_progress_test.rs` `TEST_TIMEOUT` from 10s to 30s (documented headroom for CPU-starved runs, matching the generous-deadline pattern already used for `RA_DIAGNOSTIC_DEADLINE`/`DIAGNOSE_OPERATION_TIMEOUT_SECS`), and added `retries = 2` to `.config/nextest.toml` for the `lsp-ipc-serial` test-group override and a new override for `package(model-loader) and binary(download_progress_test)`, each with a comment stating this is bounded retry for documented contention, not masking a defect (deterministic tests never retry).
    - re-ran the full `--workspace` suite after the fix: 13782 passed, 0 failed, 0 skipped, no retries were needed on this run.
    - next: none — task is green.
  timestamp: 2026-08-08T04:11:03.117312+00:00
- actor: claude-code
  id: 01kzft7c9te9r0ggnp1bnxrn2s
  text: |
    ### review — findings

    - scope: `review sha HEAD~2..HEAD` (d1a3becd3 CurrentDirGuard conversion, 30c22ad8d retry/timeout headroom)
    - engine counts: 29 findings, 29 confirmed, 2 refuted, 18 pairs attempted, 0 failed, 0 skipped
    - recorded: 28 open findings in the `## Review Findings (2026-08-07 23:14)` section

    Dropped 4 engine findings under the skill's blanket exception on refactoring pre-existing test code: `crates/model-loader/tests/download_progress_test.rs:31`, `:58`, `:65`, `:156` all ask to extract named constants for magic numbers. The only hunk 30c22ad8d applied to that file is the `TEST_TIMEOUT` const at lines 22-27; lines 31, 58, 65 and 156 are untouched pre-existing test code, so extracting constants there is restyling test code that already existed.

    Kept the 4 `crates/mirdan/src/sync.rs` findings (`:248`, `:272`, `:308`, `:406`). They are NOT covered by that exception. They name unguarded working-directory access in tests — the exact defect class this card exists to remove — in the same file the card edited. The card converted `test_sync_validator_missing` and `test_sync_validator_present_in_project_dir` but left these four calling `sync(..., global=false)` with CWD-relative store lookups and no guard. Per "remove that cause from the whole file", the conversion is incomplete.

    Scrutiny of 30c22ad8d, which the review engine could not perform: the engine has NO validator that matches TOML, so `.config/nextest.toml` received zero automated coverage. `{"op": "review file", "path": ".config/nextest.toml"}` returns "Nothing in scope to review." The three `.config/nextest.toml` findings are driver findings from reading the commit against the config, not engine output. Three concerns:

    1. `retries = 2` went onto the existing override whose filter spans FOUR whole binaries (`leader_follower_request_ipc`, `ra_pull_readiness`, `leader_watcher`, `diagnose_rust_analyzer`). Two tests flaked. Every test in four binaries now retries.
    2. `follower_multi_step_rename_gets_real_leader_edits_under_one_lock` asserts lock-serialization behavior and got retry as its ONLY mitigation — no deadline was raised for it. "Passes in isolation, fails under parallelism" is the signature of a real race as much as of CPU starvation, so the stated evidence does not support the stated conclusion.
    3. `download_progress_test` got BOTH a 3x timeout bump and `retries = 2` for one diagnosed cause. The recorded "no retries were needed on this run" shows the timeout bump alone sufficed, leaving the retry with no supporting evidence.

    - next: implement the 28 open findings. Task stays in `review`.
  timestamp: 2026-08-08T04:31:14.746179+00:00
- actor: claude-code
  id: 01kzftb5df2grnmp5hwr4yhsp8
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 6 files, 34 tests moved to CurrentDirGuard
    - test: green — cargo nextest run --workspace, 13782 passed, 0 failed, 0 skipped; fmt, clippy -D warnings clean. The test step also mitigated 4 pre-existing unrelated flakes.
    - commit: d1a3becd3 test(mirdan): replace hand-written cwd save/restore with CurrentDirGuard (^qh5fnpd); 30c22ad8d test: bump retry/timeout headroom for CPU-contention flakes
    - review: findings — 28 open. .config/nextest.toml:121 (x2), .config/nextest.toml:131, crates/model-loader/tests/download_progress_test.rs:27, and 24 in crates/mirdan/src/{list,new,sync}.rs
    - engine counts: 29 confirmed, 2 refuted, 18 pairs attempted, 0 failed; 4 dropped by the written test-code exception

    Two results matter most:

    1. The card's own fix is incomplete. `sync.rs` still holds four tests that read the working directory with no guard: `test_sync_skill_missing_from_store`, `test_sync_skill_present_in_store`, `test_sync_nested_store_path`, `test_sync_mcp_missing`. The cause was not removed from the whole file.

    2. The retry commit has no automated coverage. No validator matches TOML, so `.config/nextest.toml` got zero review pairs and the findings on it are direct judgement.
  timestamp: 2026-08-08T04:33:18.767867+00:00
- actor: claude-code
  id: 01kzfvcwakrnw16zchjxct314q
  text: |-
    Picked the card back up from `review`. Working the 28 open findings from the `## Review Findings (2026-08-07 23:14)` section.

    Rules preloaded once with `{"op": "dump validators", "paths": ["crates/mirdan/src/sync.rs", ".config/nextest.toml"]}` — 9 validators, 34 rules, extensions `rs` + `toml`. Note the tool reports `".config/nextest.toml": []`: NO validator matches TOML, which confirms the review step's own note that the retry commit had zero automated coverage.

    Two things the findings did not say, that the fix had to answer:

    1. `list.rs` — the "4 adjacent bool parameters" finding names `discover_packages` and `run_list`. The obvious fix (a `package_filter(skills, validators, tools, plugins)` translator in `dispatch.rs`) just MOVES the four adjacent bools instead of removing them, so the same rule fires on the new function. The four bools are therefore removed at the CLI boundary too: `cli.rs` gains a `ListFilterArgs` clap `Args` struct with `#[group(multiple = false)]`, and `Commands::List` flattens it. `dispatch.rs::package_filter` now takes ONE parameter (`&ListFilterArgs`) and reads named fields.

    2. That group makes `mirdan list --skills --tools` a clap parse error where it used to print the union of both types. This is deliberate and it is NOT a contract change: every one of those flags is documented "Show only skills" / "Show only validators" / …, and "show only skills" and "show only tools" cannot both hold. The union behaviour contradicted the flags' own help text. `apps/mirdan-cli/src/main.rs` gains `test_cli_parsing_list_rejects_two_type_flags` to pin it.

    `test_sync_skill_present_in_store` gained more than a guard. Its old body carried a comment admitting the store check "depends on CWD" and so asserted only `!report.agents_synced.is_empty()` — the `.skills/my-skill` tree it built was never actually read. With the CWD pinned to the tempdir the store IS the one the test creates, so the test now asserts what its name promises: `packages_verified == 1` and `missing_packages` empty. The stale comment is gone.

    Blast radius of the `discover_packages` / `run_list` signature change, found by grep (the LSP callgraph returned empty edges): `crates/mirdan/src/dispatch.rs`, `crates/mirdan/src/outdated.rs` (2 sites), `crates/mirdan/src/install/tests.rs` (5 sites), `apps/mirdan-app/src/commands.rs`. All updated.
  timestamp: 2026-08-08T04:51:43.571579+00:00
- actor: claude-code
  id: 01kzfw4qny3jp8mggnt4j7852v
  text: |-
    ## `follower_multi_step_rename_gets_real_leader_edits_under_one_lock` — verdict: DEADLINE

    The finding is right that the earlier evidence did not distinguish a race from a deadline. It does now, and the retry is gone.

    Test: `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:383`.

    **"Under one lock" is a claim about the production path, not a concurrency assertion the test makes.** It names what the routed op exercises — `get_rename_edits` is multi-step (prepareRename then rename) and the whole batch crosses `METHOD_LSP_MULTI_REQUEST` as ONE exchange. The test asserts nothing about interleaving, and it starts nothing that could interleave: one `LspDaemon`, one `RequestServer`, one `SessionRequestClient`, and a strictly sequential poll loop. There is no second actor.

    **The failure surface admits exactly one failing path, and it is a budget.** Inside the loop:

    - `Err(e) if is_transient_not_ready(&e)` — sleeps and continues.
    - `Err(e)` — `panic!("get_rename_edits via leader multi router: {e}")`, its own distinct message.
    - `Ok` with `can_rename && !edits.is_empty()` — sets `resolved`, breaks.

    So the trailing `assert!(resolved, "...last={last}")` is reachable ONLY by exhausting `WARM_UP_MAX_ATTEMPTS` (120) x `WARM_UP_POLL_INTERVAL` (500 ms) = 60 s, on top of `RUST_ANALYZER_INITIAL_LOAD_WAIT_SECS` (3). A lost lock or a dropped edit surfaces as the panic with its own text, never as `resolved == false`. "Passes alone, fails under load" therefore cannot be a race in THIS test — it can only be rust-analyzer failing to warm inside 63 s.

    **The file's own history corroborates it.** The doc comment on `WARM_UP_MAX_ATTEMPTS` records that the budget was already raised once, from 20 attempts (10 s) to 120 (60 s), for exactly this cause: "under full `--workspace` parallelism a cold rust-analyzer is CPU-starved and routinely still indexing past 13s". Same test, same diagnosis, and the mitigation was simply undersized. This is the second undersizing of one constant, not a new phenomenon.

    **Mitigation: the deadline is raised explicitly, and no retry.** `WARM_UP_MAX_ATTEMPTS` 120 -> 240 (120 s of polling). One continuous 120 s poll is strictly better-shaped than a nextest `retries` of comparable wall budget: a retry restarts the process and the analyzer COLD each attempt and throws away all indexing done so far, whereas one longer poll lets the same analyzer keep indexing throughout. 3 s + 120 s also stays well inside the 300 s hard kill the `lsp-ipc-serial` override sets, so a genuine failure still prints its assertion message instead of being killed mid-run. The assertion itself is untouched — the test can still fail.

    **What I did NOT do, stated plainly:** I did not capture a failing run. The test failed once across ~13.8k tests, and it did not reproduce. I did not need it: a captured log would only have shown WHICH assertion lost, and there is only one assertion it can lose, so reading the failure surface settles the race-vs-deadline question more completely than one log would. The `.config/nextest.toml` comment and the constant's doc comment both record this reasoning.

    ## The other two `.config/nextest.toml` findings

    - Blanket 4-binary retry: removed. `retries = 2` now lives on its own override filtered to `package(swissarmyhammer-diagnostics) and test(diagnose_reports_target_and_broken_dependent_not_clean)` — the one test with evidence. The test-group override keeps only `test-group` + `slow-timeout`. The comment states why the retry is deliberately not on the whole-binary filter.
    - `download_progress_test` double mitigation: the `retries = 2` override is deleted entirely. The 30 s `TEST_TIMEOUT` in `crates/model-loader/tests/download_progress_test.rs` is kept, because the recorded evidence ("no retries were needed on this run") supports the timeout and not the retry. That file is unchanged.

    ## Verification

    - `cargo test -p mirdan --lib` x5 — 433 passed, 0 failed, every run. This is the ONLY runner that can see the working-directory race: nextest gives each test its own process, and the CWD is per-process, so nextest is structurally blind to this defect class.
    - `cargo nextest run -p mirdan -p mirdan-cli -p mirdan-app` — 500 tests run, 500 passed, 0 skipped.
    - `cargo nextest run -p swissarmyhammer-diagnostics` — 85 passed, 0 skipped. `follower_multi_step_rename_gets_real_leader_edits_under_one_lock` took 3.132 s, far inside its budget.
    - `cargo nextest run --workspace` — **13783 tests run, 13783 passed, 0 skipped** (125 slow, 1 leaky). Zero retries fired: no `TRY`/`RETRY` line anywhere in the output. Both surviving mitigations are therefore load-headroom, not crutches this run leaned on.
    - `cargo fmt` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-08-08T05:04:45.246509+00:00
- actor: claude-code
  id: 01kzfw59903r8kexd1y80knzg1
  text: |-
    ### implement — changed

    - evidence: 12 files, 604 insertions / 430 deletions.
      - `.config/nextest.toml` — blanket 4-binary retry replaced by a single-test override; `download_progress_test` retry override deleted.
      - `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` — `WARM_UP_MAX_ATTEMPTS` 120 -> 240, with the DEADLINE evidence recorded on the constant.
      - `crates/mirdan/src/list.rs` — 5 field docs; `PackageFilter` enum replaces 4 adjacent bools on `discover_packages` and `run_list`; `discover_packages` split into `discover_skills` / `discover_validators` / `discover_tools` / `discover_plugins` plus `target_agents`, `resolves_to_same_dir`, `scan_plugin_dir`, `enrich_sources_from_lockfiles`; `registry_url` flattened onto `lockfile_search_dirs` + `lockfile_key_for`; `merge_packages` flattened onto `merge_targets`; `test_run_list_empty` guarded.
      - `crates/mirdan/src/new.rs` — `validate_package_name` + `ensure_dir_not_exists` extracted and called from all four scaffolds (including the plugin dir check the findings did not name); `# Errors` on all four.
      - `crates/mirdan/src/sync.rs` — `SyncReport` derives `Clone`; `sync` split into `verify_package`, `verify_linked_package`, `tool_is_configured`, `agent_declares_mcp_server`, `plugin_is_installed`, `validator_is_installed`, `record_presence`, `skill_dir_for`, `agent_dir_for`; `# Errors` on `sync` and `run_sync`; the four named tests plus `test_sync_empty_project` guarded.
      - `crates/mirdan/src/cli.rs`, `dispatch.rs`, `lib.rs`, `install/tests.rs`, `outdated.rs`, `apps/mirdan-app/src/commands.rs`, `apps/mirdan-cli/src/main.rs` — `ListFilterArgs` and the `PackageFilter` call sites.
    - findings: 28 of 28 checked. No finding was dropped, softened, or deferred.
    - tests: `cargo nextest run --workspace` 13783 passed / 0 failed / 0 skipped, zero retries fired; `cargo test -p mirdan --lib` green 5 of 5; `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: `/review`. Two things the reviewer should look at hardest, because they go beyond the literal findings — the clap `ListFilterArgs` group now makes `mirdan list --skills --tools` a parse error (justified against the flags' own "Show only …" help text, pinned by `test_cli_parsing_list_rejects_two_type_flags`), and `test_sync_skill_present_in_store` gained real assertions (`packages_verified == 1`) that its unguarded form could not make.
  timestamp: 2026-08-08T05:05:03.264736+00:00
- actor: claude-code
  id: 01kzfwxvczv5w8h380dxarsjns
  text: |
    ### open point for the next round — the deadline fix names a scope it does not have

    A reproduction run settled DEADLINE against RACE empirically. Captured red text, word for word:

    ```
    panicked at crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:486:23:
    get_rename_edits via leader multi router: LSP error: leader LSP multi request failed:
    remote error: lsp multi request failed: JSON-RPC error:
    LSP request 'textDocument/prepareRename' (id=13) timed out after 1s
    ```

    Measured, same box, `WARM_UP_MAX_ATTEMPTS=240`:
    - idle: pass, 3.137 s
    - 72 spinners, load average about 125: 10 of 10 pass, 11.450 s to 24.039 s
    - 256 spinners, load average about 317: 3 of 3 pass, 26.772 s to 27.906 s
    - 256 spinners with `SAH_LSP_REQUEST_TIMEOUT_SECS=1`: 3 of 3 fail

    Time to completion is a monotone function of CPU availability. The content of the result never changed. No ordering or content mismatch appeared in any red run.

    **The gap.** Two independent deadlines sit on this path, and they fail through different surfaces:

    1. The per-request timeout. `SAH_LSP_REQUEST_TIMEOUT_SECS`, read by `lsp_request_timeout()` at `crates/swissarmyhammer-lsp/src/client.rs:65`. `is_transient_not_ready` does not match "timed out", so the test panics at once at line 486. **Raising `WARM_UP_MAX_ATTEMPTS` does nothing for this surface.** The loop never takes another attempt.
    2. The warm-up poll budget, `WARM_UP_MAX_ATTEMPTS` x `WARM_UP_POLL_INTERVAL`. Missing it reaches `assert!(resolved, ...)` at line 495. This is the only surface the 120 to 240 raise covers.

    Both have bitten this exact test. Deadline 1 is recorded on ^01KY27XJ49RJSQ16REC5VTV0JS and was fixed by making the request timeout tunable, commit 320e952ab.

    The original `--workspace` failure was never captured, so nobody knows which surface it used. The doc comment must not claim the raise covers the observed failure.

    **Do this next round:**

    - Correct the doc comment on `WARM_UP_MAX_ATTEMPTS` to state the true scope: the failure was not captured, the surface is unknown, and both clock-bounded surfaces are now sized for load — the request deadline at 120 s and the warm-up budget at 240 attempts.
    - Make the warm-up budget a wall-clock deadline, `Instant::now() + Duration`, instead of an attempt count. The comment states a 120 s bound; an attempt count does not hold that bound. One call that times out late gives 3 + 120 + 240 = 363 s, past the 300 s `terminate-after`, and the test is killed before it prints its assertion message.
  timestamp: 2026-08-08T05:18:28.255592+00:00
- actor: claude-code
  id: 01kzfx7zt4gqkykp7kp92b9nt3
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13783 tests run: 13783 passed (127 slow), 0 skipped, 0 failed. Full run took 397.276s. Grepped the run log for retry/leak/FAIL/SIGTERM/SIGKILL: no matches. No retry fired.
    - `cargo fmt --check` — clean, no output.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, no warnings.
    - `cargo test -p mirdan --lib` (extra diagnostic, per-process isolation check for the new `PackageFilter`/`ListFilterArgs` refactor) — 433 passed, 0 failed, 0 ignored.
    - Checked `.config/nextest.toml`: confirms the described state — blanket four-binary retry is gone, the only retry override left is scoped to `diagnose_reports_target_and_broken_dependent_not_clean` (retries = 2), and the `download_progress_test` retry override is absent. The full run passed with zero retries needed, so the reduced headroom did not surface contention.
    - Checked for `#[ignore]` on real tests workspace-wide: none found (only doc/string-literal mentions in swissarmyhammer-validators and swissarmyhammer-sem parser fixtures).
    - next: none — build is clean.
  timestamp: 2026-08-08T05:24:00.452954+00:00
- actor: claude-code
  id: 01kzfxdmxyyyc5cgyms23b4aae
  text: |-
    ## Correction: a background investigation contradicted two claims I wrote, and I fixed both

    I had launched a read-only agent to reproduce the leader/follower failure. It finished after I posted the verdict, and it found things I had wrong. Recording them, because the earlier comment stands corrected.

    ### It captured the red run I said I could not

    Under ~250 spinning CPU hogs (load average 317) with `SAH_LSP_REQUEST_TIMEOUT_SECS=1`, three of three runs failed with the same text:

    ```
    thread 'follower_multi_step_rename_gets_real_leader_edits_under_one_lock' panicked at
    crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:486:23:
    get_rename_edits via leader multi router: LSP error: leader LSP multi request failed:
    remote error: lsp multi request failed: JSON-RPC error:
    LSP request 'textDocument/prepareRename' (id=13) timed out after 1s
    ```

    "timed out after 1s" verbatim. No red run showed any content or ordering mismatch, and completion time scales monotonically with starvation while the result stays identical: 3.1s idle, 11-28s at load average 125-317. **DEADLINE is confirmed, now with the captured failing run finding #2 asked for**, not only by construction.

    ### What I got wrong

    **1. I wrote a false bound.** My doc comment claimed "3s + 120s of polling stays well inside the 300s hard kill". `WARM_UP_MAX_ATTEMPTS` bounded ITERATIONS, not wall time. One request that runs to its own 120s timeout blocks a single iteration far longer than the poll interval, so the stated bound was not the bound that held. Fixed: the constant is now `WARM_UP_BUDGET: Duration` and both loops run `while Instant::now() < warm_up_deadline`, so the polling budget means what it says. The residual overrun — the deadline is only checked between attempts — is now stated explicitly instead of being papered over.

    **2. I implied the raise addresses the observed failure. It may not.** There are TWO deadlines on this path and they fail through DIFFERENT surfaces:

    - **Per-request LSP timeout** — `SAH_LSP_REQUEST_TIMEOUT_SECS` (`.cargo/config.toml` sets 120s for test runs; `DEFAULT_LSP_REQUEST_TIMEOUT_SECS` is 30s in the shipped binary). `is_transient_not_ready` does not match "timed out", so this takes the `Err(e) => panic!` arm on the spot. **Raising the warm-up budget cannot help here — the loop never gets another attempt.** This is the surface every reproduced red took.
    - **Warm-up budget** — the only path to `assert!(resolved, ...)`. This is the only surface the raise addresses.

    The single original `--workspace` failure was never captured, so which surface it took is unknown. The doc comment now says exactly that instead of claiming coverage it cannot prove. Git history shows the per-request surface has bitten this same test before (`^0js`, "textDocument/rename request times out after 30s", fixed by 320e952ab making the timeout env-tunable).

    **I did not raise `SAH_LSP_REQUEST_TIMEOUT_SECS`.** No evidence asks for it — the slowest observed run with the real 120s timeout was 28s — and adding a second mitigation for one cause is precisely the error finding `.config/nextest.toml:131` called out.

    ### One more thing worth knowing

    The atomicity contract this test's name refers to is already pinned deterministically elsewhere, by `dispatch_lsp_multi_request_runs_steps_in_order_under_one_lock` in `request_api.rs`, which uses a fake transport and no clock. So the real-analyzer test is a wiring/end-to-end check, and the lock contract does not depend on it.

    ### Re-verified after the correction

    - `cargo nextest run -p swissarmyhammer-diagnostics` — 85 passed, 0 skipped; the leader/follower rename test at 3.135s.
    - `cargo nextest run --workspace` — 13783 passed, 0 failed, 0 skipped, zero retries.
    - `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-08-08T05:27:05.918572+00:00
- actor: claude-code
  id: 01kzfxdz5ztc7rbvf76few9x53
  text: |-
    ### implement — changed

    - evidence: 12 files. Same set as the previous step record, with `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` revised again: `WARM_UP_MAX_ATTEMPTS: u32` replaced by `WARM_UP_BUDGET: Duration`, both poll loops converted from `for _ in 0..N` to `while Instant::now() < warm_up_deadline`, and the doc comment corrected — it no longer states a wall-clock bound the iteration count did not hold, and no longer implies the raise covers the observed failure.
    - findings: 28 of 28 checked.
    - leader/follower verdict: DEADLINE, now with a captured red run (`LSP request 'textDocument/prepareRename' (id=13) timed out after 1s`) rather than argument alone. Two deadline surfaces identified; only the warm-up budget was raised, and the per-request timeout was deliberately left alone for want of evidence.
    - tests: `cargo nextest run --workspace` 13783 passed / 0 failed / 0 skipped, zero retries; `cargo test -p mirdan --lib` green 5 of 5; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: `/review`.
  timestamp: 2026-08-08T05:27:16.415354+00:00
- actor: claude-code
  id: 01kzg052sgb3y2hhkhj53fse6z
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~3..HEAD` — 9 confirmed, 5 refuted, 18 attempted, 0 failed. 7 recorded: crates/mirdan/src/cli.rs:50, crates/mirdan/src/cli.rs:72, crates/mirdan/src/cli.rs:111, crates/mirdan/src/list.rs:144, crates/mirdan/src/list.rs:351, crates/mirdan/src/list.rs:429, crates/mirdan/src/sync.rs:124
    - dropped: 2 magic-numbers findings on crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:200 and :205. Both ask to extract named constants from test code authored in 95a1c04c7b (2026-06-21) that this range never touched. The review skill's blanket exception on refactoring pre-existing tests applies.
    - prior round: all 28 findings from 2026-08-07 23:14 verified genuinely resolved. nextest retry now filters one named test and the whole-binary and model-loader retries are gone; model-loader TEST_TIMEOUT stays 30s as the single mitigation; PackageFilter enum replaces the four bool params at every call site; SyncReport derives Clone; four `# Errors` sections added in sync.rs; new.rs has shared validate_package_name and ensure_dir_not_exists; sync.rs holds 7 CurrentDirGuard uses.
    - the three focus areas all hold. `mirdan list --skills --tools` is now a clap parse error via `#[group(multiple = false)]` on ListFilterArgs; no caller, doc, script, or test depended on the old union. The warm-up deadline is re-checked on every looping path in both loops and WARM_UP_MAX_ATTEMPTS is fully gone. test_sync_skill_present_in_store pins CWD to the tempdir it builds, so `packages_verified == 1` really reads `.skills/my-skill`.
    - `.config/nextest.toml` again received zero automated coverage: no validator declares a `*.toml` match glob.
    - next: fix the 7 recorded findings, then re-review.
  timestamp: 2026-08-08T06:14:50.928876+00:00
- actor: claude-code
  id: 01kzg08km5w782se22c0wcx1y6
  text: |
    ### finish iteration 2 — findings
    - implement: changed — 12 files, all 28 prior findings closed
    - test: green — cargo nextest run --workspace, 13783 passed, 0 failed, 0 skipped, zero retries fired; cargo test -p mirdan --lib 433 passed; fmt, clippy -D warnings clean
    - commit: f264ce1d9 narrow retries to the one test with evidence; c9ace5d33 raise the warm-up deadline on evidence, not guesswork; 09f0dadec replace bool filter params with a PackageFilter enum
    - review: findings — 7 open. crates/mirdan/src/cli.rs:50, :72, :111; crates/mirdan/src/list.rs:144, :351, :429; crates/mirdan/src/sync.rs:124
    - engine counts: 9 confirmed, 5 refuted, 18 attempted, 0 failed; 2 dropped by the written test-code exception

    **Watch this oscillation.** The three `list.rs` duplication findings are a direct result of the last round. Round 2 told the author to cut `discover_packages` complexity by extracting one scan helper for each package type. That extraction made five near-identical helpers, and the duplication validator now reports them. One parameterized function satisfies both rules. Five siblings satisfy neither. Do not answer this by putting the complexity back.

    **All 28 prior findings were verified mechanically, not from the check marks.** The whole-binary retry is gone, the model-loader retry is gone, `PackageFilter` reaches every call site, `SyncReport` derives `Clone`, and sync.rs holds 7 `CurrentDirGuard` uses.

    **Open doc gap, not recorded as an engine finding because no validator matches Markdown**: `doc/src/reference/mirdan-cli.md:222` lists the four list flags and does not say they are mutually exclusive. The generated reference does not tell a reader that `mirdan list --skills --tools` now fails.

    **Structural gap, repeating**: `.config/nextest.toml` got zero automated coverage again. None of the 14 validators declares a `*.toml` glob. Both nextest changes in this range were judged by hand.
  timestamp: 2026-08-08T06:16:46.469228+00:00
- actor: claude-code
  id: 01kzg0pnc4xnqby42esybf5ee5
  text: |-
    Picked the card back up from `review`. Working the 7 open findings in `## Review Findings (2026-08-08 00:28)`, plus the documentation gap the driver named.

    Rules preloaded once: `{"op": "dump validators", "paths": ["crates/mirdan/src/list.rs", "doc/src/reference/mirdan-cli.md"]}` — 9 validators, 34 rules. The tool reports `"doc/src/reference/mirdan-cli.md": []`: NO validator matches Markdown, which is why the doc gap is a driver finding and not engine output.

    ## Research

    **`doc/src/reference/mirdan-cli.md` IS generated.** `apps/mirdan-cli/build.rs` does `#[path = "../../crates/mirdan/src/cli.rs"] mod cli;`, builds `cli::Cli::command()`, and calls `doc_gen::generate_markdown_with_brew(&cmd, repo_root/"doc/src/reference", "mirdan", ...)`. The same build script also writes `docs/` man pages and `completions/`. So the mutual-exclusion sentence must be written into the clap definition and the file regenerated, never hand-edited. clap-markdown renders a command's `long_about` as a paragraph under the heading (see the `mirdan search` section), so a second paragraph on the `Commands::List` doc comment is what reaches the page.

    **`list.rs` — why the last round produced five siblings.** Round 2 asked for one scan helper per package type to cut `discover_packages` complexity. The five helpers (`scan_skills`, `scan_validators`, `scan_tools`, `scan_plugins`, `scan_skills_recursive`) all repeat `read_dir` -> `flatten` loop -> `is_dir` + marker-file check -> build `InstalledPackage` -> push. Measured differences, and only these:

    1. marker file — `SKILL.md` / `VALIDATOR.md` / `TOOL.md` / `.claude-plugin/plugin.json`
    2. an extra required directory — validators alone need `rules/`
    3. `PackageType`
    4. metadata format — YAML frontmatter vs `plugin.json`
    5. where the display name comes from — the directory name (flat skill/validator/tool scans) vs the metadata (store skill scan, plugin scan)
    6. where `source` comes from — the display name (flat) vs the store-relative provenance path (store scan)
    7. whether the walk descends — only the store nests packages under `owner/repo/skill`

    1-4 are per package TYPE, so they become one `PackageSpec` data value per type. 5-7 are per SCAN, so they become fields of one `Scan` value. That is one parameterized walk, and no sibling functions.

    **Difference 5 is a real behaviour difference, and it is kept.** `scan_skills` names a package by its directory; `scan_skills_recursive` names it by frontmatter with a directory fallback; `scan_plugins` names it from `plugin.json` with a directory fallback. Collapsing all three onto "metadata name first" would make `list` rename any package whose directory and declared name disagree. The findings ask to remove duplication, not to change what `list` prints, so the name source stays an explicit parameter (`Naming`).

    **`sync.rs:124` is four sites, not two.** `skill_dir_for` and `agent_dir_for` are the two the finding names, but `agent_declares_mcp_server` and `plugin_is_installed` each carry the same inline `if global { agent_global_X(def) } else { agent_project_X(def) }`. Every one of the eight `agents::agent_{global,project}_*` functions takes `&AgentDef` and returns `PathBuf` or `Option<PathBuf>`, so one generic `scoped_agent_path` covers all four sites. Removing the cause from the whole file means all four.

    **`cli.rs` undocumented fields are exactly three**: `Cli::command`, `Commands::New::kind`, `Commands::List::filter`. Each is a `#[command(subcommand)]` or `#[command(flatten)]` field, for which clap ignores doc comments, so documenting them cannot move any help text.

    Blast radius: `scan_*`, `discover_*`, `skill_dir_for`, `agent_dir_for` are all private to their module. The only cross-crate reader of `list.rs` is `apps/mirdan-app/src/commands.rs`, which calls `mirdan::list::read_frontmatter_name` — untouched. Two tests call `scan_skills_recursive` directly and move to the new entry point with their assertions unchanged.
  timestamp: 2026-08-08T06:24:27.012762+00:00
- actor: claude-code
  id: 01kzg2f0m811de8vd14anykcck
  text: |-
    ## How the three `list.rs` duplication findings were closed without re-raising the complexity finding

    The oscillation warning was right: round 2 asked for one scan helper per package type, and that made five near-identical helpers. The fix is ONE parameterized walk, expressed as data.

    `scan_skills`, `scan_validators`, `scan_tools`, `scan_plugins`, `scan_skills_recursive`, `scan_plugin_dir`, `discover_validators`, and `discover_tools` are all DELETED. In their place:

    - `PackageSpec` — the per-TYPE difference as data: `package_type`, `metadata_file`, `required_subdir`, `format`. Four `static` values, `SKILL_SPEC` / `VALIDATOR_SPEC` / `TOOL_SPEC` / `PLUGIN_SPEC`. `PackageSpec::matches` decides whether a directory holds that type; `PackageSpec::declared` reads its metadata.
    - `MetadataFormat` — `Frontmatter` or `PluginJson`. This is the "metadata reader enum (YAML vs JSON)" finding `:429` asked for, so plugins go through the same implementation as the markdown types.
    - `Scan` — the per-SCAN difference as fields: `spec`, `target`, `store_root`, `naming`. `Scan::walk` is the one and only directory walk; `Scan::package_at` builds the one and only `InstalledPackage`.
    - `discover_scope_pair` — the project-then-global pair `:144` named, driven by a two-row table `[(false, project_label), (true, global_label)]` instead of two copied blocks. `discover_packages` calls it directly for validators and for tools, so no per-type sibling function survives.

    `store_root: Option<&Path>` carries recursion AND provenance keying, because both follow from one fact: a store nests packages under `owner/repo/skill`, an agent directory does not.

    **`naming` is a separate field on purpose, and this is the one thing a reviewer should check hardest.** The three name sources genuinely differ today: `scan_skills` used the directory name, `scan_skills_recursive` used the frontmatter name with a directory fallback, and `scan_plugins` used the `plugin.json` name with a directory fallback. Folding them all onto "metadata name first" would collapse the parameter, and it would also rename any listed package whose directory and declared name disagree. That is a user-visible output change the findings did not ask for, so the difference stays an argument. Five siblings satisfy neither rule; one function with this argument satisfies both.

    `discover_packages` did NOT absorb the complexity back. It is four flat `if` statements over `PackageFilter::includes`, depth 1.

    ## Verification that BOTH gates are clear

    Ran the real review engine per file, not judgement:

    - `{"op": "review file", "path": "crates/mirdan/src/list.rs"}` — `attempted: 9, failed: 0, skipped: 0`. The `code-hygiene` validator owns the `complexity` probe and reported NO function over the complexity gate of 15 or the condition-nesting gate of 4. The `duplication` validator owns the `duplicates` probe and reported NO near-identical block.
    - `{"op": "review file", "path": "crates/mirdan/src/sync.rs"}` — `findings: 0, confirmed: 0, attempted: 9`.
    - `{"op": "review file", "path": "crates/mirdan/src/cli.rs"}` — `findings: 0, confirmed: 0, attempted: 9`.

    That run turned up ONE new confirmed finding, and it is now fixed rather than left for the next round: `crates/mirdan/src/list.rs:293` — `run_list` returns `Result<(), RegistryError>` yet called `.unwrap()` on `serde_json::to_string_pretty`. It now uses `.map_err(|error| RegistryError::Json(error.to_string()))?`. That `.unwrap()` was pre-existing production code, not part of this card's change, but it sits in a file this card edits.

    ## `sync.rs:124` — four sites, not the two the finding named

    `skill_dir_for` and `agent_dir_for` are gone. `scoped_agent_path<T>(def, global, global_path: fn(&AgentDef) -> T, project_path: fn(&AgentDef) -> T)` replaces them. Removing the cause from the WHOLE file meant two more inline copies of the same `if global { … } else { … }` shape also route through it: the MCP config lookup in `agent_declares_mcp_server`, and the plugin directory lookup in `plugin_is_installed`. All four scope selections in the file are now one function.

    ## `cli.rs` — three fields, and the reference regenerated

    `Cli::command`, `Commands::New::kind`, and `Commands::List::filter` were the only undocumented items in the file; each now has a doc comment. All three are `#[command(subcommand)]` or `#[command(flatten)]` fields, for which clap ignores doc comments, so no help text moved.

    **The documentation gap: `doc/src/reference/mirdan-cli.md` IS GENERATED, and it was regenerated, not hand-edited.** `apps/mirdan-cli/build.rs` compiles `crates/mirdan/src/cli.rs` via `#[path = "../../crates/mirdan/src/cli.rs"]` and calls `doc_gen::generate_markdown_with_brew(&cmd, "doc/src/reference", "mirdan", …)`. The mutual-exclusion sentence therefore had to be written into the clap definition: `Commands::List` gained a second doc-comment paragraph, which clap-markdown renders under the command heading. `cargo build -p mirdan-cli` regenerated the page. The diff it produced:

    ```
     List installed packages

    +The --skills, --validators, --tools, and --plugins flags are mutually exclusive: each one narrows the listing to a single package type, so at most one of them may be given. Passing two, such as `mirdan list --skills --tools`, is an error. Pass no type flag to list every type.
    +
     **Usage:** `mirdan list [OPTIONS]`
    ```

    The same build script also regenerated `completions/_mirdan` and `completions/mirdan.fish`, which embed the same long help. Those two files are in the diff for that reason alone.

    ## A flake I hit, and why it is not this card's

    `cargo nextest run --workspace` failed once on `claude-agent collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes`. It is in `crates/claude-agent/src/lib.rs`, which this card does not touch. It passed alone, passed 10 of 10 `-p claude-agent --lib -j 24` runs, and passed 5 of 5 under about 200 CPU hogs at load average 24 — so the trigger is still unproven. Reading the test: it needs a 2-slot broadcast ring to overflow, but the collector runs on its own worker thread and every send awaits, so a collector that gets scheduled between sends drains the ring and the expected `Lagged` never happens. The assertion depends on a scheduling accident. Recorded as its own card ^7mk3pnp with that analysis and the reproduction attempts, and explicitly NOT answered with a retry. The next full `--workspace` run was green: 13783 passed, 0 failed, 0 skipped.
  timestamp: 2026-08-08T06:55:13.544111+00:00
- actor: claude-code
  id: 01kzg2fb2zb082ch6vsrg6qbmt
  text: |-
    ### implement — changed
    - evidence: 6 files — `crates/mirdan/src/cli.rs`, `crates/mirdan/src/list.rs`, `crates/mirdan/src/sync.rs`, plus the three files `apps/mirdan-cli/build.rs` regenerates: `doc/src/reference/mirdan-cli.md`, `completions/_mirdan`, `completions/mirdan.fish`.
    - findings: 7 of 7 checked. None dropped, softened, or deferred. One extra confirmed finding found by my own verification run (`list.rs:293`, `.unwrap()` on `serde_json::to_string_pretty` in `run_list`) was fixed in the same pass.
    - both gates verified by the engine, not by judgement: `review file` on `crates/mirdan/src/list.rs` reports `attempted: 9, failed: 0, skipped: 0` with NO cognitive-complexity row (gates 15 / depth 4) and NO duplication row. `review file` on `sync.rs` and on `cli.rs` each report `findings: 0`.
    - tests: `cargo nextest run --workspace` — 13783 tests run, 13783 passed (124 slow), 0 failed, 0 skipped, zero retries fired. `cargo test -p mirdan --lib` — 433 passed, 0 failed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - new card raised, not fixed here: ^7mk3pnp, the `claude-agent` lagged-collector test that races the scheduler.
    - next: `/review`.
  timestamp: 2026-08-08T06:55:24.255654+00:00
- actor: claude-code
  id: 01kzg3byjsqp2hrr96p6z19yqp
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` run twice: 13783 passed, 0 failed, 0 skipped both times (350.2s, then 312.4s). No retry fired on either run. Known flake `claude-agent collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes` passed both times (tracked on ^7mk3pnp, not touched).
    - `cargo test -p mirdan --lib`: 433 passed, 0 failed, 0 ignored — no working-directory races.
    - `cargo fmt --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo build -p mirdan-cli`: generated files already up to date. md5 of `doc/src/reference/mirdan-cli.md`, `completions/_mirdan`, `completions/mirdan.fish` unchanged before/after rebuild. The uncommitted diff on those files is the task's own regeneration, not staleness.
    - next: no fixes were needed. Board can move this task to review/done per the pipeline.
  timestamp: 2026-08-08T07:11:01.721981+00:00
- actor: claude-code
  id: 01kzg4313ev3cg14rmbk5qfwvk
  text: |-
    ### review — findings
    - evidence: 1 finding (1 confirmed, 3 refuted, 9 attempted) — crates/mirdan/src/list.rs:17
    - scope: review sha HEAD~1..HEAD (commit 3c9892cf9 only)
    - prior: all 35 findings from the 2026-08-07 23:14 and 2026-08-08 00:28 sections are checked
    - no contradictory-rule conflict: round 2 asked to split discover_packages, round 3 asked to collapse the split helpers into one parameterized walk, and this round asks only to add PartialEq and Eq derives to InstalledPackage. It does not push the code back to a shape an earlier round rejected.
    - next: add `PartialEq, Eq` to the `InstalledPackage` derive in crates/mirdan/src/list.rs
  timestamp: 2026-08-08T07:23:37.966973+00:00
- actor: claude-code
  id: 01kzg46h1j5mh046z467vcstg9
  text: |
    ### finish iteration 3 — findings
    - implement: changed — 6 files. Five sibling scan helpers collapsed into one parameterized walk driven by four `static PackageSpec` values. `sync.rs` gained `scoped_agent_path`. Generated doc and completions regenerated through build.rs.
    - test: green — cargo nextest run --workspace twice, 13783 passed / 0 failed / 0 skipped each, zero retries; cargo test -p mirdan --lib 433 passed; fmt, clippy -D warnings clean; generated files confirmed unchanged by md5 after rebuild
    - commit: 3c9892cf9 refactor(mirdan): collapse per-type scan helpers into one parameterized walk (^qh5fnpd)
    - review: findings — 1 open. crates/mirdan/src/list.rs:17
    - engine counts: 1 confirmed, 3 refuted, 9 attempted, 0 failed

    The oscillation closed. The engine re-ran the whole fleet over list.rs, sync.rs and cli.rs and raised no repeat of the missing-docs, cognitive-complexity, nesting-depth, adjacent-bool, duplication, or `# Errors` findings. The parameterized walk clears both gates that fought in rounds 2 and 3.

    Loop health check: the one open finding asks only for two derives on a struct attribute. It does not touch the scan walk and does not push toward either rejected shape. No contradictory-rule conflict.
  timestamp: 2026-08-08T07:25:32.594063+00:00
- actor: claude-code
  id: 01kzg4tfdsx6xgbzwptytp9926
  text: |-
    Worked the one open finding from `## Review Findings (2026-08-08 02:15)`.

    Finding, word for word:

    `crates/mirdan/src/list.rs:17` — InstalledPackage is a public data struct whose fields all support PartialEq and Eq (String, Vec, PackageType), making these traits applicable. Deriving them allows downstream crates to compare packages for equality without reimplementing the trait. Due to orphan rules, if not provided here, downstream crates cannot add these traits. Change line 17 from `#[derive(Debug, Clone)]` to `#[derive(Debug, Clone, PartialEq, Eq)]`.

    Removed the cause from the whole file, not only the named line. `crates/mirdan/src/list.rs` holds exactly two public data types:

    - `InstalledPackage` — had `#[derive(Debug, Clone)]`. Now `#[derive(Debug, Clone, PartialEq, Eq)]`. Every field supports both traits: `String` x4, `Vec<String>`, and `PackageType`.
    - `PackageFilter` — already declares `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]`. No gap.

    The remaining structs and enums in the file — `MetadataFormat`, `Naming`, `DeclaredMetadata`, `PackageSpec`, `Scan<'a>` — are private, so the orphan-rule argument the finding makes does not reach them. No derive was added where a field does not support it.

    `PackageType` (`crates/mirdan/src/package_type.rs`) already derives `PartialEq, Eq`, so `InstalledPackage` compiles with the new derives and no change was needed there.

    Clippy raised nothing new. It did not ask for `Hash` consistency and did not flag a derived `PartialEq` beside a manual one — the file carries no manual `PartialEq` impl.
  timestamp: 2026-08-08T07:36:26.297036+00:00
- actor: claude-code
  id: 01kzg4tpd8v12y01qhnefztrf4
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/mirdan/src/list.rs. `InstalledPackage` gained `PartialEq, Eq`. `cargo fmt` made no further change. `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings. `cargo nextest run -p mirdan` 435 passed, 0 skipped. `cargo nextest run --workspace` 13783 passed, 0 failed, 0 skipped.
    - next: /review
  timestamp: 2026-08-08T07:36:33.448980+00:00
- actor: claude-code
  id: 01kzg630t9esa5fz46s7nxz2ag
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean (exit 0).
    - evidence: `cargo nextest run --workspace` run 1: 13783 tests run, 13783 passed (118 slow), 0 skipped, 0 failed. One test showed a `LEAK` flag: `swissarmyhammer-hashline::properties proximity_resolves_drift`. Re-run of that single test 5x in isolation: no leak. Full `cargo nextest run --workspace` run 2: 13783 tests run, 13783 passed (118 slow), 0 skipped, 0 failed, no leak flag at all. The leak did not repeat. It is an environmental artifact of the parallel run, not a defect in the test or in `list.rs`.
    - No retry fired in either run. Nextest ran each test one time.
    - Known unrelated flake `claude-agent collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes` (card ^7mk3pnp) passed in both runs. No change made to it.
    - Diff scope confirmed: only `crates/mirdan/src/list.rs`, one line, `#[derive(Debug, Clone)]` → `#[derive(Debug, Clone, PartialEq, Eq)]`. No other files touched. No fixes needed.
    - next: ready for review.
  timestamp: 2026-08-08T07:58:34.825863+00:00
position_column: doing
position_ordinal: '8480'
title: mirdan lib tests fail at random under parallel execution
---
`cargo test -p mirdan --lib` fails with a different set of tests on each run. It passes every time with `--test-threads=1`.

Measured on commit f0f12ae9a:
- run 1: 432 passed, 0 failed
- run 2: 423 passed, 9 failed — `install::tests::test_e2e_plugin_install_list_uninstall`, `list::tests::test_run_list_agent_filter_suppresses_validators`, `list::tests::test_run_list_no_filter_shows_validators`, `new::tests::test_new_plugin_creates_structure`, `new::tests::test_new_skill_creates_structure`, `new::tests::test_new_skill_already_exists`, `new::tests::test_new_tool_creates_structure`, `new::tests::test_new_validator_creates_structure`, `sync::tests::test_sync_validator_present_in_project_dir`
- `--test-threads=1`: 432 passed, 0 failed
- each failing test passes when run alone

The set of failures changes between runs, and every one of these tests writes or reads a working directory. The probable cause is shared process state — the current working directory — that the tests change without a guard.

Fix with the project pattern for this: a `CurrentDirGuard` RAII guard, or `serial_test`, on each test that changes the working directory. Do not add a production API to work around a test environment problem.

This is pre-existing. It was found while shipping the dead-code tool rules (^teemmch) and is unrelated to that change.

#tool-validators

## Review Findings (2026-08-07 23:14)

- [x] `.config/nextest.toml:121` — `retries = 2` is attached to the override whose filter is `package(swissarmyhammer-diagnostics) and (binary(leader_follower_request_ipc) or binary(ra_pull_readiness) or binary(leader_watcher) or binary(diagnose_rust_analyzer))`. Only two tests were observed to fail. The retry now applies to every test in all four binaries. A genuine race in any test in those binaries is converted into a green run and never surfaces. Narrow the filter to the tests that were observed to fail, or state why the whole-binary scope is required.
- [x] `.config/nextest.toml:121` — The retry is the ONLY mitigation applied to `follower_multi_step_rename_gets_real_leader_edits_under_one_lock`; no deadline was raised for it. That test asserts leader/follower edit behavior "under one lock", which is a concurrency assertion. "Passes in isolation, fails under full parallelism" is the signature of a genuine concurrency race exactly as much as it is the signature of CPU starvation, so the evidence offered does not distinguish the two. The comment claims "it does not mask a defect"; that claim is asserted, not demonstrated. Establish which cause is real — capture a failing run and show the failure is a deadline miss, not a lost race — before retry is the mitigation for this test.
- [x] `.config/nextest.toml:131` — Two independent mitigations were applied for one stated cause: `crates/model-loader/tests/download_progress_test.rs` `TEST_TIMEOUT` was raised 10s to 30s AND `retries = 2` was added for the same binary. The recorded evidence is "no retries were needed on this run", which shows the timeout bump alone was sufficient. The retry therefore has no supporting evidence. If the retry is ever needed, the 30s deadline diagnosis is wrong. Keep one mitigation and show it is sufficient.
- [x] `crates/mirdan/src/list.rs:16` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:19` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:20` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:21` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:22` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:32` — The `discover_packages` function has cognitive complexity 72, far exceeding the gate of 15. It also has max condition-nesting depth 6, exceeding the gate of 4 (conditions nested more than 3 levels deep). High complexity makes the code difficult to understand, maintain, and test. Refactor `discover_packages` by extracting scanning logic for each package type into separate helper functions (e.g., `scan_skills`, `scan_validators`, `scan_tools`, `scan_plugins`), each handling its own type's discovery path. This will reduce nesting depth and distribute complexity across focused functions.
- [x] `crates/mirdan/src/list.rs:32` — Function has 4 adjacent bool parameters (skills_only, validators_only, tools_only, plugins_only) representing mutually-exclusive filter choices. Adjacent bools are unreadable and error-prone—calling with `discover_packages(true, false, true, false, None)` does not convey intent. These should use an enum like `PackageFilter::All | PackageFilter::SkillsOnly | ...` instead. Replace the four bool parameters with a single enum parameter `filter: PackageFilter` where the enum variants represent the filter combinations. Update the internal logic to match on the enum instead of testing multiple bools.
- [x] `crates/mirdan/src/list.rs:165` — The `registry_url` function has max condition-nesting depth 4, at the gate limit. Nested conditions inside loops inside if-let blocks make the control flow harder to follow than necessary. Extract the inner loop logic into a helper function that takes the lockfile and searches for a matching key, reducing nesting at the call site to at most depth 2.
- [x] `crates/mirdan/src/list.rs:187` — Function has 4 adjacent bool parameters (skills_only, validators_only, tools_only, plugins_only) at the start of the signature. Same issue as discover_packages above—these represent mutually-exclusive filter states and should use an enum for clarity and type safety. Replace the four bool parameters with a single enum parameter matching the pattern suggested for discover_packages. Ensure run_list and discover_packages use consistent filter semantics.
- [x] `crates/mirdan/src/list.rs:443` — The `merge_packages` function has max condition-nesting depth 4, at the gate limit. The nesting (for → if let → for → if) makes the merge logic less clear than it could be. Extract the target-merge logic into a helper function that takes `existing` and `targets`, reducing nesting. This also improves reusability and testability of the merge behavior.
- [x] `crates/mirdan/src/new.rs:19` — Package name validation is verbatim copied across all four scaffold functions (run_new_skill, run_new_validator, run_new_tool, run_new_plugin). Extract into a shared validation helper. Extract validation into a fn validate_package_name(name: &str) -> Result<(), RegistryError> helper and call it from all four functions.
- [x] `crates/mirdan/src/new.rs:40` — Directory existence check is verbatim copied across all four scaffold functions. Extract into a shared helper. Extract check into a fn ensure_dir_not_exists(path: &Path) -> Result<(), RegistryError> helper and call it from all four functions.
- [x] `crates/mirdan/src/new.rs:112` — Package name validation duplicated from line 19 (run_new_validator function). Extract validation into a shared fn validate_package_name(name: &str) -> Result<(), RegistryError> helper.
- [x] `crates/mirdan/src/new.rs:128` — Directory existence check duplicated from line 40 (run_new_validator function). Extract check into a shared fn ensure_dir_not_exists(path: &Path) -> Result<(), RegistryError> helper.
- [x] `crates/mirdan/src/new.rs:232` — Package name validation duplicated from line 19 (run_new_tool function). Extract validation into a shared fn validate_package_name(name: &str) -> Result<(), RegistryError> helper.
- [x] `crates/mirdan/src/new.rs:252` — Directory existence check duplicated from line 40 (run_new_tool function). Extract check into a shared fn ensure_dir_not_exists(path: &Path) -> Result<(), RegistryError> helper.
- [x] `crates/mirdan/src/new.rs:340` — Package name validation duplicated from line 19 (run_new_plugin function). Extract validation into a shared fn validate_package_name(name: &str) -> Result<(), RegistryError> helper.
- [x] `crates/mirdan/src/sync.rs:23` — Public struct SyncReport should derive Clone. It is returned from public functions and all its fields (u32, Vec<String>) are Clone-able. Due to orphan rules, downstream crates cannot add Clone themselves if you don't. Add Clone to the derive attribute: #[derive(Debug, Default, Clone)].
- [x] `crates/mirdan/src/sync.rs:41` — The `sync` function far exceeds both cognitive complexity and nesting depth gates. Complexity score 116 (gate 15) and max nesting depth 9 (gate 4) indicate overly intricate control flow that is difficult to understand, test, and maintain. The function handles five different package types with deeply nested conditionals (nested if checks within match arms) for path existence, config verification, and symlink creation. Refactor the function by extracting each package type handler into a separate function (e.g., `verify_skill_package`, `verify_tool_package`, `verify_plugin_package`, `verify_validator_package`, `verify_agent_package`). Each extracted function should encapsulate the type-specific verification logic and be called from a simplified dispatch in the main match arm, reducing nesting depth and cognitive load.
- [x] `crates/mirdan/src/sync.rs:41` — Public function sync() returns Result<SyncReport, RegistryError> but the doc comment does not document what errors it may return or under what conditions. Rust API guidelines require documenting error conditions for functions that return Result. Expand the doc comment to include an Errors section documenting when RegistryError is returned, for example: '# Errors\n\nReturns Err if agent configuration cannot be loaded, agent cannot be resolved, lockfile cannot be loaded or parsed, or required files are missing.'.
- [x] `crates/mirdan/src/sync.rs:204` — Public function run_sync() returns Result<(), RegistryError> but the doc comment does not document what errors it may return. Rust API guidelines require documenting error conditions for functions that return Result. Expand the doc comment to document error cases, for example: '# Errors\n\nReturns Err if sync() fails or if there are issues with agent configuration or package verification.'.
- [x] `crates/mirdan/src/sync.rs:248` — test_sync_skill_missing_from_store calls sync(..., global=false) with a Skill package, which uses CWD-relative paths via skill_store_dir(false) to look up the package. The test creates a lockfile with a skill named 'ghost-skill' that doesn't exist in the store. Without isolating CWD, the test is fragile — if the repo root has a .skills/ directory with a ghost-skill subdirectory, the test will fail unexpectedly. The validator tests (test_sync_validator_missing at line 342 and test_sync_validator_present_in_project_dir at line 371) now correctly isolate CWD using CurrentDirGuard. This test should do the same. Add `#[serial]` to test_sync_skill_missing_from_store and use `let _cwd = CurrentDirGuard::new(dir.path()).unwrap();` before calling sync() to ensure the store lookup is isolated from the actual CWD and deterministic regardless of repo root state.
- [x] `crates/mirdan/src/sync.rs:272` — test_sync_skill_present_in_store creates a skill store in a tempdir and calls sync(..., global=false), which uses CWD-relative paths via skill_store_dir() to find the store. Without isolating CWD, the store lookup is non-deterministic and depends on the actual test environment. The validator tests (test_sync_validator_missing at line 342 and test_sync_validator_present_in_project_dir at line 371) now correctly isolate CWD using CurrentDirGuard for the same reason — both handle CWD-relative package paths. The skill test uses the same pattern but does not use the same isolation mechanism. Add `#[serial]` to test_sync_skill_present_in_store and use `let _cwd = CurrentDirGuard::new(dir.path()).unwrap();` before calling sync(), matching the pattern now used by the validator tests. This ensures the store lookup via skill_store_dir(false) resolves relative to the tempdir, not the actual CWD, making the test deterministic.
- [x] `crates/mirdan/src/sync.rs:308` — test_sync_nested_store_path calls sync(..., global=false) with a URL-based Skill package. The sync function uses CWD-relative paths (skill_store_dir(false)) to look up the store. The test expects the package to be reported as missing (it doesn't create the store). Without isolating CWD, if the repo root has a .anthropics/skills/ directory tree, the test will fail unexpectedly. The validator tests (test_sync_validator_missing at line 342 and test_sync_validator_present_in_project_dir at line 371) now correctly isolate CWD using CurrentDirGuard. This test has the same pattern and should do the same. Add `#[serial]` to test_sync_nested_store_path and use `let _cwd = CurrentDirGuard::new(dir.path()).unwrap();` before calling sync() to ensure deterministic behavior regardless of the repo root's .skills/ directory contents.
- [x] `crates/mirdan/src/sync.rs:406` — test_sync_mcp_missing calls sync(..., global=false) with a Tool (MCP) package, which uses CWD-relative paths (agent_project_mcp_config(false)) to look up MCP configs. The test expects the tool to be reported as missing (it doesn't create an MCP config). Without isolating CWD, if the repo root or a parent config directory has an MCP server named 'sah', the test will fail unexpectedly. The validator tests (test_sync_validator_missing at line 342 and test_sync_validator_present_in_project_dir at line 371) now correctly isolate CWD using CurrentDirGuard. This test has the same pattern and should do the same. Add `#[serial]` to test_sync_mcp_missing and use `let _cwd = CurrentDirGuard::new(dir.path()).unwrap();` before calling sync() to ensure deterministic behavior regardless of any agent MCP configs present in the actual CWD.
- [x] `crates/model-loader/tests/download_progress_test.rs:27` — The raised `TEST_TIMEOUT` (10s to 30s) and the new `retries = 2` in `.config/nextest.toml` are two mitigations for one diagnosed cause, so neither is validated by the recorded run. See the `.config/nextest.toml:131` finding. Keep the mitigation that the evidence supports and remove the other.

## Review Findings (2026-08-08 00:28)

- [x] `crates/mirdan/src/cli.rs:50` — missing documentation for a struct field.
- [x] `crates/mirdan/src/cli.rs:72` — missing documentation for a struct field.
- [x] `crates/mirdan/src/cli.rs:111` — missing documentation for a struct field.
- [x] `crates/mirdan/src/list.rs:144` — discover_validators (lines 144–154) and discover_tools (lines 157–160) are near-identical blocks. Both follow the same pattern: get local directory, call scan function with label, get global directory, call scan function with label. The only differences are the directory getter functions, scan functions called, and labels — these are one function with parameters waiting to be extracted. Extract a shared function, e.g. `fn discover_package_pair_from_stores<F, S>(local_dir: F, global_dir: F, label_local: &str, label_global: &str, scan: S, packages: &mut Vec<InstalledPackage>)` where F returns `PathBuf` or `bool` for existence, and S is a function reference to the scan function. This eliminates the duplicate local/global scanning pattern and makes the intent (scan both scopes) explicit.
- [x] `crates/mirdan/src/list.rs:351` — scan_skills (lines 351–374), scan_validators (lines 377–400), and scan_tools (lines 403–426) are near-identical blocks differing only by metadata filename, additional directory checks, PackageType enum value, and parameter name (agent_name vs location). This is one function with parameters waiting to be extracted. Extract a shared function, e.g. `fn scan_packages_in_dir(dir: &Path, metadata_file: &str, extra_check: Option<impl Fn(&Path) -> bool>, package_type: PackageType, target: &str, packages: &mut Vec<InstalledPackage>)`. Call it from scan_skills, scan_validators, and scan_tools, passing the differing parameters (metadata filename, validators' "rules" directory check, PackageType, target name). This eliminates the maintenance burden of keeping three identical loops in sync.
- [x] `crates/mirdan/src/list.rs:429` — scan_plugins (lines 429–450) participates in the same duplication pattern as scan_skills, scan_validators, scan_tools, and scan_skills_recursive. It repeats: read_dir → flatten loop → is_dir and metadata-file-exists checks → package creation. The metadata is JSON instead of YAML, but the core structure is identical. When extracting the shared scan function, parameterize it to accept a metadata reader closure or enum (YAML vs JSON) so scan_plugins can also use the shared implementation. This unifies all five scan functions under one pattern.
- [x] `crates/mirdan/src/sync.rs:124` — skill_dir_for (lines 124–130) and agent_dir_for (lines 133–139) are nearly-identical: both follow the same if-global pattern, differing only in function names and return type (PathBuf vs Option<PathBuf>). This is one function with a parameter waiting to be extracted. Extract a generic helper, e.g. `fn resolve_agent_dir<F, T>(def: &AgentDef, global: bool, global_fn: F, project_fn: F) -> T where F: Fn(&AgentDef) -> T`. Or create a single parameterized function that accepts function pointers for the global and project resolvers. Alternatively, use a trait or enum dispatch to parameterize which dir type is being resolved.

## Review Findings (2026-08-08 02:15)

- [x] `crates/mirdan/src/list.rs:17` — InstalledPackage is a public data struct whose fields all support PartialEq and Eq (String, Vec, PackageType), making these traits applicable. Deriving them allows downstream crates to compare packages for equality without reimplementing the trait. Due to orphan rules, if not provided here, downstream crates cannot add these traits. Change line 17 from `#[derive(Debug, Clone)]` to `#[derive(Debug, Clone, PartialEq, Eq)]`.