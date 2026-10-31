---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69b48p7q7zw8p0mfe8nkwf4
  text: |-
    ### implement — changed
    - evidence: Removed deny_tool (mirdan strategy and applier), serve-time deny (profiles.rs, mod.rs, handler.rs), tests serve_time_bash_deny.rs and mirdan_test_support.rs. Bash removed from SUPERSEDED_NATIVE_DENY_TOOLS. New LEGACY_BASH_DENY_TOOL: init and deinit remove old Bash deny entries (apply_edit_redirect_at, both directions). Shell tool lifecycle now calls allow_tool on init and deinit. New real-path tests: init_profile_does_not_deny_bash, init_profile_removes_a_legacy_bash_deny, deinit_profile_removes_a_legacy_bash_deny, plus 3 edit_redirect tests. Checks: nextest mirdan 511 pass; swissarmyhammer-tools --lib 1334 pass; swissarmyhammer-cli deinit 16 pass; per_client_tool_composition 2 pass. Used --override-version-check. Tests were written after the code, no RED run.
    - note: ToolRegistry::replacement_natives() has no non-test callers now. Later shell-removal cards can delete it.
    - next: /review
  timestamp: 2026-10-31T16:01:50.279727+00:00
- actor: claude-code
  id: 01m69bc802nxjvrz0a5caa50yk
  text: |-
    ### test — green
    - evidence: cargo check --workspace --tests clean. cargo nextest run --override-version-check -E 'rdeps(mirdan) | rdeps(swissarmyhammer-tools)': 3207 run, 3207 passed, 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: clean. cargo fmt --check: clean.
    - fixes: clippy needless_borrows_for_generic_args at three MirdanConfigGuard::set calls in crates/mirdan/src/install/profile_tests.rs (borrow removed). rustfmt diff in fake_agent_deny in the same file (cargo fmt applied).
    - note: installed nextest is 0.9.116, repo needs 0.9.145, so --override-version-check was used.
    - next: review
  timestamp: 2026-10-31T16:06:11.714251+00:00
- actor: claude-code
  id: 01m69bp3zw93bdtzzwvn13tts0
  text: |-
    ### review — findings
    - evidence: 2 findings — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1538, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1583
    - next: Add a deinit step to each test that re-seeds the legacy Bash deny after init, then assert that deinit removes it.
  timestamp: 2026-10-31T16:11:35.292291+00:00
- actor: claude-code
  id: 01m69bps7jsb184zp0vv5jkgz3
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 17 paths
    - test: green — nextest 3207 passed, clippy and fmt clean
    - commit: c7b2a1a2f
    - review: findings — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1538, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1583
  timestamp: 2026-10-31T16:11:57.042150+00:00
- actor: claude-code
  id: 01m69brnn5t2kc0pysmxtad1q6
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs. Tests test_tool_lifecycle_user_scope and test_tool_lifecycle_local_scope now re-seed the legacy Bash deny after init and before deinit, and assert that deinit removes it. test_tool_lifecycle_project_scope had no deinit step (same gap), so it now runs init, re-seeds, runs deinit, and asserts the removal. Check: cargo nextest run --override-version-check -p swissarmyhammer-tools shell — 197 run, 197 passed.
    - note: No RED run. The tests changed after the production code, so I did not prove that they fail without the cleanup step.
    - next: /review
  timestamp: 2026-10-31T16:12:58.917293+00:00
- actor: claude-code
  id: 01m69bvvsp13ky731abm20t7x9
  text: |-
    ### test — green
    - evidence: cargo nextest run --override-version-check -p swissarmyhammer-tools: 1541 run, 1541 passed, 0 failed, 0 skipped. cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings: 0 warnings. cargo fmt --check: clean.
    - next: review
  timestamp: 2026-10-31T16:14:43.510453+00:00
- actor: claude-code
  id: 01m69bz6pzjkck0rv3494zfyst
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (bc70aab3c): zero new findings; both prior findings are checked.
    - next: task moved to done.
  timestamp: 2026-10-31T16:16:32.991284+00:00
- actor: claude-code
  id: 01m69bzr8gp6r2rgmq17qqtwbr
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 1 file
    - test: green — nextest 1541 passed, clippy and fmt clean
    - commit: bc70aab3c
    - review: clean — zero new findings, both prior findings checked, task done
  timestamp: 2026-10-31T16:16:50.960071+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffc880
project: remove-shelltool
title: Stop denying Bash and drop shell from agent setup
---
Do this first. The shell tool replaces Bash today. Agents get a Bash deny rule at serve time. If the shell tool goes and the deny stays, agents have no way to run commands.

Work:
- Remove the Bash deny logic from `crates/swissarmyhammer-tools/src/mcp/host.rs`, `server/profiles.rs`, `mirdan/src/settings.rs`, `mirdan/src/strategy/claude_settings.rs`, `mirdan/src/install/profile.rs` and `applier.rs`.
- Remove the test `crates/swissarmyhammer-tools/tests/integration/serve_time_bash_deny.rs`.
- Change `edit_redirect_tests.rs` if it depends on the shell tool.
- Add a real-path test: after `init`, the Bash tool is NOT denied.
- Add a cleanup step: `deinit` or `doctor` removes old Bash deny entries that sah wrote before.

Done when: no code writes a Bash deny rule, and the new test passes. #remove-shell

## Review Findings (2026-10-31 11:07)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 17 file(s) reviewed, 6 not reviewed.

> 6 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 6 file(s)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1538` `completeness/inverse-operation-coverage` — The user-scope test asserts that Bash stays allowed after deinit, but init has already removed the seeded deny, so deinit's legacy Bash cleanup (the allow_tool call in run_lifecycle, reached via the Remove direction) is never shown to do work. Deleting that step would leave this test passing. The same gap exists in test_tool_lifecycle_local_scope at line 1583. The install half of the pair is covered, and the deinit half is not. In the user-scope and local-scope tests, re-seed the legacy Bash deny after init and before deinit, then assert that deinit removes it. Keep the existing post-init assertion. This makes the deinit cleanup path observable in both scopes.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:1583` `completeness/inverse-operation-coverage` — The local-scope test asserts that Bash stays allowed after deinit, but init has already removed the seeded legacy deny, so deinit's cleanup step is never shown to do work. Removing the allow_tool call from the Remove direction would leave this assertion passing. Re-seed the legacy Bash deny in the local settings file after init and before deinit, then assert that deinit removes it. Apply the same change as the user-scope test so both scopes exercise the deinit cleanup path.
