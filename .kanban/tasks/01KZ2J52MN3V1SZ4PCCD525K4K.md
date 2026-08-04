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
position_column: doing
position_ordinal: '8380'
title: Three committed .skills/ snapshots have drifted from builtin/skills/
---
## What\n\nThree generated `.skills/` files are tracked in git and have drifted far from\ntheir `builtin/skills/` sources:\n\n- `apps/kanban-cli/.skills/kanban/SKILL.md`\n- `apps/code-context-cli/.skills/code-context/SKILL.md`\n- `apps/code-context-cli/.skills/lsp/SKILL.md`\n\n`diff builtin/skills/kanban/SKILL.md apps/kanban-cli/.skills/kanban/SKILL.md`\nreports dozens of differing paragraphs. Neither `apps/kanban-cli/build.rs` nor\n`apps/code-context-cli/build.rs` writes them, so nothing regenerates them on a\nbuild. They are a runtime deploy artifact that was committed once and left.\n\nFound while card ^3y5n9g6 deleted the `llama-agent` crate:\n`apps/kanban-cli/.skills/kanban/SKILL.md:19` still tells users the kanban board\nis \"the single source of truth across Claude Code and llama-agent sessions\".\nThe `builtin/skills/kanban/SKILL.md` source was corrected there; the snapshot\ncould not be, because `.skills/` must never be hand-edited.\n\nDecide and act:\n\n- Regenerate all three through the real deploy path\n  (`swissarmyhammer-skills::deploy`), which re-renders each SKILL.md from\n  `parse_skill_md` -> `format_skill_md` with template variables resolved — a\n  raw copy is NOT equivalent, and\n\n- add a test that fails when a committed snapshot drifts from its source, or\n\n- stop tracking `.skills/` and add it to `.gitignore`.\n\nPick one. A snapshot that nothing regenerates and nothing checks will drift\nagain.\n\n## Decision: untrack\n\nVerified before deciding: `SkillResolver` (`crates/swissarmyhammer-skills/src/skill_resolver.rs`)\nonly resolves project-local skills from `{git_root}/.skills`, and `{git_root}`\nfor every crate under `apps/*-cli/` in this workspace is the repository root\n(`/Users/wballard/github/swissarmyhammer/swissarmyhammer-main`), never the\n`apps/kanban-cli/` or `apps/code-context-cli/` subdirectory. So these three\nfiles (plus their `.claude/skills/`, `.zed/skills/` symlink siblings — found\nduring investigation, six more tracked paths beyond the three named above) are\nnever read by any running binary. They are pure leftovers from someone running\n`kanban init` / `code-context skill` directly inside those source\ndirectories, whose output then got `git add`-ed because the repo-root\n`.gitignore` rules (`/.skills/`, `/.claude/skills/`, `/.zed/skills/`, ...) were\nanchored to the repo root only and did not cover nested `apps/*` directories.\n\nUntracking is the correct fix, not regeneration: regeneration would keep\ncommitting a runtime deploy artifact with zero runtime purpose, forever\nneeding a guard test to catch drift. Untracking removes the artifact and the\nclass of bug at once.\n\n### Subtasks\n- [x] Decide: regenerate + guard, or untrack. -> untrack.\n- [x] Apply the decision to all three files (plus the six symlink siblings\n      found during investigation: `apps/kanban-cli/.claude/skills/kanban`,\n      `apps/kanban-cli/.zed/skills/kanban`,\n      `apps/code-context-cli/.claude/skills/code-context`,\n      `apps/code-context-cli/.claude/skills/lsp`,\n      `apps/code-context-cli/.zed/skills/code-context`,\n      `apps/code-context-cli/.zed/skills/lsp`).\n\n## Acceptance Criteria\n- [x] Either every committed `.skills/*/SKILL.md` matches what the deploy path\n      produces from its `builtin/skills/` source, or no `.skills/` file is\n      tracked. -> no `.skills/`/`.claude/skills/`/`.zed/skills/` file is\n      tracked under `apps/kanban-cli/` or `apps/code-context-cli/` anymore.\n- [x] If they stay tracked, a test fails when one drifts. -> N/A (untracked),\n      but added `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`,\n      which fails if any such artifact is ever re-committed under `apps/`\n      (via `git ls-files`, RED verified against the pre-removal tree then\n      GREEN after removal). Also widened `.gitignore` patterns\n      (`.skills/`, `.agents/`, `**/.claude/skills/`, `**/.claude/agents/`,\n      `**/.zed/skills/`) from repo-root-anchored to recursive, so a future\n      local deploy run inside any `apps/*-cli/` directory is ignored instead\n      of `git add`-able by accident.\n\n## Tests\n- [x] Run `cargo nextest run -p swissarmyhammer-skills`. -> 126 passed, 0 skipped.\n- [x] Run `cargo nextest run --workspace`. -> 13499 passed, 0 skipped.\n\nRelated: ^qg2h0ta (\"Regenerate the deployed .skills/ copies so they lose the\nllama-agent wording\") assumed the regenerate path; superseded by this card's\nuntrack decision — see comment left there.\n\n## Review Findings (2026-08-04 13:03)\n\n- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:34` — Function `repo_root()` reimplements the same logic that already exists in `apps/kanban-cli/tests/build_artifacts.rs:17`. Both functions are identical: they use `CARGO_MANIFEST_DIR`, call `.parent()` twice, and return a `PathBuf` to the workspace root. This duplicate should be unified rather than reimplemented. Extract `repo_root()` to a shared test utility module (e.g., `crates/swissarmyhammer-skills/tests/common/mod.rs`) so both test files can reuse it, or reference the existing implementation in kanban-cli as a pattern. -> Fixed: added `pub fn workspace_root_from_manifest_dir(manifest_dir: &str) -> PathBuf` to `crates/swissarmyhammer-common/src/test_utils.rs` (a module already `pub mod`-exported unconditionally and already a regular dependency of both `swissarmyhammer-skills` and `kanban-cli` — the reachable shared test-util location). Both `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs` and `apps/kanban-cli/tests/build_artifacts.rs` now call it instead of each defining their own copy. Added unit tests for the new helper.\n- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:55` — `.expect()` panics on expected failure modes; process spawn errors (missing git, permission denied) are environmental failures, not internal invariants. Change test signature to `fn no_generated_skill_deploy_artifacts_tracked_under_apps() -> Result<(), Box<dyn std::error::Error>>` and use `?` operator instead of `.expect()`. -> Fixed using the pattern already established elsewhere in this codebase for spawning git in tests (`crates/swissarmyhammer-tools/tests/git_tool_integration_test.rs`, `git_diff_integration_test.rs`): `.output().unwrap_or_else(|e| panic!(\"git ls-files failed to spawn: {e}\"))` instead of `.expect(...)`.\n#bug #cleanup #docs #skills\n\n## Review Findings (2026-08-04 13:26)\n\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:73` — Example in `CurrentDirGuard` documentation uses `.unwrap()` instead of `?`, teaching bad error-handling patterns. Either restructure the example to use `?` in a function context (e.g., wrapped in `fn main() -> std::io::Result<()> { ... }`), or omit the error case from the example and provide the TempDir by other means. -> Fixed: doc example now wraps the code in `fn main() -> std::io::Result<()> { ... Ok(()) }` and uses `?` for both `TempDir::new()?` and `CurrentDirGuard::new(...)?`. Applied the same fix to the `ProcessGuard` doc example (same class of issue, same file) since it also used `.spawn().unwrap()`.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:75` — Example in `CurrentDirGuard` documentation uses `.unwrap()` instead of `?`, teaching bad error-handling patterns. Either restructure the example to use `?` in a function context, or wrap this in error handling that demonstrates proper patterns. -> Fixed as part of the same doc-comment rewrite above.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:80` — Public struct `CurrentDirGuard` has non-empty representation but does not implement `Debug`, violating the requirement that all public types have `Debug` impls. Add `#[derive(Debug)]` to the struct definition at line 80, or implement `Debug` manually if custom formatting is needed. -> Fixed: added `#[derive(Debug)]` to `CurrentDirGuard`.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:220` — Hardcoded 10 millisecond retry backoff delay should be a named constant. Define `const RETRY_BACKOFF_BASE_MS: u64 = 10;` and use it instead. -> Fixed: extracted `PROCESS_POLL_INTERVAL_MS` (distinct name/purpose from the directory-retry backoff, per instruction not to conflate unrelated numbers under one constant) and used it in the shared `ProcessGuard::wait_for_exit` helper.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:229` — Hardcoded 1 second timeout for process wait should be a named constant. Define `const PROCESS_KILL_TIMEOUT_SECS: u64 = 1;` and use it instead. -> Fixed: extracted `PROCESS_KILL_TIMEOUT_SECS` and used it in `terminate_gracefully`'s post-kill wait and in `force_kill` (both instances of this same magic number, including the one in `force_kill` not explicitly listed as a finding).\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:232` — Hardcoded 10 millisecond retry backoff delay should be a named constant. Define `const RETRY_BACKOFF_BASE_MS: u64 = 10;` and use it instead. -> Fixed via `PROCESS_POLL_INTERVAL_MS` (see above); also deduplicated `terminate_gracefully`'s and `force_kill`'s identical poll loops into one private `wait_for_exit` helper so the poll interval and timeout only appear once each.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:372` — Lines 372–375 duplicate the exact lock acquisition logic of the public `acquire_home_env_lock()` function (lines 299–302). The code inside `IsolatedTestHome::new()` should call that function instead of replicating the implementation, so the acquisition pattern remains synchronized. Replace lines 372–375 with `let lock_guard = acquire_home_env_lock();` and remove the comment; the function already exists and documents the intent. -> Fixed exactly as suggested: `IsolatedTestHome::new()` now calls `acquire_home_env_lock()` instead of re-implementing the poisoned-lock recovery logic.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:430` — Hardcoded 3 retry attempts limit should be a named constant. Define `const MAX_RETRY_ATTEMPTS: u32 = 3;` and use it instead. -> Fixed: extracted `MAX_RETRY_ATTEMPTS` (shared by `IsolatedTestEnvironment::new` and `create_temp_dir`, since both represent the same conceptual retry-on-transient-filesystem-error policy).\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:433` — Hardcoded 3 retry attempts limit should be a named constant. Use named constant `MAX_RETRY_ATTEMPTS` instead of hardcoding 3. -> Fixed alongside the above.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:435` — Hardcoded 10 millisecond retry backoff base should be a named constant. Define `const RETRY_BACKOFF_BASE_MS: u64 = 10;` and use it instead. -> Fixed: extracted `DIR_RETRY_BACKOFF_BASE_MS` (named distinctly from `PROCESS_POLL_INTERVAL_MS` even though both equal 10ms, since they serve different purposes: filesystem-retry backoff multiplier vs. process-exit poll interval).\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:564` — Hardcoded 3 retry attempts limit should be a named constant. Use named constant `MAX_RETRY_ATTEMPTS` instead of hardcoding 3. -> Fixed in `create_temp_dir` using the same `MAX_RETRY_ATTEMPTS` constant.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:566` — Hardcoded 10 millisecond retry backoff base should be a named constant. Define `const RETRY_BACKOFF_BASE_MS: u64 = 10;` and use it instead. -> Fixed in `create_temp_dir` using `DIR_RETRY_BACKOFF_BASE_MS`.\n- [x] `crates/swissarmyhammer-common/src/test_utils.rs:787` — Hardcoded 100 millisecond test delay should be a named constant. Define `const TEST_PROCESS_COMPLETION_WAIT_MS: u64 = 100;` and use it instead. -> Fixed: added `TEST_PROCESS_COMPLETION_WAIT_MS` inside `mod tests` and replaced all three occurrences of the same 100ms test-completion-wait magic number (in `test_process_guard_is_running_finished_process` and both places in `test_process_guard_terminate_gracefully_already_exited`) with it. Left the deliberately-distinct 50ms literal in `test_process_guard_terminate_gracefully_timeout_then_kill` as-is, since it is an intentionally different (too-short) value for that test's specific scenario, not a duplicate of the same constant.\n\nRe-scanned the whole file for the same magic-number/duplication class beyond the 13 listed findings and additionally fixed:\n- `ProcessGuard::force_kill`'s identical 1-second-timeout/10ms-poll wait loop (same class as findings at :229/:232, not explicitly listed) — now shares the `wait_for_exit` helper and named constants.\n- The `500`ms literal in `ProcessGuard`'s `Drop` impl (graceful-termination timeout) — extracted to `PROCESS_GRACEFUL_TERMINATION_TIMEOUT_MS`.\n- The duplicate 1-second-timeout/10ms-poll wait-loop *code* itself (not just its magic numbers) between `terminate_gracefully`'s post-kill wait and `force_kill` — unified into a single private `ProcessGuard::wait_for_exit` helper.\n\nVerified: `cargo build -p swissarmyhammer-common`, `cargo clippy -p swissarmyhammer-common --lib --tests -- -D warnings` (clean), `cargo test -p swissarmyhammer-common` (603 lib tests + 23 doc tests passed, including the `CurrentDirGuard` doctest which actually executes, not just compiles).\n#bug #cleanup #docs #skills