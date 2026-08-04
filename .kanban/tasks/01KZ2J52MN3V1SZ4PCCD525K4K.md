---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6yv65wnvcfke1rz2v8zs72
  text: |-
    ### implement — changed
    - evidence: Investigated whether `.skills/` is read at runtime before deciding (per instructions): `SkillResolver::new()` (crates/swissarmyhammer-skills/src/skill_resolver.rs) resolves project-local skills only from `{git_root}/.skills`; `git_root` for `apps/kanban-cli/` and `apps/code-context-cli/` is this repo's root, never those subdirectories. Confirmed via `git ls-files -s` that the three named `.skills/*/SKILL.md` files are mode 100644 real files, plus six more tracked mode-120000 symlinks (`apps/kanban-cli/.claude/skills/kanban`, `apps/kanban-cli/.zed/skills/kanban`, `apps/code-context-cli/.claude/skills/{code-context,lsp}`, `apps/code-context-cli/.zed/skills/{code-context,lsp}`) all pointing at `../../.skills/<name>` — the full output of someone running `kanban init`/`code-context skill` directly inside those source dirs and committing the result. Decided: untrack (not regenerate+guard), since these are dead artifacts with zero runtime purpose, not just drifted ones.
      - `git rm` all 9 paths (3 files + 6 symlinks).
      - `.gitignore`: widened `/.skills/`, `/.agents/`, `/.claude/skills/`, `/.claude/agents/`, `/.zed/skills/` (repo-root-anchored) to `.skills/`, `.agents/`, `**/.claude/skills/`, `**/.claude/agents/`, `**/.zed/skills/` (recursive), matching the existing `**/.validators/` precedent already in the file, so a local deploy run inside any `apps/*-cli/` dir is ignored instead of accidentally `git add`-able.
      - Added `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`: greps `git ls-files apps` for these path shapes. Verified RED (test failed listing all 9 paths) against the pre-removal tree, then GREEN after `git rm`.
      - Left a comment on ^qg2h0ta (the related "regenerate the llama-agent wording" card) noting it's superseded by this decision.
      - `cargo nextest run -p swissarmyhammer-skills`: 126 passed, 0 skipped.
      - `cargo nextest run -p kanban-cli -p code-context-cli -p mirdan`: 570 passed, 0 skipped.
      - `cargo nextest run --workspace`: 13499 passed, 0 skipped.
      - `cargo clippy -p swissarmyhammer-skills --tests --all-targets`: clean.
    - next: ready for review
    task: ^d525k4k
  timestamp: 2026-08-04T17:58:48.252460+00:00
- actor: claude-code
  id: 01kz6z172xc84adp63gmf03a1w
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13499 tests run: 13499 passed, 0 skipped (114.079s). `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, no warnings.
    - confirmed: new regression test `swissarmyhammer-skills::no_committed_skill_deploy_artifacts no_generated_skill_deploy_artifacts_tracked_under_apps` ran and passed.
    - next: none. Build is clean.
  timestamp: 2026-08-04T18:02:05.789932+00:00
- actor: claude-code
  id: 01kz6z2mcb972gy8g2c2ycy3td
  text: |-
    ### commit — changed
    - evidence: 99aaa65ef fix(skills): untrack drifted .skills/ deploy artifacts under apps/
    - next: none — ready for review
  timestamp: 2026-08-04T18:02:52.171729+00:00
- actor: claude-code
  id: 01kz6zcbsb7819ktbb3xy56c9q
  text: |-
    ### review — findings
    - evidence: 2 findings — crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:34, crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:55
    - scope: review sha HEAD~1..HEAD (commit 99aaa65ef)
    - next: fix the duplicate `repo_root()` and the `.expect()` on process spawn, then re-review
  timestamp: 2026-08-04T18:08:11.051432+00:00
- actor: claude-code
  id: 01kz6zctdfrnhpdwap993z2vb7
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — untracked 9 stale deploy artifacts (apps/kanban-cli, apps/code-context-cli), widened .gitignore, added crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs
    - test: green — cargo nextest run --workspace, 13499 passed; clippy clean
    - commit: 99aaa65ef
    - review: findings — crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:34,55
  timestamp: 2026-08-04T18:08:26.031717+00:00
- actor: claude-code
  id: 01kz6zzy90e2s33h1wbhgtstdt
  text: |-
    ### implement — changed
    - evidence: Fixed both iteration-1 review findings.
      - Finding 1 (repo_root() duplication): investigated reachability — swissarmyhammer-skills and apps/kanban-cli are separate crates whose `tests/*.rs` files compile as separate binaries, so a private fn in one is not reachable from the other; no shared test-util crate existed. Found `crates/swissarmyhammer-common/src/test_utils.rs` (module `pub mod test_utils;`, unconditionally exported, already a regular dependency of both crates) as the correct reachable location. Added `pub fn workspace_root_from_manifest_dir(manifest_dir: &str) -> PathBuf` there (with unit tests). Updated `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs` and `apps/kanban-cli/tests/build_artifacts.rs` to both call the shared helper instead of each defining its own `repo_root()`.
      - Finding 2 (`.expect()` on git spawn): checked existing codebase patterns for spawning git in tests (`crates/swissarmyhammer-tools/tests/git_tool_integration_test.rs`, `git_diff_integration_test.rs`) — both use `.output().unwrap_or_else(|e| panic!("git {:?} failed: {e}", args))`. Matched that pattern: `.output().unwrap_or_else(|e| panic!("git ls-files failed to spawn: {e}"))`.
      - Flipped both `## Review Findings` checklist items to `- [x]` on the task description with a note on how each was fixed.
      - Note: an earlier `update task` call of mine double-escaped the description (copied literal `\n` text from a prior JSON tool response instead of real newlines), which corrupted formatting and reset `tags`/`progress` to empty/0. Caught it immediately via a follow-up `get task` and re-issued the update with real newlines; description, tags (bug/cleanup/docs/skills), and progress (1.0) are now correct.
    - tests: `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-skills -p kanban-cli` — 833 passed, 0 skipped. `cargo nextest run -p kanban-cli --test build_artifacts` — 3 passed. `cargo clippy -p swissarmyhammer-skills -p swissarmyhammer-common -p kanban-cli --tests --all-targets -- -D warnings` — clean. `cargo nextest run --workspace` — 13500/13501 passed (1 failure: `swissarmyhammer-kanban::search_tasks_e2e qwen_embedding_search_tasks_real_pipeline_e2e`, unrelated to this change — re-ran in isolation and it passed, confirming pre-existing flakiness, not a regression from these edits).
    - files changed: crates/swissarmyhammer-common/src/test_utils.rs, crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs, apps/kanban-cli/tests/build_artifacts.rs
    - next: ready for review
    task: ^d525k4k
  timestamp: 2026-08-04T18:18:52.576124+00:00
- actor: claude-code
  id: 01kz70b9qzmqvrw9g9mqh8n8y9
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13501 passed, 0 failed, 0 skipped (3 slow, 1 leaky, informational only); cargo clippy --workspace --all-targets --all-features -- -D warnings — clean, 0 warnings, 0 errors across 66 workspace members
    - targeted checks: test_workspace_root_from_manifest_dir, test_workspace_root_from_manifest_dir_panics_when_too_shallow (swissarmyhammer-common), no_generated_skill_deploy_artifacts_tracked_under_apps (swissarmyhammer-skills), generated_manpage_exists, generated_markdown_reference_exists, generated_shell_completions_exist (kanban-cli) — all passed
    - qwen_embedding_search_tasks_real_pipeline_e2e passed cleanly on the first run (2.164s), no isolation retry needed
    - zero #[ignore] attributes found workspace-wide
    - no fixes required, no files modified
    - next: ready for review
  timestamp: 2026-08-04T18:25:04.767815+00:00
- actor: claude-code
  id: 01kz70cnxjjycm5z71a34g2t0g
  text: |-
    ### commit — changed
    - evidence: 399fc7921 refactor(test-utils): share workspace-root helper, drop .expect() on git spawn
    - next: none
  timestamp: 2026-08-04T18:25:50.002310+00:00
- actor: claude-code
  id: 01kz70z0yv39p41rtra5cj1y4g
  text: |-
    ### review — findings
    - evidence: 13 findings, all confirmed — crates/swissarmyhammer-common/src/test_utils.rs:73,75,80,220,229,232,372,430,433,435,564,566,787
    - scope: review sha HEAD~1..HEAD (checkpoint 399fc7921)
    - next: address each finding, then re-run /review d525k4k
  timestamp: 2026-08-04T18:35:51.131085+00:00
- actor: claude-code
  id: 01kz70zdc0j5hw5af25ccmj9vp
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — extracted workspace_root_from_manifest_dir shared helper in test_utils.rs, fixed .expect() on git spawn
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 399fc7921
    - review: findings — crates/swissarmyhammer-common/src/test_utils.rs:73,75,80,220,229,232,372,430,433,435,564,566,787
  timestamp: 2026-08-04T18:36:03.840983+00:00
- actor: claude-code
  id: 01kz71chw7y3wj8w3r4atcjm5t
  text: |-
    ### implement — changed
    - evidence: Fixed all 13 review findings in crates/swissarmyhammer-common/src/test_utils.rs:
      1-2. CurrentDirGuard doc example rewritten to use `fn main() -> std::io::Result<()> { ... ? ... Ok(()) }` instead of `.unwrap()`; also fixed the same class of issue in the ProcessGuard doc example (unlisted but same root cause).
      3. Added `#[derive(Debug)]` to `CurrentDirGuard`.
      4-13. Extracted named constants for every distinct magic-number purpose: `PROCESS_POLL_INTERVAL_MS`, `PROCESS_KILL_TIMEOUT_SECS`, `PROCESS_GRACEFUL_TERMINATION_TIMEOUT_MS` (unlisted, same class, found on re-scan), `MAX_RETRY_ATTEMPTS`, `DIR_RETRY_BACKOFF_BASE_MS`, and a test-scoped `TEST_PROCESS_COMPLETION_WAIT_MS`. Deduplicated `ProcessGuard::terminate_gracefully`'s post-kill wait and `force_kill`'s identical wait loop (same 1s-timeout/10ms-poll code, only :229/:232 were listed but :245/:248 in `force_kill` had the identical pattern) into one private `wait_for_exit` helper.
      4. `IsolatedTestHome::new()` now calls `acquire_home_env_lock()` instead of re-implementing its poisoned-lock recovery logic.
    - Re-scanned the whole file for the same magic-number/duplication class per the task instruction; found and fixed the `force_kill` duplicate loop and the `500`ms Drop-impl literal beyond the 13 listed findings.
    - Verified: `cargo build -p swissarmyhammer-common` (clean), `cargo clippy -p swissarmyhammer-common --lib --tests -- -D warnings` (zero warnings), `cargo test -p swissarmyhammer-common` (603 lib tests passed, 23 doc tests passed including the `CurrentDirGuard` doctest which executes, not just compiles), `mcp__sah__diagnostics check file` on test_utils.rs (0 errors, 0 warnings).
    - All 13 checklist items flipped to `- [x]` on the task description with per-finding fix notes.
    - Task left in `doing` column, ready for `/review`.

    step: implement
    outcome: changed
    evidence: crates/swissarmyhammer-common/src/test_utils.rs — 13 findings fixed, plus 2 additional same-class fixes found on re-scan (force_kill duplicate wait loop, 500ms Drop-impl literal)
    task: ^d525k4k
  timestamp: 2026-08-04T18:43:14.439414+00:00
- actor: claude-code
  id: 01kz71jw5bmmzjfv62y4ax7p20
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run: 13501 passed, 0 failed, 0 skipped (118.830s); `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - next: no fixes needed. Ready to move forward.
  timestamp: 2026-08-04T18:46:41.579726+00:00
- actor: claude-code
  id: 01kz71ma3gzyytkpyyxzafz1fq
  text: |-
    ### commit — changed
    - evidence: 9f612ea98 fix(swissarmyhammer-common): round-2 review cleanup for test_utils
    - next: proceed to review
  timestamp: 2026-08-04T18:47:28.624968+00:00
- actor: claude-code
  id: 01kz72b6c40ghq96q8n2rjjb85
  text: '### review — findings\n- evidence: 13 findings — test_utils.rs:140,219,244,263,280,312,362,368,445,458,587,591,592 (scope: HEAD~1..HEAD, commit 9f612ea98). 2 additional engine findings dropped under the never-refactor-existing-tests rule (test_utils.rs:673, test_utils.rs:849 — pre-existing test code untouched by this commit).\n- next: fix the 13 findings, commit, re-review.'
  timestamp: 2026-08-04T18:59:58.468028+00:00
- actor: claude-code
  id: 01kz72bzq3p3ngq413qhe6x7xy
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — named constants for magic numbers, Debug on CurrentDirGuard, doc-comment fixes, deduped lock-acquisition and wait-for-exit logic in test_utils.rs
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 9f612ea98
    - review: findings — crates/swissarmyhammer-common/src/test_utils.rs:140,219,244,263,280,312,362,368,445,458,587,591,592 (whole-file review boundary continuing to surface pre-existing debt in this large support file; no finding repeats a prior round's file:line)
  timestamp: 2026-08-04T19:00:24.419365+00:00
- actor: claude-code
  id: 01kz73jed32vgycrdznzg0rk3g
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-common/src/test_utils.rs (lock_or_recover + retry_with_backoff helpers extracted; ProcessGuard/IsolatedTestEnvironment/IsolatedTestHome now derive Debug; new ProcessGuardError via thiserror replaces Box<dyn Error> on terminate_gracefully/force_kill; wait_for_exit rewritten on the wait-timeout crate's ChildExt instead of a manual poll loop; ensure_subdir 4x duplication folded into a loop; create_temp_dir documents # Panics and shares retry_with_backoff), Cargo.toml (added wait-timeout = "0.2" to [workspace.dependencies]), crates/swissarmyhammer-common/Cargo.toml (added wait-timeout.workspace = true as a direct dependency, not dev-only, since test_utils is a plain pub module)
    - verification: `cargo check -p swissarmyhammer-common --lib`/`--tests` clean, `cargo check --workspace --tests` clean (zero warnings), `cargo nextest run -p swissarmyhammer-common` 613 passed, `cargo nextest run -p swissarmyhammer -p swissarmyhammer-skills` 209 passed, `cargo test -p swissarmyhammer-common --doc` 23 passed/4 ignored/0 failed
    - note: finding #3 literally claimed `std::process::Child::wait_timeout()` exists in std; confirmed by a standalone rustc compile that it does not (E0599). Satisfied the finding's actual intent (stop hand-rolling the poll loop) via the `wait-timeout` crate's `ChildExt::wait_timeout`, which is already a transitive dependency (assert_cmd) and provides the exact method signature the finding called for as an extension trait on `std::process::Child`. Not treated as a true conflict since a compiling path to the same outcome existed.
    - all 13 findings from the 2026-08-04 13:47 review flipped to [x] on the task description with real newlines; tags confirmed intact (bug, cleanup, docs, skills)
    task: ^d525k4k
  timestamp: 2026-08-04T19:21:24.643517+00:00
- actor: claude-code
  id: 01kz73xh3027t6bvze2sdqt1mc
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 passed, 0 failed, 0 skipped (4 slow, 1 leaky); `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, exit 0
    - checked: no downstream callers of `ProcessGuard::terminate_gracefully`/`force_kill` pattern-match on the old `Box<dyn std::error::Error>` type. `apps/swissarmyhammer-cli/tests/integration/sah_serve_tools_validation.rs` and `sah_serve_integration.rs` only call `ProcessGuard::is_running()`, unaffected by the `ProcessGuardError` change.
    - next: none
  timestamp: 2026-08-04T19:27:27.840777+00:00
- actor: claude-code
  id: 01kz73yxat2r36etzz7bj2g6wn
  text: |-
    ### commit — changed
    - evidence: 2622d5077 fix(swissarmyhammer-common): round-3 review fixes for test_utils
    - next: not pushed; local commit only
  timestamp: 2026-08-04T19:28:13.146706+00:00
- actor: claude-code
  id: 01kz74hvhystcq3et644hc112d
  text: |-
    ### review — findings
    - evidence: 4 findings (of 6 confirmed; 2 dropped as pre-existing untouched test code) — crates/swissarmyhammer-common/src/test_utils.rs:379, :387, :423, :602
    - next: fix panics-on-expected-failure at test_utils.rs:379,387,602 and the retry-semantics gap at test_utils.rs:423, then re-review
  timestamp: 2026-08-04T19:38:33.918815+00:00
- actor: claude-code
  id: 01kz74jy5acqvmy95j8b3h9fax
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — lock_or_recover helper, ProcessGuard Debug derive, wait-timeout crate replacing manual poll loop, ProcessGuardError via thiserror, ensure_subdir loop, Debug on IsolatedTestEnvironment/IsolatedTestHome, retry_with_backoff helper, # Panics doc on create_temp_dir
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 2622d5077
    - review: findings — crates/swissarmyhammer-common/src/test_utils.rs:379,387,423,602 (expect()/panic on expected I/O failure instead of Result; IsolatedTestHome::new() not retry-wrapped)
  timestamp: 2026-08-04T19:39:09.354984+00:00
- actor: claude-code
  id: 01kz7542qys6h0qtyaw944gnjd
  text: |-
    ### implement — changed
    - evidence: Fixed all 4 findings from the 2026-08-04 14:28 review round, all in `crates/swissarmyhammer-common/src/test_utils.rs`:
      1. (line ~379) `create_isolated_test_home()` now returns `std::io::Result<(TempDir, PathBuf)>`; the `SwissarmyhammerDirectory::from_custom_root(..)` call uses `.map_err(std::io::Error::other)?` instead of `.expect(..)`.
      2. (line ~387) same function's `ensure_subdir` loop uses `.map_err(std::io::Error::other)?` instead of `.unwrap_or_else(|e| panic!(...))`.
      3. (line ~423) split `IsolatedTestHome::new()` into `pub fn new() -> Self` (retries via `retry_with_backoff(Self::try_new)`, panics only after `MAX_RETRY_ATTEMPTS` exhausted — used by `Default::default()` and the 4 direct test call sites) and `fn try_new() -> std::io::Result<Self>` (single attempt, no retry, no panic). `IsolatedTestEnvironment::try_create()` now calls `IsolatedTestHome::try_new()?` directly instead of `IsolatedTestHome::new()`, so its single fallible attempt is retried exactly once by the existing outer `retry_with_backoff(Self::try_create)` — this avoids a 3×3 double-retry that would have resulted from calling the panicking `new()` from inside an already-retried `try_create()`. Both the direct-instantiation path and the `IsolatedTestEnvironment` path now retry the identical number of times with identical backoff.
      4. (line ~602) `create_temp_dir()` now returns `std::io::Result<TempDir>` (`retry_with_backoff(TempDir::new)`, no panic). Verified no external call sites exist anywhere else in the workspace (only this file, plus an unrelated same-named local function in `apps/swissarmyhammer-cli/tests/test_utils.rs`). Updated the 5 in-file call sites (`create_isolated_test_home` + 4 `#[cfg(test)]` tests) to propagate/`.unwrap()`.
    - Verification: `cargo check -p swissarmyhammer-common --lib --tests` clean, zero warnings; `cargo check --workspace --tests` clean, zero warnings; `cargo nextest run -p swissarmyhammer-common` -> 613 passed, 0 skipped; `cargo test -p swissarmyhammer-common --doc` -> 23 passed, 4 ignored, 0 failed; `cargo nextest run -p swissarmyhammer -p swissarmyhammer-skills` -> 209 passed, 0 skipped; `cargo nextest run --workspace` -> 13501 passed, 0 skipped.
    - Description updated: all 4 finding checkboxes flipped to `- [x]` with fix explanations appended, using real newlines (verified via `get task` and by reading the raw `.kanban/tasks/01KZ2J52MN3V1SZ4PCCD525K4K.md` on disk — no literal `\n` corruption, tags `bug`/`cleanup`/`docs`/`skills` and progress 1.0 intact).
    - Not a full plateau: `IsolatedTestHome::swissarmyhammer_dir()` (private struct, ~line 475) still has `.expect("Failed to get swissarmyhammer directory")` — the same panic-on-expected-I/O-failure pattern as the 4 findings just fixed, and it was never flagged across 5 review rounds despite exhaustive scans. Left alone deliberately: fixing it forces `IsolatedTestEnvironment::swissarmyhammer_dir()` (the public wrapper) to also become fallible, and that getter is called without `.unwrap()` in at least 5 other files across 4 other crates (`apps/swissarmyhammer-cli/tests/test_utils.rs`, `crates/swissarmyhammer/tests/test_home_integration.rs`, `crates/swissarmyhammer-config/tests/integration/{fresh_loadings,precedences,integrations}.rs`) — a materially larger, cross-crate blast radius than anything in this round's 4 findings, which were all confined to a single file. If a future round flags it, it is a legitimate new finding, not a re-litigation — I did not fix it here only because it exceeds what this round asked for and would require touching 4+ other crates.
    - next: ready for review
    task: ^d525k4k
  timestamp: 2026-08-04T19:48:31.102222+00:00
- actor: claude-code
  id: 01kz759b9zndtb7w9bhd01sxvf
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run, 13501 passed, 0 failed, 0 skipped (114.276s). `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, no warnings.
    - next: ready for review.
  timestamp: 2026-08-04T19:51:23.711916+00:00
- actor: claude-code
  id: 01kz75ahwbkeq6np1at442k2ds
  text: |-
    ### commit — changed
    - evidence: 68b88e123 fix(swissarmyhammer-common): return io::Result from test_utils temp-dir helpers
    - next: none
  timestamp: 2026-08-04T19:52:03.211693+00:00
- actor: claude-code
  id: 01kz764qkt3503egw35cdyf708
  text: |-
    ### review — findings
    - evidence: 4 findings — crates/swissarmyhammer-common/src/test_utils.rs:7, crates/swissarmyhammer-common/src/test_utils.rs:167, crates/swissarmyhammer-common/src/test_utils.rs:302, crates/swissarmyhammer-common/src/test_utils.rs:450
    - scope: review sha HEAD~1..HEAD (commit 68b88e123)
    - dropped: test_utils.rs:702 (hardcoded thread count 5 in test_concurrent_access) — never-refactor-existing-tests rule, pre-existing test code untouched by this commit
    - next: task stays in review; a fix pass must address the 4 open findings
  timestamp: 2026-08-04T20:06:21.050079+00:00
- actor: claude-code
  id: 01kz765h0tffb808x3p03hbd6v
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — create_isolated_test_home/create_temp_dir now return io::Result instead of panicking; IsolatedTestHome::new()/try_new() split for retry semantics
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 68b88e123
    - review: findings — crates/swissarmyhammer-common/src/test_utils.rs:7 (broken rustdoc code fence), :167 (duplicated dir-exists check), :302 (duplicated kill-and-wait sequence), :450 (nested retry bug: try_new() indirectly retries via create_temp_dir causing 3x3=9 attempts instead of 3)
  timestamp: 2026-08-04T20:06:47.066058+00:00
- actor: claude-code
  id: 01kz76p2v77231jjb26394m4ec
  text: |-
    ### implement — changed

    Fixed all 4 review findings from the 2026-08-04 14:52 round, all in `crates/swissarmyhammer-common/src/test_utils.rs`:

    1. Module-level doc had an unclosed code fence and no example. Replaced with a closed `# Examples` block using `IsolatedTestEnvironment::new()?`. Verified: the module-level doctest now compiles and runs (`cargo test -p swissarmyhammer-common --doc`).
    2. Extracted `fn check_dir_exists(path: &Path, msg: impl Into<String>) -> std::io::Result<()>` and switched both existence checks in `CurrentDirGuard::new` (pre-lock and post-lock) to call it.
    3. Extracted `fn kill_and_wait(&mut self) -> Result<(), ProcessGuardError>` on `ProcessGuard`; `terminate_gracefully` calls it after its graceful wait times out, `force_kill` is now a one-line call to it.
    4. Real bug: `create_isolated_test_home()` now calls `TempDir::new()?` directly instead of `create_temp_dir()?`, so `try_new()` is truly single-attempt as documented, eliminating the nested 3×3=9-attempt retry. `create_temp_dir()` remains a public utility with its own internal retry for direct/test callers, just no longer called from inside a `try_*` function that already sits inside an outer retry wrapper.

    Also did a full whole-file re-read of the retry/lock/panic control flow afterward (not just grepping for magic numbers), specifically hunting for the same nested-retry-inside-retry class as finding 4. Confirmed: the only two retry wrappers are `IsolatedTestHome::new()` (wraps now-single-attempt `try_new`) and `IsolatedTestEnvironment::new()` (wraps `try_create`, which calls `IsolatedTestHome::try_new()` directly, not `new()`, avoiding double-wrapping). `create_temp_dir()`'s own retry is only reached from `#[cfg(test)]` tests now, never from inside another retry wrapper. No further nested-retry instances found.

    One self-inflicted issue caught and fixed during this pass: my first edit for finding 2 accidentally inserted `check_dir_exists`'s function body between `retry_with_backoff`'s doc comment and its `fn` signature, orphaning both doc comments (a rustdoc attachment bug, not a compile error). Caught it on re-read and reordered so each function keeps its own doc comment.

    Also caught and fixed a self-inflicted kanban tagging issue: the description text originally read "finding #4 above", which the kanban tool parsed as a `#4` tag marker, adding a spurious "4" tag to the task. Removed the `#` before "4" and reset tags explicitly back to `bug, cleanup, docs, skills` via `update task`.

    Verification: `cargo check -p swissarmyhammer-common --lib --tests` clean, zero warnings; `cargo check --workspace --tests` clean, zero warnings; `cargo test -p swissarmyhammer-common --doc` -> 23 passed, 4 ignored, 0 failed; `cargo nextest run -p swissarmyhammer-common` -> 613 passed, 0 skipped; `cargo nextest run --workspace` -> 13501 passed, 0 skipped.

    - evidence: crates/swissarmyhammer-common/src/test_utils.rs (module doc, check_dir_exists, kill_and_wait, create_isolated_test_home)
    - next: ready for another review pass; task left in `doing` per process (not moved to review by this step)

    step: implement
    outcome: changed
    evidence: 1 file — crates/swissarmyhammer-common/src/test_utils.rs
    task: ^d525k4k
  timestamp: 2026-08-04T20:15:49.607310+00:00
- actor: claude-code
  id: 01kz76vf6zc5vc8zpa5avan8yh
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13501 tests run: 13501 passed, 0 failed, 0 skipped (3 slow, 1 leaky); cargo clippy --workspace --all-targets --all-features -- -D warnings — clean, exit 0
    - next: none
  timestamp: 2026-08-04T20:18:46.111125+00:00
- actor: claude-code
  id: 01kz76x66t1210pz0cfqn89yn7
  text: |-
    ### commit — changed
    - evidence: 31bdff42d fix(swissarmyhammer-common): close rustdoc fence, extract helpers, fix nested-retry bug
    - next: ready for review

    step: commit
    outcome: changed
    evidence: 31bdff42d fix(swissarmyhammer-common): close rustdoc fence, extract helpers, fix nested-retry bug
    task: ^d525k4k
  timestamp: 2026-08-04T20:19:42.426316+00:00
- actor: claude-code
  id: 01kz773ae961gkr7afk7ccw25v
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit 31bdff42d) -> 1 raw finding, test_utils.rs:738 (`(0..5)` thread count in `test_concurrent_access`), dropped under the never-refactor-existing-tests rule (pre-existing test code, not touched by this commit's diff; same finding already dropped 3x in prior rounds at lines 673/669/702). Net: 0 live findings. All prior dated Review Findings sections fully checked.
    - next: task moved to done.
  timestamp: 2026-08-04T20:23:03.369051+00:00
- actor: claude-code
  id: 01kz773tp8kp0pefsnbet6scc2
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — closed rustdoc fence, extracted check_dir_exists/kill_and_wait helpers, fixed nested-retry bug
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 31bdff42d
    - review: clean — 0 net findings (1 raw finding dropped, pre-existing untouched test code); task moved to done
  timestamp: 2026-08-04T20:23:20.008625+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa780
title: Three committed .skills/ snapshots have drifted from builtin/skills/
---
## What

Three generated `.skills/` files are tracked in git and have drifted far from
their `builtin/skills/` sources:

- `apps/kanban-cli/.skills/kanban/SKILL.md`
- `apps/code-context-cli/.skills/code-context/SKILL.md`
- `apps/code-context-cli/.skills/lsp/SKILL.md`

`diff builtin/skills/kanban/SKILL.md apps/kanban-cli/.skills/kanban/SKILL.md`
reports dozens of differing paragraphs. Neither `apps/kanban-cli/build.rs` nor
`apps/code-context-cli/build.rs` writes them, so nothing regenerates them on a
build. They are a runtime deploy artifact that was committed once and left.

Found while card ^3y5n9g6 deleted the `llama-agent` crate:
`apps/kanban-cli/.skills/kanban/SKILL.md:19` still tells users the kanban board
is "the single source of truth across Claude Code and llama-agent sessions".
The `builtin/skills/kanban/SKILL.md` source was corrected there; the snapshot
could not be, because `.skills/` must never be hand-edited.

Decide and act:

- Regenerate all three through the real deploy path
  (`swissarmyhammer-skills::deploy`), which re-renders each SKILL.md from
  `parse_skill_md` -> `format_skill_md` with template variables resolved — a
  raw copy is NOT equivalent, and

- add a test that fails when a committed snapshot drifts from its source, or

- stop tracking `.skills/` and add it to `.gitignore`.

Pick one. A snapshot that nothing regenerates and nothing checks will drift
again.

## Decision: untrack

Verified before deciding: `SkillResolver` (`crates/swissarmyhammer-skills/src/skill_resolver.rs`)
only resolves project-local skills from `{git_root}/.skills`, and `{git_root}`
for every crate under `apps/*-cli/` in this workspace is the repository root
(`/Users/wballard/github/swissarmyhammer/swissarmyhammer-main`), never the
`apps/kanban-cli/` or `apps/code-context-cli/` subdirectory. So these three
files (plus their `.claude/skills/`, `.zed/skills/` symlink siblings — found
during investigation, six more tracked paths beyond the three named above) are
never read by any running binary. They are pure leftovers from someone running
`kanban init` / `code-context skill` directly inside those source
directories, whose output then got `git add`-ed because the repo-root
`.gitignore` rules (`/.skills/`, `/.claude/skills/`, `/.zed/skills/`, ...) were
anchored to the repo root only and did not cover nested `apps/*` directories.

Untracking is the correct fix, not regeneration: regeneration would keep
committing a runtime deploy artifact with zero runtime purpose, forever
needing a guard test to catch drift. Untracking removes the artifact and the
class of bug at once.

### Subtasks
- [x] Decide: regenerate + guard, or untrack. -> untrack.
- [x] Apply the decision to all three files (plus the six symlink siblings
      found during investigation: `apps/kanban-cli/.claude/skills/kanban`,
      `apps/kanban-cli/.zed/skills/kanban`,
      `apps/code-context-cli/.claude/skills/code-context`,
      `apps/code-context-cli/.claude/skills/lsp`,
      `apps/code-context-cli/.zed/skills/code-context`,
      `apps/code-context-cli/.zed/skills/lsp`).

## Acceptance Criteria
- [x] Either every committed `.skills/*/SKILL.md` matches what the deploy path
      produces from its `builtin/skills/` source, or no `.skills/` file is
      tracked. -> no `.skills/`/`.claude/skills/`/`.zed/skills/` file is
      tracked under `apps/kanban-cli/` or `apps/code-context-cli/` anymore.
- [x] If they stay tracked, a test fails when one drifts. -> N/A (untracked),
      but added `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`,
      which fails if any such artifact is ever re-committed under `apps/`
      (via `git ls-files`, RED verified against the pre-removal tree then
      GREEN after removal). Also widened `.gitignore` patterns
      (`.skills/`, `.agents/`, `**/.claude/skills/`, `**/.claude/agents/`,
      `**/.zed/skills/`) from repo-root-anchored to recursive, so a future
      local deploy run inside any `apps/*-cli/` directory is ignored instead
      of `git add`-able by accident.

## Tests
- [x] Run `cargo nextest run -p swissarmyhammer-skills`. -> 126 passed, 0 skipped.
- [x] Run `cargo nextest run --workspace`. -> 13499 passed, 0 skipped.

Related: ^qg2h0ta ("Regenerate the deployed .skills/ copies so they lose the
llama-agent wording") assumed the regenerate path; superseded by this card's
untrack decision — see comment left there.

## Review Findings (2026-08-04 13:03)

- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:34` — Function `repo_root()` reimplements the same logic that already exists in `apps/kanban-cli/tests/build_artifacts.rs:17`. Both functions are identical: they use `CARGO_MANIFEST_DIR`, call `.parent()` twice, and return a `PathBuf` to the workspace root. This duplicate should be unified rather than reimplemented. Extract `repo_root()` to a shared test utility module, or reference the existing implementation. -> Fixed: added `pub fn workspace_root_from_manifest_dir(manifest_dir: &str) -> PathBuf` to `crates/swissarmyhammer-common/src/test_utils.rs`. Both call sites now use it.
- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:55` — `.expect()` panics on expected failure modes; process spawn errors are environmental failures, not internal invariants. -> Fixed using `.output().unwrap_or_else(|e| panic!("git ls-files failed to spawn: {e}"))`, matching the existing pattern used elsewhere in this codebase.

## Review Findings (2026-08-04 13:26)

- [x] `crates/swissarmyhammer-common/src/test_utils.rs:73` — Example in `CurrentDirGuard` documentation uses `.unwrap()` instead of `?`. -> Fixed: doc example wraps code in `fn main() -> std::io::Result<()> { ... Ok(()) }` and uses `?`. Same fix applied to `ProcessGuard` doc example.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:75` — Same doc-comment issue. -> Fixed as part of the same rewrite above.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:80` — `CurrentDirGuard` lacks `Debug`. -> Fixed: added `#[derive(Debug)]`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:220` — Hardcoded 10ms retry backoff should be a named constant. -> Fixed: extracted `PROCESS_POLL_INTERVAL_MS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:229` — Hardcoded 1s timeout should be a named constant. -> Fixed: extracted `PROCESS_KILL_TIMEOUT_SECS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:232` — Hardcoded 10ms retry backoff should be a named constant. -> Fixed via `PROCESS_POLL_INTERVAL_MS`; deduplicated poll loops into a `wait_for_exit` helper.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:372` — `IsolatedTestHome::new()` duplicates `acquire_home_env_lock()`'s logic. -> Fixed: now calls `acquire_home_env_lock()` directly.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:430` — Hardcoded 3 retry attempts should be a named constant. -> Fixed: extracted `MAX_RETRY_ATTEMPTS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:433` — Same. -> Fixed alongside the above.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:435` — Hardcoded 10ms retry backoff should be a named constant. -> Fixed via `DIR_RETRY_BACKOFF_BASE_MS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:564` — Same 3-attempts magic number. -> Fixed using `MAX_RETRY_ATTEMPTS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:566` — Same 10ms backoff magic number. -> Fixed using `DIR_RETRY_BACKOFF_BASE_MS`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:787` — Hardcoded 100ms test delay should be a named constant. -> Fixed: added `TEST_PROCESS_COMPLETION_WAIT_MS`, replaced all three occurrences.

Re-scanned the whole file for the same class beyond the 13 listed findings and additionally fixed: `ProcessGuard::force_kill`'s identical wait loop, the `500ms` literal in `ProcessGuard`'s `Drop` impl (extracted `PROCESS_GRACEFUL_TERMINATION_TIMEOUT_MS`), and unified the duplicate wait-loop code into a single private `wait_for_exit` helper.

## Review Findings (2026-08-04 13:47)

- [x] `crates/swissarmyhammer-common/src/test_utils.rs:140` — Lock acquisition and poisoning recovery pattern duplicated inline; identical blocks exist at lines 312 and 331 but already extracted into helper functions. Extract a generic lock-acquisition helper parameterized by lock, message, and a callback, or refactor `CurrentDirGuard::new` to call a shared function for this pattern. -> Fixed: added `fn lock_or_recover<T>(lock: &'static Mutex<T>, name: &str) -> MutexGuard<'static, T>` and switched all 3 call sites (`CurrentDirGuard::new`, `acquire_semantic_db_lock`, `acquire_home_env_lock`) to call it.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:219` — `ProcessGuard` is a public type with non-empty representation (wraps `std::process::Child`) but does not implement `Debug`. Add `#[derive(Debug)]`. -> Fixed: `std::process::Child` implements `Debug` (verified by compiling a standalone check), so a plain `#[derive(Debug)]` on `ProcessGuard` compiles with no manual impl needed.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:244` — `wait_for_exit` reimplements process exit-wait-with-timeout logic using a polling loop when the standard library provides `std::process::Child::wait_timeout()`. Replace the custom polling implementation with `self.0.wait_timeout(timeout)`, adjusting call sites for the different return type. -> Verified first: `std::process::Child::wait_timeout()` does NOT exist in std (confirmed by a failing standalone `rustc` compile, error E0599). The intent is satisfied instead through the `wait-timeout` crate (already a transitive dependency in `Cargo.lock` via `assert_cmd`), whose `ChildExt` trait adds exactly this method (`fn wait_timeout(&mut self, dur: Duration) -> io::Result<Option<ExitStatus>>`) directly onto `std::process::Child`. Added `wait-timeout = "0.2"` to the workspace `[workspace.dependencies]` and as a direct dependency of `swissarmyhammer-common`, then rewrote `wait_for_exit` as `Ok(self.0.wait_timeout(timeout)?.is_some())`. The dead `PROCESS_POLL_INTERVAL_MS` constant and its now-unused manual poll loop were removed.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:263` — `ProcessGuard::terminate_gracefully` returns `Box<dyn std::error::Error>`, preventing callers from matching on specific error types. Define a typed error enum via `thiserror` and return `Result<(), ProcessGuardError>`. -> Fixed: added `#[derive(Debug, thiserror::Error)] pub enum ProcessGuardError { #[error("process management I/O error: {0}")] Io(#[from] std::io::Error) }` and changed the return type.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:280` — `ProcessGuard::force_kill` has the same typed-error violation as line 263. Use the same `ProcessGuardError` type. -> Fixed alongside the above; both methods now return `Result<(), ProcessGuardError>`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:312` — Lock acquisition and poisoning recovery pattern duplicated; identical blocks exist at lines 140 and 331. Extract a shared generic lock-acquisition helper to replace all three instances. -> Fixed by the same `lock_or_recover` helper described above.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:362` — Near-identical `ensure_subdir` call pattern repeated 4 times (workflows, todo, issues, issues/complete), differing only by directory name. Extract into a loop over `&["workflows", "todo", "issues", "issues/complete"]`. -> Fixed: replaced the 4 near-identical calls with `for subdir in ["workflows", "todo", "issues", "issues/complete"] { sah_dir.ensure_subdir(subdir).unwrap_or_else(|e| panic!("Failed to create {subdir} directory: {e}")); }`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:368` — Same `ensure_subdir` duplication as line 362 (issues variant). Fold into the same loop extraction. -> Folded into the same loop above.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:445` — `IsolatedTestEnvironment` is a public struct with non-empty representation but does not implement `Debug`. Add `#[derive(Debug)]` (or manually implement if internal types aren't all `Debug`). -> Fixed: added `#[derive(Debug)]` to `IsolatedTestEnvironment`, and to the private `IsolatedTestHome` struct it wraps (all its fields — `TempDir`, `Option<String>`, `MutexGuard<'static, ()>` — are `Debug`).
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:458` — Retry loop with backoff pattern duplicated at lines 458–472 and 592–610, differing only in the operation retried and success/error handling. Extract a generic `retry_with_backoff` helper accepting a closure. -> Fixed: added `fn retry_with_backoff<T, E>(mut operation: impl FnMut() -> Result<T, E>) -> Result<T, E>`. `IsolatedTestEnvironment::new` is now `retry_with_backoff(Self::try_create)`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:587` — `create_temp_dir` panics on exhausted retries with no `# Panics` doc section. Add one, or change the signature to return `Result<TempDir, std::io::Error>`. -> Fixed: added a `# Panics` doc section explaining the panic only occurs after `MAX_RETRY_ATTEMPTS` retries under severe filesystem failure.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:591` — `create_temp_dir()` reimplements the retry-with-backoff pattern already in `IsolatedTestEnvironment::new()`. Extract a shared generic helper both can call. -> Fixed: `create_temp_dir` is now `retry_with_backoff(TempDir::new).unwrap_or_else(|e| panic!(...))`, using the same `retry_with_backoff` helper as `IsolatedTestEnvironment::new`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:592` — Same retry-loop duplication as line 458 (see that finding). Extract a shared helper to replace both instances. -> Fixed by the same `retry_with_backoff` helper described above.

Note: two findings from this scan were dropped under the never-refactor-existing-tests rule (subject was pre-existing test code untouched by this commit): test_utils.rs:673 (magic number 5 in `test_concurrent_access`) and test_utils.rs:849 (magic number 50 in `test_process_guard_terminate_gracefully_timeout_then_kill`).

Re-scanned the whole file once more for the same classes of issue after applying the above (missing `Debug` derives, duplicated retry/lock/poll loops, magic numbers, undocumented panics, `Box<dyn Error>` in public APIs): no further instances found. All lock-acquisition sites, both retry-with-backoff sites, and both remaining process-management methods now share one helper each; every public struct in the file (`CaptureWriter`, `CurrentDirGuard`, `ProcessGuard`, `ProcessGuardError`, `IsolatedTestEnvironment`) derives `Debug`; no remaining `Box<dyn Error>` in this file.

Verification: `cargo check -p swissarmyhammer-common --lib` and `--tests` clean with zero warnings; `cargo check --workspace --tests` clean with zero warnings (confirms downstream consumers of `ProcessGuard`/`terminate_gracefully`/`force_kill` in `apps/swissarmyhammer-cli/tests/` still compile against the new `Result<(), ProcessGuardError>` return type); `cargo nextest run -p swissarmyhammer-common` -> 613 passed, 0 skipped; `cargo nextest run -p swissarmyhammer -p swissarmyhammer-skills` -> 209 passed, 0 skipped; `cargo test -p swissarmyhammer-common --doc` -> 23 passed, 4 ignored, 0 failed.

## Review Findings (2026-08-04 14:28)

- [x] `crates/swissarmyhammer-common/src/test_utils.rs:379` — Panics on expected I/O failure (directory creation). The rule forbids panicking on expected failure modes — only bugs (internal invariant violations) should panic. Return `Result` or propagate the error via `?` instead of panicking via `.expect()`. -> Fixed: `create_isolated_test_home()` now returns `std::io::Result<(TempDir, PathBuf)>` and propagates `SwissarmyhammerDirectory::from_custom_root(..)` failures with `.map_err(std::io::Error::other)?` instead of `.expect(..)`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:387` — Panics on expected I/O failure (directory creation). The rule forbids panicking on expected failure modes — only internal invariant violations should panic. Return `Result` or propagate the error via `?` instead of panicking. -> Fixed: the `ensure_subdir` loop now does `sah_dir.ensure_subdir(subdir).map_err(std::io::Error::other)?` inside the now-`Result`-returning `create_isolated_test_home()`, instead of `.unwrap_or_else(|e| panic!(...))`.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:423` — `create_isolated_test_home()` performs filesystem operations (creating subdirectories at lines 384–388) that the docstring for `retry_with_backoff` identifies as transiently failing under parallel test execution. This function is called from two paths with inconsistent retry semantics: from `IsolatedTestEnvironment::try_create()` (line 484), which is wrapped in `retry_with_backoff` at line 477, and directly from `IsolatedTestHome::new()` (line 423), whose callers in tests (lines 641, 672, 908, 1074) do not retry. Direct test calls to `IsolatedTestHome::new()` will panic on transient failures instead of retrying. Wrap `IsolatedTestHome::new()` in `retry_with_backoff` as well (or refactor to `try_new()` paired with a retry wrapper at call sites), so the same transient-failure scenario is handled consistently whether `create_isolated_test_home()` is reached via `IsolatedTestEnvironment` or direct instantiation. -> Fixed via the `try_new()` option: split `IsolatedTestHome::new()` into `pub fn new() -> Self` (retries via `retry_with_backoff(Self::try_new)`, panics only after `MAX_RETRY_ATTEMPTS` exhausted — used by `Default::default()` and the 4 direct test call sites) and `fn try_new() -> std::io::Result<Self>` (single attempt, no retry, no panic). `IsolatedTestEnvironment::try_create()` now calls `IsolatedTestHome::try_new()?` directly instead of `IsolatedTestHome::new()`, so it stays a single fallible attempt and is retried exactly once by the existing outer `retry_with_backoff(Self::try_create)` in `IsolatedTestEnvironment::new()` — avoiding a double-retry (3×3 attempts) that calling the panicking `new()` from inside `try_create()` would have caused. Both paths now retry the identical number of times with identical backoff.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:602` — Public function panics on expected I/O failure. Library functions should return `Result` for expected failures, not panic. Filesystem exhaustion and permission errors are expected failure modes, not internal invariants. Change function signature to `pub fn create_temp_dir() -> std::io::Result<TempDir>` and return the result instead of panicking. -> Fixed: `create_temp_dir()` now returns `std::io::Result<TempDir>` and is simply `retry_with_backoff(TempDir::new)` (the `Err` case returns the last underlying `io::Error` from `TempDir::new` directly, no panic). Verified there are no external call sites of this function anywhere else in the workspace (only within this file and the CLI's own unrelated locally-defined `create_temp_dir` in `apps/swissarmyhammer-cli/tests/test_utils.rs`, which is a separate function) — updated the 5 in-file call sites (`create_isolated_test_home`, and 4 `#[cfg(test)]` tests) to propagate/`.unwrap()` the new `Result`.

Note: Two findings from this scan are dropped under the never-refactor-existing-tests rule. The subject is pre-existing test code. This commit did not touch it. Dropped items: test_utils.rs:669 (magic number 5 in `test_concurrent_access`) and test_utils.rs:844 (magic number 50 in `test_process_guard_terminate_gracefully_timeout_then_kill`).

Verification: `cargo check -p swissarmyhammer-common --lib --tests` clean, zero warnings; `cargo check --workspace --tests` clean, zero warnings; `cargo nextest run -p swissarmyhammer-common` -> 613 passed, 0 skipped; `cargo test -p swissarmyhammer-common --doc` -> 23 passed, 4 ignored, 0 failed; `cargo nextest run -p swissarmyhammer -p swissarmyhammer-skills` -> 209 passed, 0 skipped; `cargo nextest run --workspace` -> 13501 passed, 0 skipped.

## Review Findings (2026-08-04 14:52)

- [x] `crates/swissarmyhammer-common/src/test_utils.rs:7` — Module-level documentation opens a code block with `/// ``` ` but never closes it, which will cause a rustdoc parsing error. Additionally, the documentation rule requires crate-level docs to include concrete examples showing common use cases, but this module provides only descriptive text without any working code examples. Replace line 7 with a complete code example showing common module usage. -> Fixed: replaced the module doc with a closed code-fence example under a `# Examples` heading, using `IsolatedTestEnvironment::new()?` / `home_path()` / `assert!(home.exists())` inside `fn main() -> std::io::Result<()> { ... Ok(()) }`. Verified with `cargo test -p swissarmyhammer-common --doc`: the module-level doctest now compiles and passes (previously it could not even be collected due to the unclosed fence).
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:167` — Directory existence checks at lines 167-175 and 199-207 are near-identical blocks that differ only in the error message. Both check `!path.exists()`, construct a `NotFound` error with a formatted message, and return early. This is one function parameterized by the message. Extract a helper function: `fn check_dir_exists(path: &Path, msg: impl Into<String>) -> io::Result<()> { if !path.exists() { return Err(io::Error::new(io::ErrorKind::NotFound, msg.into())); } Ok(()) }` and call it at both sites with the appropriate message string. -> Fixed: added `fn check_dir_exists(path: &Path, msg: impl Into<String>) -> std::io::Result<()>` exactly as specified, and switched both existence checks in `CurrentDirGuard::new` (pre-lock and post-lock) to call it with their respective messages.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:302` — The kill-and-wait sequence at lines 302-304 (in `terminate_gracefully`) is verbatim identical to lines 309-311 (in `force_kill`). Both execute `self.0.kill()?; self.wait_for_exit(...).map(|_| ())` with the same timeout constant. Extract a helper method `fn kill_and_wait(&mut self) -> Result<(), ProcessGuardError>` that encapsulates both lines, then call it from both `terminate_gracefully` (after the graceful timeout fails) and `force_kill`. -> Fixed: added `fn kill_and_wait(&mut self) -> Result<(), ProcessGuardError>` doing exactly the kill-then-wait sequence; `terminate_gracefully` now calls it after its graceful wait times out, and `force_kill` is now just a one-line call to it.
- [x] `crates/swissarmyhammer-common/src/test_utils.rs:450` — try_new() is documented (lines 445–446) as 'single attempt, no retry', but the implementation indirectly retries through create_isolated_test_home() (line 455) → create_temp_dir() (line 382) → retry_with_backoff() (line 639). This violates the invariant that try_* functions do not retry internally; retry is delegated to the caller. When IsolatedTestHome::new() (line 439) wraps try_new() in its own retry_with_backoff(), the result is nested retries—up to 3 (internal) × 3 (outer) = 9 total TempDir::new() attempts instead of the documented/intended 3—wasting resources and contradicting the contract. Change create_isolated_test_home() (line 382) from create_temp_dir()? to TempDir::new()?, so that try_new() remains truly single-attempt as documented. Centralize all retry logic in the dedicated wrappers (IsolatedTestHome::new() and IsolatedTestEnvironment::new()). Keep create_temp_dir() as a public utility with internal retry for direct callers, but do not call it from try_* functions where the caller already handles retry. -> Fixed: `create_isolated_test_home()` now calls `TempDir::new()?` directly instead of `create_temp_dir()?`. `try_new()` is now truly single-attempt with no internal retry, matching its doc contract. `create_temp_dir()` is unchanged and remains available as a public utility with its own internal retry for direct/test callers; it is no longer called from any `try_*` function that already sits inside an outer retry wrapper.

Final whole-file re-read of the retry/lock/panic control flow (not just a grep for magic numbers), specifically hunting for the same nested-retry-inside-retry class as finding above: the only two retry wrappers in the file are `IsolatedTestHome::new()` (wraps `try_new`, which is now single-attempt) and `IsolatedTestEnvironment::new()` (wraps `try_create`, which calls `IsolatedTestHome::try_new()` directly -- not `new()` -- so it is not double-wrapped). `create_temp_dir()` keeps its own internal `retry_with_backoff` but is now called only from `#[cfg(test)]` tests directly, never from inside another retry wrapper. `IsolatedTestEnvironment::try_create()`'s own working-directory temp dir uses `TempDir::new()?` directly (not `create_temp_dir()`), so no nesting there either. No other instance of an inner retry wrapper nested inside an outer one was found.

Note: `crates/swissarmyhammer-common/src/test_utils.rs:702` (hardcoded thread count 5 in `test_concurrent_access`) is dropped under the never-refactor-existing-tests rule. The subject is pre-existing test code. This commit did not touch it.

Verification: `cargo check -p swissarmyhammer-common --lib --tests` clean, zero warnings; `cargo check --workspace --tests` clean, zero warnings; `cargo test -p swissarmyhammer-common --doc` -> 23 passed, 4 ignored, 0 failed; `cargo nextest run -p swissarmyhammer-common` -> 613 passed, 0 skipped; `cargo nextest run --workspace` -> 13501 passed, 0 skipped.
#bug #cleanup #docs #skills