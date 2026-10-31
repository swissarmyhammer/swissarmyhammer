---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69xrdtn6jpcn5dwyqgne0f7
  text: |-
    ### Requirement from the user (2026-10-31): never deny Bash or Edit to Claude again

    The user said: "we also need to make sure we do NOT deny Bash and Edit to claude any more". Treat this as a hard requirement of this card. It covers Bash too, not only Edit, Read and Write.

    State found on this machine after the finished Bash card ^axt7hzt and a fresh `sah init user`:
    - `~/.claude/settings.json` has `permissions.deny = ["Edit","Read","Write"]`. Bash is gone at user scope.
    - Project and local scope files still hold old denies, because the cleanup only runs when `init` or `deinit` runs at that scope:
      - `/Users/wballard/github/swissarmyhammer/.claude/settings.local.json`: `["Bash","Edit"]`
      - `/Users/wballard/github/swissarmyhammer/swissarmyhammer/.claude/settings.json`: `["Bash","Edit","Read","Write"]`
      - `/Users/wballard/github/swissarmyhammer/swissarmyhammer/.claude/settings.local.json`: `["Bash","Edit"]`

    So add to the work:
    1. The legacy cleanup list is `Bash`, `Edit`, `Read`, `Write` (one constant, one place; merge `LEGACY_BASH_DENY_TOOL` into it). It runs in `init` and `deinit` at EVERY scope: user, project and local. Check that the project and local code paths call it, not only the user path.
    2. Add a real-path test per scope (user, project, local): seed the settings file with `["Bash","Edit","Read","Write","WebFetch"]`, run the real init, assert only `WebFetch` stays. Run the real deinit with the same seed and assert the same.
    3. Add a guard test that scans every profile (sah, kanban-cli, code-context-cli, mirdan) after a real init at each scope and asserts the settings file never has Bash, Edit, Read or Write in `permissions.deny`.
    4. Look for any OTHER code that denies these tools: `crates/claude-agent/src/claude_process.rs` (permissions deny handling near lines 187 and 1261), `crates/swissarmyhammer-agents` (`disallowed_tools`), `crates/mirdan/src/strategy/claude_settings.rs`, `settings.rs`. Report each hit on this card. Remove a deny that sah writes for the main Claude session.
    5. Do NOT change the `disallowed-tools: "write edit"` line in `builtin/agents/double-check/AGENT.md` here. That limits one read-only verifier sub agent, it is not a deny for Claude. Record on this card that it names the removed sah tools, so the skills card ^84v63mm decides its new form.
    6. Final check, live: after the code lands, the user runs `sah init user`, `sah init project` and `sah init local` in this repo. List the three settings files above and the user file with `jq '.permissions.deny'` and show that none holds Bash, Edit, Read or Write.
  timestamp: 2026-10-31T21:27:25.269024+00:00
- actor: claude-code
  id: 01m69zv0vpg92ss3znhaqaaqkg
  text: |-
    ### implement — changed
    - files: crates/mirdan/src/install/profile.rs, status.rs, test_support.rs, tool_install.rs, agents.rs, strategy/claude_settings.rs, install/mod.rs, install/profile_tests.rs, install/profile_consistency_tests.rs, install/legacy_deny_tests.rs (renamed from edit_redirect_tests.rs); apps/swissarmyhammer-cli commands/profile.rs, registry.rs, install/deinit.rs, doctor/checks.rs.
    - code: removed Profile.edit_redirect, the deny fragment, SUPERSEDED_NATIVE_DENY_TOOLS, LEGACY_BASH_DENY_TOOL, Component::Permissions and permissions_present. New LEGACY_NATIVE_DENY_TOOLS (Bash, Edit, Read, Write). init_profile and deinit_profile now run the cleanup for every profile at every scope. Local scope now cleans settings.local.json (the old fragment used settings.json for local scope).
    - RED: 10 of 12 new tests failed before the code change (the 2 that passed are no-change invariant tests). GREEN: 19 of 19 pass.
    - tests: per scope init and deinit seed [Bash,Edit,Read,Write,WebFetch] and keep only WebFetch; guard tests for sah, kanban, code-context and mirdan (Profile::default; mirdan has no own profile) at every scope; fresh install writes no deny key; unrelated keys kept; clean file not rewritten.
    - other deny hits: none write a deny. claude_process.rs (lines near 187, 1261) only loads setting sources. swissarmyhammer-agents disallowed_tools is parse and format only. strategy/claude_settings.rs holds key constants only. settings.rs has only generic helpers and a test fixture. strategy allow_tool only removes a deny and has no non-test caller. builtin/agents/double-check/AGENT.md `disallowed-tools: "write edit"` is unchanged; it names the removed sah tools, so card ^84v63mm decides its new form.
    - left for later cards: doc/src/concepts/closed-write-surface.md still describes the deny; tool_registry/tests/disabled_tools.rs comments name the native edit surface.
    - checks: cargo check --workspace --tests ok; nextest rdeps(mirdan)|rdeps(swissarmyhammer-cli) 2983 passed, 0 failed; cargo fmt --check ok; clippy -D warnings ok. review working: 0 findings. review file: legacy_deny_tests.rs 0 findings. review file on whole profile.rs (18), status.rs (7) and deinit.rs (1 other) still reports findings in code this card did not write (error types, newtypes, duplication); they are NOT fixed.
    - the user must still run live: sah init user/project/local, then jq '.permissions.deny' on the four files.
  timestamp: 2026-10-31T22:03:47.446211+00:00
- actor: claude-code
  id: 01m6a00bpt16rx58cmyrtwpjyn
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace --no-fail-fast: 13942 run, 13942 passed (38 slow), 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: exit 0, zero warnings. cargo fmt --check: exit 0.
    - next: review
  timestamp: 2026-10-31T22:06:42.394847+00:00
position_column: doing
position_ordinal: '8480'
project: remove-files-tool
title: Stop denying native Edit, Read and Write at install time
---
Do this first. Today sah closes the host file tools. `SUPERSEDED_NATIVE_DENY_TOOLS = ["Edit", "Read", "Write"]` in `crates/mirdan/src/install/profile.rs:953` writes a `permissions.deny` list into `~/.claude/settings.json` (and project settings). If the MCP files tool goes and the deny stays, agents cannot read or write any file.

Same shape as the finished Bash card: ^axt7hzt (`LEGACY_BASH_DENY_TOOL`). Copy its method.

Work:
- Remove the `edit_redirect` field from `Profile`, `desired_edit_redirect_fragment`, `apply_edit_redirect_at`, `apply_profile_edit_redirect`, and the `if profile.edit_redirect` branches in `init_profile` and `deinit_profile` (`profile.rs` lines about 238-241, 859, 939-1030, 1265, 1366).
- Replace the deny with cleanup: on `init` and `deinit`, strip `Edit`, `Read` and `Write` from `permissions.deny` in the settings file. Name the constant `LEGACY_NATIVE_DENY_TOOLS`. Keep unrelated deny entries and unrelated keys.
- Remove the status and doctor checks for the redirect in `crates/mirdan/src/status.rs` (lines about 627-641, 950-1004), and the wording in `sah doctor` and `mirdan doctor`.
- Remove the redirect from `apps/swissarmyhammer-cli/src/commands/profile.rs`, `registry.rs` and `install/deinit.rs`, and from `crates/mirdan/src/install/mod.rs` and `test_support.rs`.
- Delete or rewrite `crates/mirdan/src/install/edit_redirect_tests.rs`. Rewrite as cleanup tests that run the real `init_profile` and `deinit_profile`: seed a settings file with the three deny entries plus an unrelated deny entry, run init, assert only the unrelated entry stays. Do the same for deinit.
- Add a test that proves a fresh install writes no deny for Edit, Read or Write.

Warning: the repo may hold uncommitted edits in `crates/mirdan/src/install/profile.rs`, `profile_tests.rs` and `profile_consistency_tests.rs` from the stopped card ^y1japkh. Run `git status` first. If those edits are still there, stop and report. Do not mix them into this card.

Done when: no code writes a deny for Edit, Read or Write, the cleanup test passes, and `cargo nextest run --workspace` is green. #remove-files