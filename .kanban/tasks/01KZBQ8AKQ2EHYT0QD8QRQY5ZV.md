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
position_column: doing
position_ordinal: '8380'
title: 'One deny set for superseded natives: add Bash + Read, make the doctor probe agree'
---
## What

`sah doctor` reports `Claude Code · project · Permissions ┆ missing at .claude/settings.json` although `sah init` wrote the permissions. The installer and the detector do not agree, and the deny set does not contain all the natives that sah supersedes.

Evidence on disk today:

| File | `permissions.deny` | Writer |
|---|---|---|
| `~/.claude/settings.json` | `["Edit", "Write"]` | `sah init user` |
| `<repo>/.claude/settings.json` | `[]` | `sah init` |
| `<repo>/.claude/settings.local.json` | `["Bash", "Edit"]` | serve-time deny |

Two defects:

1. **The detector probes for the wrong tool.** `permissions_present` (`crates/mirdan/src/status.rs:627`) returns true only when `permissions.deny` contains `"Bash"`. The init-time fragment (`desired_edit_redirect_fragment`, `crates/mirdan/src/install/profile.rs:835`) writes `EDIT_REDIRECT_DENY_TOOLS = ["Edit", "Write"]` (`profile.rs:823`) and never writes `"Bash"`. Thus a correct install always reports `Missing`. The `"Bash"` deny comes from a different mechanism — serve-time `mirdan::install::deny_tool` at `crates/swissarmyhammer-tools/src/mcp/server.rs:1171` — which uses `InitScope::Local` and so writes `settings.local.json`. `STATUS_SCOPES` (`status.rs:413`) is `[Project, User]` only, so the doctor never reads that file.

2. **The deny set is incomplete.** sah supersedes `Bash` with the `shell` tool and `Read`/`Edit`/`Write` with the `files` tool, but the init-time fragment denies only `Edit` and `Write`. The model can still call native `Bash` and native `Read`.

Fix: make one constant the single source of truth for the whole superseded-native set, and make the detector check that whole set.

Do this:

1. In `crates/mirdan/src/install/profile.rs`, rename `EDIT_REDIRECT_DENY_TOOLS` to `SUPERSEDED_NATIVE_DENY_TOOLS` and set it to `&["Bash", "Edit", "Read", "Write"]`. Update the doc comment: the deny forces `Bash` to the served `shell` tool and `Read`/`Edit`/`Write` to the served `files` tool. Keep the `edit_redirect` profile flag and the `apply_edit_redirect_at` / `desired_edit_redirect_fragment` function names as they are — renaming those is not part of this task.
2. In `crates/mirdan/src/status.rs`, change `permissions_present` to return true only when `permissions.deny` contains **every** entry of `SUPERSEDED_NATIVE_DENY_TOOLS`. Import the constant; do not respell the tool names.
3. Update the mechanical uses of the old constant name in `crates/mirdan/src/install/profile_tests.rs:277` and `:307`, and in `crates/mirdan/src/install/edit_redirect_tests.rs:17`, `:51`, `:121`. These loops already iterate the constant, so they need the new name only.

Accepted consequences, agreed with the user:

- Native `Read` becomes denied. The agent must use the `files` read op.
- The serve-time `Local`-scope deny keeps writing `["Bash", "Edit"]` to `settings.local.json`. That overlap is harmless and stays out of scope. Making `ToolCategory::Replacement` carry more than one native is a separate concern — do not do it here.

## Acceptance Criteria

- [ ] `SUPERSEDED_NATIVE_DENY_TOOLS` in `crates/mirdan/src/install/profile.rs` equals `["Bash", "Edit", "Read", "Write"]`, and `EDIT_REDIRECT_DENY_TOOLS` no longer exists in the workspace (`rg EDIT_REDIRECT_DENY_TOOLS crates/` returns nothing).
- [ ] `permissions_present` returns `true` for a settings file whose `permissions.deny` holds all four tools, and `false` when any one of the four is absent.
- [ ] Installing the fragment with `apply_edit_redirect_at(path, true)` then calling `permissions_present(path)` returns `true` — installer and detector agree.
- [ ] `deinit_profile` still strips exactly the four entries and keeps unrelated `deny` entries and unrelated settings keys (the existing test at `crates/mirdan/src/install/profile_tests.rs:295`).
- [ ] `cargo test -p mirdan` passes with no new warnings.

## Tests

- [ ] `crates/mirdan/src/install/edit_redirect_tests.rs`: add `edit_redirect_install_satisfies_permissions_detector` — write a temp settings file, call `apply_edit_redirect_at(path, true)`, assert `crate::status::permissions_present(path)` is `true`. This test fails before the fix (the fragment holds no `"Bash"`) and passes after. It is the regression test for the reported doctor bug.
- [ ] `crates/mirdan/src/install/edit_redirect_tests.rs`: add `superseded_deny_set_is_exactly_the_four_natives` — assert the constant equals `["Bash", "Edit", "Read", "Write"]`, so a silent change to the set fails a test.
- [ ] `crates/mirdan/src/status.rs` (inline `mod tests`, line 670): add `permissions_present_false_when_deny_set_is_partial` — for each tool in the set, write a settings file that holds the other three and assert `permissions_present` is `false`.
- [ ] `crates/mirdan/src/status.rs` (inline `mod tests`): add `project_permissions_state_is_installed_after_full_deny` — write `.claude/settings.json` with all four denies, call `check_component(&agent, Component::Permissions, InitScope::Project)` and assert `.state == ComponentState::Installed`. Follow the `check_component` pattern at `status.rs:1035`.
- [ ] Run `cargo test -p mirdan` — all tests pass.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.

#mirdan #init-doctor #bug
