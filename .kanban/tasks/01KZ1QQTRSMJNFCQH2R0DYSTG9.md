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
- actor: claude-code
  id: 01kz6rs2pzavmwwnhf1z870fgt
  text: |-
    ### commit — changed
    - evidence: 8443ab358 fix(swissarmyhammer-tools): stop shell-tool tests from littering .shell in crate dirs
    - next: none (not pushed, per instruction)
  timestamp: 2026-08-04T16:12:47.711526+00:00
- actor: claude-code
  id: 01kz6t05wh67deg6hmm1t097cv
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (commit 8443ab358) — 23 raw findings, confirmed 23. Of these, 7 are on production code and are kept (health_registry.rs:34,47,69; tool_config.rs:45,72,152,212). The remaining 16 target test code that already existed before this checkpoint (magic numbers and string-literal duplication in pre-existing test bodies in health_registry.rs, tool_config.rs, file_size_limits.rs, mcp_server_paritys.rs) — dropped per the review skill's blanket test-refactor exception.
    - next: implementer must address the 7 open findings, then re-review.
  timestamp: 2026-08-04T16:34:08.913195+00:00
- actor: claude-code
  id: 01kz6t0s3x2aaenzpps0b4rpxt
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 4 files (tool_config.rs, tool_registry.rs, file_size_limits.rs, mcp_server_paritys.rs); filed follow-up ^2kqcswb for McpServer::new() call sites out of scope
    - test: green — cargo nextest run --workspace, 13497 passed; fixed additional unguarded CWD sites in health_registry.rs, mcp_tools_registration.rs, end_to_end.rs
    - commit: 8443ab358
    - review: findings — crates/swissarmyhammer-tools/src/health_registry.rs:34,47,69; crates/swissarmyhammer-tools/src/mcp/tool_config.rs:45,72,152,212
  timestamp: 2026-08-04T16:34:28.605107+00:00
- actor: claude-code
  id: 01kz6tx26eyw5v4k6997y2dtfy
  text: |-
    Fixed all 7 review findings (iteration 1):

    1. `health_registry.rs::run_health_checks` — extracted 3 helpers: `check_prompts_directory` (handles both the user- and local-prompts existence/count branches), `validate_frontmatter_file` (read + parse a single file), and `collect_yaml_errors_from_dir` (walk a directory, collect errors via `validate_frontmatter_file`). `run_health_checks` is now a flat sequence of calls with no nested loops, which resolves both the cognitive-complexity-23 and nesting-depth-4 gate failures.
    2. `.prompts"` literal — extracted `const PROMPTS_DIR_NAME: &str = ".prompts";`, replaced all 3 occurrences (user prompts join, local prompts join, dirs_to_check push).
    3. `"Local prompts directory"` literal — extracted `const LOCAL_PROMPTS_CHECK_NAME`; also extracted `const USER_PROMPTS_CHECK_NAME` for the same duplication pattern on the user-prompts side (not explicitly named in the finding, but same root cause — a hardcoded check-name literal — so fixed for the whole file per the "root cause across the whole file" rule).
    4/5. `ToolEntry` and `ToolConfig` now derive `Eq` in addition to `PartialEq`. Verified: `ToolEntry` has only a `bool` field (Eq-safe); `ToolConfig` has `HashMap<String, ToolEntry>` (Eq-safe once `ToolEntry: Eq`).
    6. `".sah"` literal — extracted `const SAH_CONFIG_DIR: &str = ".sah";`, replaced both occurrences (`global_config_path`, `project_config_path`).
    7. `ToolConfigWatcher` — added `#[derive(Debug, Clone, PartialEq, Eq)]`. Verified all 4 fields (`Option<PathBuf>` x2, `Option<SystemTime>` x2) support all four derives — `SystemTime` implements `Eq`.

    Verification: `cargo nextest run -p swissarmyhammer-tools health_registry tool_config` — 22/22 passed. `cargo build -p swissarmyhammer-tools` clean. `cargo clippy -p swissarmyhammer-tools --lib -- -D warnings` clean, zero warnings. `mcp__sah__diagnostics check working` — 0 errors, 0 warnings.

    Discovery: ran an unsolicited self-check `review file` pass on both touched files afterward. It surfaced 7 additional findings, all against code that predates this card's edits (a case-sensitive `.md` extension check, a capitalized error message, and duplication in `project_config_path`/`global_config_path` and in `ToolConfigWatcher`'s mtime-reading code — none introduced by this card's constant-extraction/derive-only edits). Per "new work discovered goes on a new card," filed these as ^kymcq7j rather than expanding this card's scope.
  timestamp: 2026-08-04T16:49:55.406585+00:00
- actor: claude-code
  id: 01kz6tx7b8kmzr435k4jg4qeh0
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-tools/src/health_registry.rs, crates/swissarmyhammer-tools/src/mcp/tool_config.rs. All 7 review findings fixed and checked off. `cargo nextest run -p swissarmyhammer-tools health_registry tool_config` 22/22 passed; `cargo clippy -p swissarmyhammer-tools --lib -- -D warnings` clean.
    - next: ready for /review. Filed ^kymcq7j for pre-existing latent findings surfaced by a self-check review pass (out of this card's scope).
  timestamp: 2026-08-04T16:50:00.680435+00:00
- actor: claude-code
  id: 01kz6v3pg7s8296t6pvrdn4ek0
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13497 passed, 0 failed, 0 skipped; cargo clippy --workspace --all-targets --all-features -- -D warnings — clean, 0 warnings
    - next: none
  timestamp: 2026-08-04T16:53:32.807305+00:00
- actor: claude-code
  id: 01kz6v51v4hzzssyhhm9e4yqk5
  text: |-
    ### commit — changed
    - evidence: 66250a1de fix(tools): reduce complexity + add missing derives in health_registry/tool_config
    - next: none
  timestamp: 2026-08-04T16:54:17.188613+00:00
- actor: claude-code
  id: 01kz6vsxytsm0etafzpysthr7s
  text: |-
    ### review — findings
    - evidence: 5 findings (1 refuted — pre-existing test at tool_config.rs:489 dropped per test-refactor exception) — crates/swissarmyhammer-tools/src/health_registry.rs:138, crates/swissarmyhammer-tools/src/health_registry.rs:147, crates/swissarmyhammer-tools/src/mcp/tool_config.rs:157, crates/swissarmyhammer-tools/src/mcp/tool_config.rs:180, crates/swissarmyhammer-tools/src/mcp/tool_config.rs:226
    - scope: review sha HEAD~1..HEAD (commit 9b2e89707)
    - next: implementer must fix all 5 findings, then re-review
  timestamp: 2026-08-04T17:05:41.338908+00:00
- actor: claude-code
  id: 01kz6vtcxrpbjk3ebaehhw1a5b
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — health_registry.rs (extracted helpers + constants), tool_config.rs (Eq derives, SAH_CONFIG_DIR const, ToolConfigWatcher derives); filed follow-up ^kymcq7j for pre-existing issues found but out of scope
    - test: green — cargo nextest run --workspace, 13497 passed; clippy clean
    - commit: 9b2e89707
    - review: findings — crates/swissarmyhammer-tools/src/health_registry.rs:138,147; crates/swissarmyhammer-tools/src/mcp/tool_config.rs:157,180,226
  timestamp: 2026-08-04T17:05:56.664703+00:00
- actor: claude-code
  id: 01kz6w7kvyhqmgnzjg8q3x4n0j
  text: |-
    Fixed all 5 review findings (iteration 2):

    1. `health_registry.rs:138` — extracted `iter_markdown_files(dir)` shared by `count_markdown_files()` (now `iter_markdown_files(path).count()`) and `collect_yaml_errors_from_dir()` (now iterates it in its `filter_map`), removing the duplicated walk-and-filter block.
    2. `health_registry.rs:147` — `iter_markdown_files` matches the extension case-insensitively via `ext.eq_ignore_ascii_case("md")` instead of `== Some("md")`, so `.MD`/`.Md` files are no longer silently excluded. Added `test_markdown_files_are_matched_case_insensitively`, which verifies both `count_markdown_files` (mixed-case files all counted) and `collect_yaml_errors_from_dir` (a `.MD` file with bad frontmatter is still flagged).
    3. `tool_config.rs:157` — extracted `resolve_config_path<F: FnOnce() -> Option<PathBuf>>(dir_source: F)`; `global_config_path()` is now `resolve_config_path(dirs::home_dir)` and `project_config_path()` is `resolve_config_path(find_git_repository_root)`.
    4. `tool_config.rs:180` — extracted `load_and_merge_layer<F: FnOnce() -> Option<PathBuf>>(config: &mut ToolConfig, get_path: F)`; `load_merged_tool_config()` now calls it twice (once per layer) instead of two near-identical nested-if blocks.
    5. `tool_config.rs:226` — added `/// Equivalent to [`Self::new`].` doc comment on `ToolConfigWatcher`'s `Default::default()`.

    Root-cause sweep of both files (per the "recurring findings in the same file" pattern from rounds 1 and 2) turned up two more instances of the same duplication class, fixed proactively:

    - `tool_config.rs` — `ToolConfigWatcher::new()` and `check_and_reload()` both independently computed `global_path.as_ref().and_then(|p| file_mtime(p))` / `project_path.as_ref().and_then(|p| file_mtime(p))`. Extracted `read_layer_mtimes(global_path: &Option<PathBuf>, project_path: &Option<PathBuf>) -> (Option<SystemTime>, Option<SystemTime>)`, called from both.
    - `health_registry.rs` — `run_health_checks()` called `dirs::home_dir()` twice and recomputed `home.join(PROMPTS_DIR_NAME)` twice (once for the user-prompts check, once when building `dirs_to_check` for the YAML scan). Now resolves `home_prompts` once and reuses it for both the check and the YAML-scan list.

    Verification:
    - `cargo nextest run -p swissarmyhammer-tools health_registry tool_config` — 23/23 passed (2 pre-existing "LEAK" markers on unrelated tests, consistent with prior iterations' notes).
    - `cargo clippy -p swissarmyhammer-tools --lib --tests -- -D warnings` — clean.
    - `cargo nextest run --workspace` — 13498 passed, 0 failed, 0 skipped.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
    - `mcp__sah__diagnostics check working` — 0 errors, 0 warnings.

    All 5 checklist items under "Review Findings (2026-08-04 11:54)" flipped to `[x]` on the task description.

    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-tools/src/health_registry.rs, crates/swissarmyhammer-tools/src/mcp/tool_config.rs. All 5 review findings fixed and checked off, plus 2 additional same-root-cause duplication instances fixed proactively (ToolConfigWatcher mtime reading, run_health_checks home-dir path resolution). `cargo nextest run -p swissarmyhammer-tools health_registry tool_config` 23/23 passed; `cargo nextest run --workspace` 13498 passed; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
    - next: ready for /review.

    step: implement
    outcome: changed
    evidence: 2 files — crates/swissarmyhammer-tools/src/health_registry.rs, crates/swissarmyhammer-tools/src/mcp/tool_config.rs
    task: ^0dystg9
  timestamp: 2026-08-04T17:13:09.758697+00:00
- actor: claude-code
  id: 01kz6w9n438m8nb0bzefgfj2jq
  text: 'Correction: an earlier `update task` call in this session accidentally wrote the description with literal `\n` escape sequences instead of real newlines, and wiped the tags (an update task call replaces the whole description). Re-issued the update with a proper multi-line description and restored tags (bug, shelltool, test-hygiene, tools). All 5 round-2 review findings remain checked off; no content was lost.'
  timestamp: 2026-08-04T17:14:16.579556+00:00
- actor: claude-code
  id: 01kz6wf3ykaqa1wyr463ar5022
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13498 passed, 0 failed, 0 skipped; `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, no warnings
    - next: ready for review
  timestamp: 2026-08-04T17:17:15.603334+00:00
- actor: claude-code
  id: 01kz6wg3eca9qa57dj2yewyfxh
  text: |-
    ### commit — changed
    - evidence: 06fc53d37 refactor(swissarmyhammer-tools): extract helpers in health_registry and tool_config
    - next: proceed to review
  timestamp: 2026-08-04T17:17:47.852633+00:00
- actor: claude-code
  id: 01kz6wwdag5b1z4w1h3scfana0
  text: |-
    ### review — findings
    - evidence: 1 finding — crates/swissarmyhammer-tools/src/mcp/tool_config.rs:305
    - next: fix read_layer_mtimes to accept Option<&Path>, update call sites at lines 249 and 266, re-run review
  timestamp: 2026-08-04T17:24:31.184555+00:00
- actor: claude-code
  id: 01kz6wz34z7t480h9pe1n5bkzx
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — health_registry.rs (iter_markdown_files helper, case-insensitive .md, dedup home_dir resolution), tool_config.rs (resolve_config_path, load_and_merge_layer, read_layer_mtimes helpers, doc comment on default())
    - test: green — cargo nextest run --workspace, 13498 passed; clippy clean
    - commit: 06fc53d37
    - review: findings — crates/swissarmyhammer-tools/src/mcp/tool_config.rs:305
  timestamp: 2026-08-04T17:25:59.071363+00:00
- actor: claude-code
  id: 01kz6x398char0v6r3kwwx2esb
  text: |-
    Fixed the last open finding (2026-08-04 12:18): `read_layer_mtimes` in `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` accepted `&Option<PathBuf>` for both parameters. Changed the signature to `Option<&Path>` for both `global_path` and `project_path`, and simplified the body to `global_path.and_then(file_mtime)` / `project_path.and_then(file_mtime)`.

    Call sites updated:
    - `ToolConfigWatcher::new` (line ~249): `read_layer_mtimes(global_path.as_deref(), project_path.as_deref())`.
    - `ToolConfigWatcher::check_and_reload` (line ~266): `read_layer_mtimes(self.global_path.as_deref(), self.project_path.as_deref())`.

    Note: the finding text suggested `.as_ref()`, but `Option<PathBuf>::as_ref()` yields `Option<&PathBuf>`, not `Option<&Path>` — that combination fails to compile (E0308). Used `.as_deref()` instead, which deref-coerces to `Option<&Path>` and satisfies the actual intent of the finding (accept the generic `Option<&Path>`, not the concrete `PathBuf`).

    Verified: `cargo check -p swissarmyhammer-tools --lib` clean, `cargo clippy -p swissarmyhammer-tools --lib -- -D warnings` clean, `cargo nextest run -p swissarmyhammer-tools --lib tool_config` — 12/12 passed.

    Checklist item under "Review Findings (2026-08-04 12:18)" flipped to `[x]`.
  timestamp: 2026-08-04T17:28:16.396169+00:00
- actor: claude-code
  id: 01kz6x3awmm0m9zzhp38n6fq2h
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-tools/src/mcp/tool_config.rs — read_layer_mtimes now takes Option<&Path> instead of &Option<PathBuf>; call sites in ToolConfigWatcher::new and ToolConfigWatcher::check_and_reload use .as_deref(). cargo check/clippy clean, nextest tool_config tests 12/12 passed.
    - next: leave in doing for review
  timestamp: 2026-08-04T17:28:18.068058+00:00
- actor: claude-code
  id: 01kz6x5ghdv022b425hnpgw9w5
  text: 'Correction: an earlier update task call in this pass accidentally wrote the description with literal `\n` escape text and dropped the tags (progress showed 0.0, tags emptied). Immediately re-issued update task with real newlines and restored tags=[bug, shelltool, test-hygiene, tools]. Verified via get task: description renders correctly, progress is 0.944 (17/18 checked), tags restored. No functional/code impact — this was a kanban metadata mistake, not a code change.'
  timestamp: 2026-08-04T17:29:29.389130+00:00
- actor: claude-code
  id: 01kz6xbg16c28vqn36fm2t0tqh
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13498 tests run: 13498 passed (5 slow), 0 failed, 0 skipped. cargo clippy --workspace --all-targets --all-features -- -D warnings — clean, 0 warnings.
    - next: no fix was needed. Build is clean.
  timestamp: 2026-08-04T17:32:45.478478+00:00
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
      `CurrentDirGuard`-pinned temp dir).

## Review Findings (2026-08-04 11:13)

Scope: `review sha HEAD~1..HEAD` (commit 8443ab358). Findings against pre-existing test code (magic numbers, string-literal duplication inside test bodies that already existed before this checkpoint) are dropped per the review skill's test-refactor exception. Findings against production code are kept in full.

- [x] `crates/swissarmyhammer-tools/src/health_registry.rs:34` — Function run_health_checks has cognitive complexity 23 (gate 15) and max condition-nesting depth 4 (gate 4). The function performs multiple responsibilities: checking user and project prompt directories, walking directory trees, reading file contents, parsing YAML frontmatter, and collecting errors. The nested loops with multiple conditional branches (directory existence checks, file filtering, match on read result, nested if-let on parse error) create excessive complexity. Extract helper functions to reduce complexity. Create separate functions for: (1) checking and reporting a single directory's prompts, (2) validating YAML frontmatter in a single file, (3) collecting YAML errors from a directory. This makes each function easier to understand and test.
- [x] `crates/swissarmyhammer-tools/src/health_registry.rs:47` — The literal string ".prompts" is repeated 3 times in this file (lines 47, 65, 84). This is a configuration value — the name of the prompts directory — that should be defined once as a constant. Repeating it requires updating multiple places if the directory name ever changes. Define a constant at the top of the file: `const PROMPTS_DIR_NAME: &str = ".prompts";` and replace all three occurrences with references to this constant.
- [x] `crates/swissarmyhammer-tools/src/health_registry.rs:69` — The literal string "Local prompts directory" is repeated 2 times in this function (lines 69, 75). This check label should be defined once as a constant to avoid duplication and ensure consistency if the label needs to change. Define a constant at the module level: `const LOCAL_PROMPTS_CHECK_NAME: &str = "Local prompts directory";` and replace both occurrences with this constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:45` — ToolEntry implements PartialEq but not Eq, despite having no NaN-capable fields (only a bool). Per trait-implementations rule, when PartialEq is implemented, Eq must also be implemented for types that support it, since downstream crates cannot add Eq due to orphan rules. Add Eq to the derive list: #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)].
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:72` — ToolConfig implements PartialEq but not Eq, despite having no NaN-capable fields (HashMap of ToolEntry values where ToolEntry contains only bool). Downstream crates cannot add Eq due to orphan rules. Add Eq to the derive list: #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)].
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:152` — The literal string ".sah" is repeated 2 times in this file (lines 152, 160). This is a configuration value — the name of the SAH configuration directory — that should be defined once as a constant. Repeating it requires updating multiple places if the directory name ever changes. Define a constant at the top of the file: `const SAH_CONFIG_DIR: &str = ".sah";` and replace both occurrences with references to this constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:212` — ToolConfigWatcher is a public struct with no trait derives. It should implement Debug (required for all public types per Rust conventions), Clone (all fields are Clone-able), PartialEq, and Eq (all fields support these). Downstream crates cannot add these due to orphan rules. Add #[derive(Debug, Clone, PartialEq, Eq)] before the struct definition at line 212.

## Review Findings (2026-08-04 11:54)

Scope: `review sha HEAD~1..HEAD` (commit 9b2e89707). The finding against `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:489` targets `test_watcher_detects_file_change`, a test unmodified by this checkpoint — dropped per the review skill's test-refactor exception. Findings against production code are kept in full.

- [x] `crates/swissarmyhammer-tools/src/health_registry.rs:138` — Duplicates the directory-walk-and-markdown-filter pattern from count_markdown_files() instead of extracting a shared helper; the core iteration logic (lines 143–147) is identical to lines 170–174. Extract a helper like iter_markdown_files(path: &Path) that yields DirEntry for all markdown files; have count_markdown_files() call .count() on it and collect_yaml_errors_from_dir() use it in its filter_map.
- [x] `crates/swissarmyhammer-tools/src/health_registry.rs:147` — The extension check `== Some("md")` is case-sensitive, matching only lowercase — a file named `prompt.MD` or `prompt.Md` won't be collected for YAML frontmatter validation, silently excluding uppercase extensions from health checks. Either normalize the extension to lowercase before comparing (e.g., `s.to_lowercase() == "md"`), or add one test that creates a `.MD` file and verifies it is correctly found and validated.
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:157` — Path construction logic is duplicated between `global_config_path` (line 157) and `project_config_path` (line 165). Both functions differ only in which directory source is used (`dirs::home_dir()` vs `find_git_repository_root()`), then both apply the identical transformation: `.map(|dir| dir.join(SAH_CONFIG_DIR).join(TOOLS_CONFIG_FILENAME))`. This is one operation with an argument waiting to be extracted. Extract a shared helper function `resolve_config_path<F: FnOnce() -> Option<PathBuf>>(dir_source: F) -> Option<PathBuf>` that encapsulates the path construction. Both `global_config_path` and `project_config_path` become single-line callers: `resolve_config_path(dirs::home_dir)` and `resolve_config_path(find_git_repository_root)` respectively.
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:180` — Config loading and merging logic is duplicated in `load_merged_tool_config`. Lines 180–184 (global layer) and 187–191 (project layer) are near-verbatim blocks that differ only in variable names and which path source is used. Both follow the identical pattern: get path, load config, merge. This is one operation with an argument waiting to be extracted. Extract a shared helper function `load_and_merge_layer<F>(config: &mut ToolConfig, get_path: F)` where `F: FnOnce() -> Option<PathBuf>`. Call it twice: `load_and_merge_layer(&mut config, global_config_path)` and `load_and_merge_layer(&mut config, project_config_path)`. Eliminates the nested-if boilerplate and ensures both layers follow identical merge semantics.
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:226` — Public method `fn default()` lacks a doc comment; the documentation rule requires all public items to be documented. Add a doc comment to the method, e.g., `/// Create a new watcher resolving config paths from the current environment.` (or ``/// Equivalent to [`Self::new`].`` if preferring to reference the documented `new` method).

## Review Findings (2026-08-04 12:18)

Scope: `review sha HEAD~1..HEAD` (commit 06fc53d37).

- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:305` — Function parameters accept `&Option<PathBuf>` containing concrete PathBuf types, violating the principle "accept generics, not concrete types" (rule example: prefer `AsRef<Path>` over `&PathBuf`). Should accept more generic `Option<&Path>` instead. Change function signature to accept `Option<&Path>` instead of `&Option<PathBuf>`. Update call sites at lines 249 and 266 to use `self.global_path.as_ref()` and `self.project_path.as_ref()`. #bug #shelltool #test-hygiene #tools