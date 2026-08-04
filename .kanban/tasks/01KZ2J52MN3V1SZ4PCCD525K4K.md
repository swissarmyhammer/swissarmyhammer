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
position_column: doing
position_ordinal: '8380'
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

- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:34` — Function `repo_root()` reimplements the same logic that already exists in `apps/kanban-cli/tests/build_artifacts.rs:17`. Both functions are identical: they use `CARGO_MANIFEST_DIR`, call `.parent()` twice, and return a `PathBuf` to the workspace root. This duplicate should be unified rather than reimplemented. Extract `repo_root()` to a shared test utility module (e.g., `crates/swissarmyhammer-skills/tests/common/mod.rs`) so both test files can reuse it, or reference the existing implementation in kanban-cli as a pattern. -> Fixed: added `pub fn workspace_root_from_manifest_dir(manifest_dir: &str) -> PathBuf` to `crates/swissarmyhammer-common/src/test_utils.rs` (a module already `pub mod`-exported unconditionally and already a regular dependency of both `swissarmyhammer-skills` and `kanban-cli` — the reachable shared test-util location). Both `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs` and `apps/kanban-cli/tests/build_artifacts.rs` now call it instead of each defining their own copy. Added unit tests for the new helper.
- [x] `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs:55` — `.expect()` panics on expected failure modes; process spawn errors (missing git, permission denied) are environmental failures, not internal invariants. Change test signature to `fn no_generated_skill_deploy_artifacts_tracked_under_apps() -> Result<(), Box<dyn std::error::Error>>` and use `?` operator instead of `.expect()`. -> Fixed using the pattern already established elsewhere in this codebase for spawning git in tests (`crates/swissarmyhammer-tools/tests/git_tool_integration_test.rs`, `git_diff_integration_test.rs`): `.output().unwrap_or_else(|e| panic!("git ls-files failed to spawn: {e}"))` instead of `.expect(...)`.
#bug #cleanup #docs #skills