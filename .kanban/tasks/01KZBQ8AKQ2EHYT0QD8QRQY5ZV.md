---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzbrejgtsmrq2nfp0hh9k2g2
  text: |-
    Picked up. Research notes for the next agent:

    - `permissions_present` has exactly one caller in the workspace: `detect_component` in `crates/mirdan/src/status.rs`. No other crate reads it, so the blast radius of the contract change is inside mirdan.
    - `EDIT_REDIRECT_DENY_TOOLS` had 6 uses, all inside mirdan (1 definition, 1 use in `desired_edit_redirect_fragment`, 4 in tests). No external crate imported it.
    - Two more sites the card did not list needed the change:
      1. `status.rs::test_permissions_missing_then_installed` wrote `["Bash", "WebFetch"]` and asserted `Installed`. That test encodes the OLD one-tool contract. It now builds its deny list from the constant plus an unrelated `WebFetch` entry, so it still proves that unrelated deny entries do not disturb the probe.
      2. `edit_redirect_tests.rs::fragment_denies_native_edit_tools_without_hook` is renamed `fragment_denies_superseded_natives_without_hook`; its doc said "native edit tools" and "`shell` replaces `Bash`", which the one-set change makes wrong. Comments in `profile_tests.rs` that said "native mutators" are now "superseded native".
    - `apply_edit_redirect_at`'s doc said "the three deny entries" (wrong before the change too — the set held two). It now names the constant instead of a count.

    TDD record (RED observed before each fix):
    - `superseded_deny_set_is_exactly_the_four_natives` — RED: `left: ["Edit", "Write"]`, `right: ["Bash", "Edit", "Read", "Write"]`.
    - `edit_redirect_install_satisfies_permissions_detector` — RED: "the fragment the installer wrote must satisfy the doctor's permissions probe".
    - `permissions_present_false_when_deny_set_is_partial` — RED after the constant grew: "a deny set missing Edit must not read as installed".
    - `project_permissions_state_is_installed_after_full_deny` — passes in both states; it is the doctor-row coverage the card asked for, not a RED driver.

    Out of scope and unchanged, as the card directs: the serve-time `Local` deny still writes `["Bash", "Edit"]` to `settings.local.json`, and the `edit_redirect` profile flag plus `apply_edit_redirect_at` / `desired_edit_redirect_fragment` keep their names.
  timestamp: 2026-08-06T14:43:15.610689+00:00
- actor: claude-code
  id: 01kzbrevtkjqvjfw1z4pev24c0
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/profile_tests.rs, crates/mirdan/src/install/edit_redirect_tests.rs, crates/mirdan/src/status.rs. `cargo nextest run --package mirdan`: 432 tests run, 432 passed, 0 skipped. `cargo clippy --package mirdan --all-targets -- -D warnings`: clean. `cargo fmt` applied. `rg EDIT_REDIRECT_DENY_TOOLS crates/` returns nothing. `cargo check` on the mirdan dependents (swissarmyhammer-tools, swissarmyhammer-validators) passes.
    - next: /review
  timestamp: 2026-08-06T14:43:25.139332+00:00
- actor: claude-code
  id: 01kzbs6cwb2bm034x3kxwhvrc9
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run -E 'rdeps(mirdan)'` — 3231 passed, 0 failed, 0 skipped
    - found + fixed a real regression surfaced by the blast-radius run: `apps/swissarmyhammer-cli/src/commands/install/deinit.rs::tests::test_deinit_does_not_reallow_bash` asserted Bash survives `sah deinit`, which now contradicts this task's own accepted design (`SUPERSEDED_NATIVE_DENY_TOOLS` folds Bash into the profile-managed roster, and acceptance criterion #4 states deinit strips exactly the four entries). Root cause: the test seeded the simulated serve-time deny into the wrong file (`settings.json`, the file the profile's `edit_redirect` fragment actually manages) instead of `settings.local.json` (the file `InitScope::Local`'s `local_settings_sibling` resolves to, which the profile fragment never touches at any scope — confirmed via `mirdan::install::profile::resolve_agent_file`, which only ever calls `agent_project_settings_file`/`agent_global_settings_file`).
    - fix: rewrote the test to seed Bash into `settings.local.json` (the true sticky file) and added a companion test, `test_deinit_removes_bash_from_profile_managed_settings`, asserting deinit does strip Bash from the profile-managed `settings.json` — documenting both halves of the new, intended contract.
    - next: hand back to the caller for review/commit.
    task: ^qrqy5zv
  timestamp: 2026-08-06T14:56:16.267745+00:00
- actor: claude-code
  id: 01kzbsjveqmg1ae4rgkjmqje0v
  text: |
    ### review — findings
    - evidence: 1 finding (1 confirmed, 0 refuted) on scope `review sha HEAD~1..HEAD` (915b7d542) — apps/swissarmyhammer-cli/src/commands/install/deinit.rs:108
    - next: fix the finding in the `## Review Findings (2026-08-06 09:57)` section, then re-run /review
  timestamp: 2026-08-06T15:03:04.407157+00:00
- actor: claude-code
  id: 01kzbskr489fwwc552vjbyqtkh
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 4 files, 145 insertions, 47 deletions (mirdan profile.rs, status.rs, edit_redirect_tests.rs, profile_tests.rs)
    - test: green — cargo fmt --check clean; cargo clippy --workspace --all-targets -D warnings clean; cargo nextest run -E 'rdeps(mirdan)' 3231 passed, 0 failed, 0 skipped. Blast radius fixed one reverse-dependency test in apps/swissarmyhammer-cli/src/commands/install/deinit.rs
    - commit: 915b7d542
    - review: findings — apps/swissarmyhammer-cli/src/commands/install/deinit.rs:108 (test_deinit_removes_bash_from_profile_managed_settings seeds all four SUPERSEDED_NATIVE_DENY_TOOLS but asserts only Bash is absent; must loop the constant and assert each of the four)
  timestamp: 2026-08-06T15:03:33.768385+00:00
- actor: claude-code
  id: 01kzbt2sdnnvcfp638cc5231fs
  text: |-
    Fixed the review finding in `apps/swissarmyhammer-cli/src/commands/install/deinit.rs`. The finding, word for word: the test `test_deinit_removes_bash_from_profile_managed_settings` seeds all four tools from `SUPERSEDED_NATIVE_DENY_TOOLS` but only asserts that 'Bash' is removed; the assertion must loop over the constant and assert each of the four is absent.

    The cause is a hardcoded single tool name where the constant drives the behavior. I removed that cause from the whole file, not only the named line:

    - `test_deinit_removes_bash_from_profile_managed_settings` is renamed `test_deinit_removes_superseded_natives_from_profile_managed_settings`. It now loops `SUPERSEDED_NATIVE_DENY_TOOLS` and asserts each entry is absent.
    - The sibling test had the same partial-assertion shape: `test_deinit_does_not_reallow_bash` seeded only `["Bash"]` into `settings.local.json` and asserted only `Bash` survives, although the sticky-file contract holds for every entry of the roster. It is renamed `test_deinit_does_not_reallow_superseded_natives`, seeds the whole roster, and loops the constant to assert each entry survives.
    - Both tests seeded the tool names as JSON string literals. The literals are gone: a new fixture helper, `seed_superseded_deny(home, file_name)`, builds `permissions.deny` from the constant, so a tool added to the roster is seeded too. A second helper, `deny_entries(path)`, reads the array back. The helpers also keep the two tests free of duplicated read/parse code.

    TDD record (RED observed for each assertion loop, then reverted):
    - Removal direction: temporarily made `apply_edit_redirect_at` skip `Read` on removal. RED: "Read must be removed from settings.json's permissions.deny by deinit (it is part of SUPERSEDED_NATIVE_DENY_TOOLS), got [\"Read\"]". The old Bash-only assertion passed under the same break — this is exactly the silent breakage the finding names.
    - Sticky direction: temporarily simulated a deinit that strips `Read` from `settings.local.json`. RED: "Read must remain in settings.local.json's permissions.deny after deinit (the serve-time deny is sticky), got [\"Bash\", \"Edit\", \"Write\"]". A Bash-only assertion would have passed.
    - GREEN after both reverts: 14 deinit tests run, 14 passed.

    No production code changed — the finding is about test assertions, and the removal logic already iterates the constant correctly.
  timestamp: 2026-08-06T15:11:46.613150+00:00
- actor: claude-code
  id: 01kzbt36t9xk312ht5g58b63b9
  text: |-
    ### implement — changed
    - evidence: 1 file — apps/swissarmyhammer-cli/src/commands/install/deinit.rs (132 lines changed). `cargo fmt` applied. `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo nextest run -E 'rdeps(mirdan)'`: 3231 tests run, 3231 passed, 0 skipped. Review finding checked off in the `## Review Findings (2026-08-06 09:57)` section.
    - next: /review
  timestamp: 2026-08-06T15:12:00.329886+00:00
- actor: claude-code
  id: 01kzbt94gy99x82hv2wzwf6e4s
  text: |-
    ### commit — changed
    - evidence: c48169ae2 test(install): loop SUPERSEDED_NATIVE_DENY_TOOLS in deinit tests
    - next: none
  timestamp: 2026-08-06T15:15:14.590531+00:00
- actor: claude-code
  id: 01kzbtgwz5v8gjdnqbxmgxykgy
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (c48169ae2) — 0 findings, 0 confirmed, 1 refuted, 9 attempted. Prior finding at apps/swissarmyhammer-cli/src/commands/install/deinit.rs:108 is resolved — the test now loops `SUPERSEDED_NATIVE_DENY_TOOLS` and asserts each tool is absent (deinit.rs:130-135). Its checklist item is checked.
    - next: none — task moved to done.
  timestamp: 2026-08-06T15:19:28.997713+00:00
- actor: claude-code
  id: 01kzbtjzzbsqhk7ygy1m5apys3
  text: |
    ### finish iteration 2 — clean
    - implement: changed — 1 file, 78 insertions, 54 deletions (apps/swissarmyhammer-cli/src/commands/install/deinit.rs); both deinit tests now seed and loop SUPERSEDED_NATIVE_DENY_TOOLS through shared fixture helpers
    - test: green — cargo fmt --check clean; cargo clippy --workspace --all-targets -D warnings clean; cargo nextest run -E 'rdeps(mirdan)' 3231 passed, 0 failed, 0 skipped (verified first-hand by the orchestrator, not only by the sub agent)
    - commit: c48169ae2
    - review: clean — zero new findings, prior finding checked, task moved to done
  timestamp: 2026-08-06T15:20:37.611010+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffb080
title: 'One deny set for superseded natives: add Bash + Read, make the doctor probe agree'
---
## What

`sah doctor` reports `Claude Code · project · Permissions ┆ missing at .claude/settings.json` although `sah init` wrote the permissions. The installer and the detector do not agree, and the deny set does not contain all the natives that sah supersedes.

Evidence on disk before the fix:

| File | `permissions.deny` | Writer |
|---|---|---|
| `~/.claude/settings.json` | `["Edit", "Write"]` | `sah init user` |
| `<repo>/.claude/settings.json` | `[]` | `sah init` |
| `<repo>/.claude/settings.local.json` | `["Bash", "Edit"]` | serve-time deny |

Two defects:

1. **The detector probes for the wrong tool.** `permissions_present` (`crates/mirdan/src/status.rs`) returned true only when `permissions.deny` contains `"Bash"`. The init-time fragment (`desired_edit_redirect_fragment`, `crates/mirdan/src/install/profile.rs`) writes `EDIT_REDIRECT_DENY_TOOLS = ["Edit", "Write"]` and never writes `"Bash"`. Thus a correct install always reports `Missing`. The `"Bash"` deny comes from a different mechanism — serve-time `mirdan::install::deny_tool` at `crates/swissarmyhammer-tools/src/mcp/server.rs:1171` — which uses `InitScope::Local` and so writes `settings.local.json`. `STATUS_SCOPES` (`status.rs:413`) is `[Project, User]` only, so the doctor never reads that file.

2. **The deny set is incomplete.** sah supersedes `Bash` with the `shell` tool and `Read`/`Edit`/`Write` with the `files` tool, but the init-time fragment denied only `Edit` and `Write`. The model could still call native `Bash` and native `Read`.

Fix: make one constant the single source of truth for the whole superseded-native set, and make the detector check that whole set.

Do this:

1. In `crates/mirdan/src/install/profile.rs`, rename `EDIT_REDIRECT_DENY_TOOLS` to `SUPERSEDED_NATIVE_DENY_TOOLS` and set it to `&["Bash", "Edit", "Read", "Write"]`. Update the doc comment: the deny forces `Bash` to the served `shell` tool and `Read`/`Edit`/`Write` to the served `files` tool. Keep the `edit_redirect` profile flag and the `apply_edit_redirect_at` / `desired_edit_redirect_fragment` function names as they are — renaming those is not part of this task.
2. In `crates/mirdan/src/status.rs`, change `permissions_present` to return true only when `permissions.deny` contains **every** entry of `SUPERSEDED_NATIVE_DENY_TOOLS`. Import the constant; do not respell the tool names.
3. Update the mechanical uses of the old constant name in `crates/mirdan/src/install/profile_tests.rs` and `crates/mirdan/src/install/edit_redirect_tests.rs`. These loops already iterate the constant, so they need the new name only.

Accepted consequences, agreed with the user:

- Native `Read` becomes denied. The agent must use the `files` read op.
- The serve-time `Local`-scope deny keeps writing `["Bash", "Edit"]` to `settings.local.json`. That overlap is harmless and stays out of scope. Making `ToolCategory::Replacement` carry more than one native is a separate concern — do not do it here.

## Acceptance Criteria

- [x] `SUPERSEDED_NATIVE_DENY_TOOLS` in `crates/mirdan/src/install/profile.rs:827` equals `["Bash", "Edit", "Read", "Write"]`, and `EDIT_REDIRECT_DENY_TOOLS` no longer exists in the workspace (`rg EDIT_REDIRECT_DENY_TOOLS crates/ apps/` returns nothing).
- [x] `permissions_present` returns `true` for a settings file whose `permissions.deny` holds all four tools, and `false` when any one of the four is absent.
- [x] Installing the fragment with `apply_edit_redirect_at(path, true)` then calling `permissions_present(path)` returns `true` — installer and detector agree.
- [x] `deinit_profile` still strips exactly the four entries and keeps unrelated `deny` entries and unrelated settings keys.
- [x] `cargo nextest run -E 'rdeps(mirdan)'` passes with no new warnings. (This criterion first said `cargo test -p mirdan`; plain `cargo test` is forbidden in this workspace, so the nextest blast-radius run replaces it.)

## Tests

- [x] `crates/mirdan/src/install/edit_redirect_tests.rs`: `edit_redirect_install_satisfies_permissions_detector` — write a temp settings file, call `apply_edit_redirect_at(path, true)`, assert `crate::status::permissions_present(path)` is `true`. Observed RED before the fix. It is the regression test for the reported doctor bug.
- [x] `crates/mirdan/src/install/edit_redirect_tests.rs`: `superseded_deny_set_is_exactly_the_four_natives` — assert the constant equals `["Bash", "Edit", "Read", "Write"]`. Observed RED (`left: ["Edit", "Write"]`).
- [x] `crates/mirdan/src/status.rs` (inline `mod tests`): `permissions_present_false_when_deny_set_is_partial` — for each tool in the set, write a settings file that holds the other three and assert `permissions_present` is `false`. Observed RED.
- [x] `crates/mirdan/src/status.rs` (inline `mod tests`): `project_permissions_state_is_installed_after_full_deny` — write `.claude/settings.json` with all four denies, call `check_component(&agent, Component::Permissions, InitScope::Project)` and assert `.state == ComponentState::Installed`.
- [x] `cargo nextest run -E 'rdeps(mirdan)'` — 3231 passed, 0 failed, 0 skipped.

## Out of scope, found while working

The blast-radius run exposed `apps/swissarmyhammer-cli/src/commands/install/deinit.rs::test_deinit_does_not_reallow_bash`, which seeded the simulated serve-time Bash deny into `settings.json` (which the profile's `edit_redirect` fragment does manage) instead of `settings.local.json` (which `resolve_agent_file` never touches at any scope). Both deinit tests now seed and loop the whole roster through shared fixture helpers.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.

#mirdan #init-doctor #bug

## Review Findings (2026-08-06 09:57)

- [x] `apps/swissarmyhammer-cli/src/commands/install/deinit.rs:108` — The test `test_deinit_removes_bash_from_profile_managed_settings` seeds all four tools from `SUPERSEDED_NATIVE_DENY_TOOLS` (line 94) but only asserts that 'Bash' is removed (line 108). Since the code removes all four tools via iteration over the constant, the test assertion should verify all four are gone to prevent silent breakage if the removal logic ever skips Edit, Read, or Write. Replace the single Bash assertion with a loop that iterates over `SUPERSEDED_NATIVE_DENY_TOOLS` and asserts each tool is absent: `for tool in SUPERSEDED_NATIVE_DENY_TOOLS { assert!(!deny.iter().any(|v| v.as_str() == Some(tool)), "{tool} must be removed"); }`.
