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
position_column: doing
position_ordinal: '8480'
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