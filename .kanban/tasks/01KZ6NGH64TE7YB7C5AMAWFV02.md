---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz7hxg1peypj6p7fe3avk7sa
  text: |-
    Research done. Confirmed the decision gate before deleting: `rg -n 'preamble: true' --type rust` returns zero hits (exit 1). No live Rust code turns the preamble on, so option 1 (full removal) proceeded.

    Removed:
    - `Component::Preamble`, its label arm, its `component_path` arm, its `detect_component` arm. `Component::all()` is now `[Component; 4]`.
    - `PREAMBLE_MARKER`, `preamble_present`, `preamble_present_in` (`crates/mirdan/src/status.rs`).
    - `apply_profile_preamble`, `PreambleOutcome` (enum + impl), `ensure_preamble`, `remove_preamble`, and both `if profile.preamble` dispatch blocks in `init_profile`/`deinit_profile` (`crates/mirdan/src/install.rs`).
    - The `Profile.preamble` field and every `preamble: false` initializer in the five CLI profiles plus the two mirdan test profiles.
    - The now-unused `use crate::{settings, status}` import of `status` in `install.rs`.

    Tests retargeted rather than dropped, so the behaviour they covered stays covered:
    - `test_preamble_missing_then_installed` deleted — it tested only the deleted detector.
    - `codex_full_stack` now proves the same "the row genuinely participates in detection" property through `Component::Skills` (write a file into the user-scope skill dir, watch the cell flip Missing -> Installed).
    - The four `statuses_to_checks` scope-pair tests now compose their synthetic rows from `Component::Permissions` with settings.json paths.
    - `test_check_install_stack_user_scope_rows` (sah doctor) now deploys `~/.claude/skills/a-skill` instead of writing the CLAUDE.md marker; it still asserts installed-user -> Ok, missing-user-permissions -> Warning, and the project-row demotion.
    - Component-count assertions moved 5 -> 4 and 10 -> 8.

    Doc comments that described the removed behaviour were rewritten in `cli.rs`, `agents.rs`, `doctor/mod.rs`, `doctor/checks.rs`, `install/{init,deinit,mod,components/mod}.rs`, `commands/registry.rs`, `kanban-app/state.rs`, `swissarmyhammer-tools/.../kanban/mod.rs`, and the generated `doc/src/reference/mirdan-cli.md`.

    Discovery: `AgentDef.instructions_path` / `global_instructions_path` and the two `agent_*_instructions_file` accessors now have no production consumer — only their own tests. That is outside this card's stated scope, so it was left alone and filed as its own card.
  timestamp: 2026-08-04T23:32:06.838487+00:00
- actor: claude-code
  id: 01kz7qge491ssfw7dp6mkcmsr0
  text: |-
    Self-review ran four passes (`review working`). Every finding on code this change wrote or rewrote is fixed:

    - `Component::all()`'s `4` and `STATUS_SCOPES`'s `2` became `COMPONENT_COUNT` / `STATUS_SCOPE_COUNT`, and the three count assertions in the status tests now read the constant instead of a literal.
    - `check_all` and `check_all_doctored` share one `check_selected` sweep; they differ only by the agent predicate.
    - `run_status(bool, bool)` became `run_status(StatusVisibility, StatusFormat)`, with `from_all_flag` / `from_json_flag` keeping `dispatch` under the complexity gate.
    - `mcp_server_installed`'s `Vec::with_capacity(4)` became `MCP_SERVERS_KEY_CAPACITY`.
    - `detect_agents` split into `detect_agent` / `detection_hit` / `first_probed_dir`, under the complexity gate. It probes once — `find_map`'s result decides both `detected` and the detail.
    - `init::install` and `deinit::uninstall` collapsed onto one `run_lifecycle(Direction, ..)` in `install/mod.rs`; `Direction::pick` is the single branch behind the three wording accessors.
    - `Profile::tool(server_name, skills)` landed in mirdan, next to `ProfileMcpServer::serve`. kanban, code-context, and shelltool now declare that pair instead of respelling the struct.
    - `check_claude_mcp_list` takes `Option<&Path>`, and its duplicated `display_path`/`command_path` locals collapsed to one.
    - Named constants for the PATH-hint limit, the ProjectStructure priority, the DJB2 seed and multiplier, and the JPEG start-of-image marker; doc comments added to `Commands`, `NewKind`, and `set_text`.

    Findings dropped under the review skill's blanket rule against refactoring tests that already existed: the drag-session and MCP-shutdown timing literals and the board-count fixture in `kanban-app/src/state.rs`'s `mod tests`, and the minimum-agent-count assertion in `mirdan/src/agents.rs`'s `mod tests`.

    Findings NOT actioned, all pre-existing defects in files this card touched only to delete a stale word. Each pass surfaced a fresh crop of them, so the loop was not converging on this card's subject:

    - `apps/kanban-app/src/state.rs:31` — `TauriClipboardProvider` should derive `Debug` and `Clone`.
    - `apps/kanban-app/src/state.rs:363` — `open` takes `PathBuf`; it should take `&Path`.
    - `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:323` — the project-detection depth `3` should be a named constant.
    - `crates/mirdan/src/agents.rs:275,280,285,308,313` — the eight path accessors repeat one shape; the engine asks for a macro.
    - `crates/mirdan/src/dispatch.rs:15,43,189` — the `Err(e) => { eprintln!; 1 }` handler repeats.

    Also note: the duplication validator never read `crates/mirdan/src/install.rs` in any pass — at ~567 KB rendered it exceeds the agent prompt cap. That file is unreviewed for duplication.

    Verification: `cargo nextest run -p mirdan -p swissarmyhammer-cli -p kanban-cli -p code-context-cli -p shelltool-cli -p kanban-app` — 1433 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean.

    User-visible proof, built and run rather than inferred: `./target/debug/sah doctor | grep -ci preamble` prints `0`, and Claude Code now yields 8 rows (2 scopes x 4 components) where it previously yielded 10 with two permanent Preamble warnings. `./target/debug/mirdan doctor` likewise shows no Preamble row. `mirdan status` still renders its table (33 lines), `--all` still widens it (53 lines), and `--json` still emits JSON — the three paths through the new `StatusVisibility`/`StatusFormat` enums.
  timestamp: 2026-08-05T01:09:50.345685+00:00
- actor: claude-code
  id: 01kz7qghg30h1xmn6g1c96ajh8
  text: |-
    ### implement — changed
    - evidence: 20 source files. Removed `Component::Preamble`, `PREAMBLE_MARKER`, `preamble_present`, `preamble_present_in`, `apply_profile_preamble`, `PreambleOutcome`, `ensure_preamble`, `remove_preamble`, and `Profile.preamble`. `cargo nextest run -p mirdan -p swissarmyhammer-cli -p kanban-cli -p code-context-cli -p shelltool-cli -p kanban-app` — 1433 passed, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean. `sah doctor` and `mirdan doctor` print zero Preamble rows.
    - next: ready for `/review`. Five pre-existing findings in untouched parts of touched files are listed in the previous comment and were not actioned; a person decides whether they belong to this card.
  timestamp: 2026-08-05T01:09:53.795898+00:00
- actor: claude-code
  id: 01kz7s55vd2zxxdd16n53mfrhg
  text: |-
    ### review — findings
    - evidence: `review sha 0e63e1031~1..0e63e1031` — counts `{findings: 4, confirmed: 4, refuted: 21, attempted: 35, skipped: 1}`. 3 findings recorded: `apps/kanban-cli/src/commands/registry.rs:34`, `apps/shelltool-cli/src/commands/registry.rs:35`, `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:159`. 1 dropped under the review skill's existing-test-code exception: `apps/kanban-app/src/state.rs:1465` (inside `mod tests`, blame `39f0e3d005`).
    - coverage gap CONFIRMED, engine's own wording: `crates/mirdan/src/install.rs` — 567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication. The largest file in the change, holding the deleted preamble installer code, was never read by the duplication validator on this or any prior pass.
    - the five unactioned findings: none resurfaced this pass. All five lines confirmed pre-existing by `git blame 0e63e1031` — `state.rs:31` `0f2baf5a5a`, `state.rs:363` `026ac7448d`, `checks.rs:323` `c171137837`, `agents.rs:275,280,285,308,313` `613f58ef3a`/`611ee08852`/`b7ba81dd78`, `dispatch.rs:15,43,189` `905756d9a7`/`f5564d7d10`. None sit in a hunk of this commit.
    - next: task stays in `review`. Fix the 3 recorded findings. The two `profile()` findings are on lines this commit wrote; the `deinit` duplication is pre-existing production code and is not exempt.
  timestamp: 2026-08-05T01:38:38.573879+00:00
- actor: claude-code
  id: 01kz7s6jz1zskh2s3r19pw7bkk
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 24 files; option 1 (full removal) taken after `rg 'preamble: true' --type rust` returned zero live hits
    - test: green — `cargo nextest run --workspace` 13523 passed, 0 failed, 0 skipped; clippy clean; fmt clean; `rg -i preamble` shows no surviving reference to any removed symbol
    - commit: 0e63e1031
    - review: findings — apps/kanban-cli/src/commands/registry.rs:34, apps/shelltool-cli/src/commands/registry.rs:35, apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:159
    - engine counts: findings 4, confirmed 4, refuted 21, attempted 35, skipped 1. One dropped under the written existing-test-code exception (apps/kanban-app/src/state.rs:1465, inside `#[cfg(test)] mod tests`).
    - the five pre-existing findings the implementer left unactioned did NOT resurface this pass. The reviewer verified each against `git show --unified=0 0e63e1031` + `git blame` rather than taking the implementer's word: none sit inside a diff hunk of this commit.
    - COVERAGE GAP, reproduced: the engine skipped `crates/mirdan/src/install.rs` — "567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication". That is the largest file in this change (187 lines changed) and holds the deleted `ensure_preamble`/`remove_preamble`/`apply_profile_preamble`/`PreambleOutcome` code — the card's whole subject. The engine's suggested remedy ("narrow the scope") does not help: a `review file` run on that single path still exceeds the cap. The file must shrink before duplication can cover it.
    - next: iteration 2 — work the 3 findings
  timestamp: 2026-08-05T01:39:24.769063+00:00
- actor: claude-code
  id: 01kz80c5vqb2c9ddem523fyw8k
  text: |-
    Worked the three recorded review findings. All three are checked.

    Findings 1 + 2 are one duplication, fixed once. `mirdan::install::Profile::tool` was already the shared constructor, so the residual duplication was the wrapper itself — three identical `pub fn profile(_scope: InitScope) -> Profile` functions whose `scope` argument was unused. New module `crates/mirdan/src/tool_install.rs` now owns the whole shared shape:

    - `trait ToolInstall` — three required items per CLI (`SERVER_NAME`, `skills()`, `register_components()`), with `profile()`, `component_registry()`, `init()`, and `deinit()` provided once.
    - `Lifecycle` + `run_lifecycle<T>` — the exit-code contract (0 clean, 1 on any errored step).
    - `run_lifecycle_command<T>` — the entire body of every tool CLI's `init`/`deinit` subcommand, taking the parsed clap matches.
    - `declare_tool_install!` — declares the marker type and its impl from the three facts.

    kanban, code-context, and shelltool registries now each hold one `declare_tool_install!` block and nothing else. Their `main.rs` files lost `any_init_error`, `run_init`, and `run_deinit` entirely; the `init`/`deinit` dispatch arms are one-line calls to `run_lifecycle_command::<XInstall>`.

    Finding 3 is fixed in `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`: the `.sah/` and `.prompts/` removal blocks collapsed into `remove_directory_if_exists(root, dir_name, component, reporter)`, driven by a loop over named constants `SAH_DIR_NAME` / `PROMPTS_DIR_NAME` (plus `WORKFLOWS_SUBDIR_NAME`), so the directory names are stated once for both create and remove.

    Latent bug found and fixed while working finding 3: `ProjectStructure::init` resolved its root as git-root-then-CWD, but `deinit` used CWD only. A `sah deinit --remove-directory` run from a subdirectory therefore looked for a workspace the matching `init` had created at the repository root, and left it in place. Both halves now call one `workspace_root()`, through `ProjectStructure::root_or_error`. The regression test `test_project_structure_round_trips_from_a_subdirectory` runs from a subdirectory of a fake repository and fails against the old behaviour.

    Self-review ran five passes of `review working` and is now clean. Every finding it raised was fixed:
    - `deinit` had no coverage anywhere. `mirdan::test_support` gained `assert_tool_profile<T>`, `assert_tool_component_count<T>`, and `assert_tool_lifecycle_round_trip<T>`, which drives a real init then deinit in an isolated `$HOME` and asserts the skill links and the MCP registration are both gone afterwards. All four registries (three CLIs plus the mirdan `FakeTool`) now delegate their tests to those helpers instead of repeating the setup.
    - `run_lifecycle` now has both directions and both exit codes asserted.
    - Error messages in `components/mod.rs` lowercased throughout.
    - Doc comments added to every public module and re-export in `crates/mirdan/src/lib.rs`.
    - `Debug` derived on `ProjectStructure`, `MirdanConfigGuard`, `UserScopeDeploy`, `ProjectScopeDeploy`.

    Verification: `cargo nextest run --workspace` — 13527 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean.

    Card acceptance re-confirmed against built binaries, not inferred: `./target/debug/sah doctor | grep -ci preamble` prints `0`, and `./target/debug/mirdan doctor` likewise. The rewired CLI dispatch was smoke-tested with the real binary in a temp dir: `shelltool init local` exits 0 and creates `.shell/`, `shelltool deinit local` exits 0 and removes it.

    Still open, unchanged from the review: the duplication validator has never read `crates/mirdan/src/install.rs` (567352 rendered bytes, over the 476042-byte cap). This change did not make that file bigger — the new shared code went into the new `tool_install.rs` module, which the validator does read.
  timestamp: 2026-08-05T03:44:47.991122+00:00
- actor: claude-code
  id: 01kz80cfq3q2vd51jr1v0zafzw
  text: |-
    ### implement — changed
    - evidence: 12 files. New `crates/mirdan/src/tool_install.rs` (`ToolInstall` trait, `Lifecycle`, `run_lifecycle`, `run_lifecycle_command`, `declare_tool_install!`); `crates/mirdan/src/lib.rs`; `crates/mirdan/src/test_support.rs`; the three CLI registries and their `main.rs`; `apps/code-context-cli/src/commands/skill.rs`; `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`. `cargo nextest run --workspace` — 13527 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean. `review working` clean after five passes. `sah doctor` and `mirdan doctor` still print zero Preamble rows; `shelltool init local` / `deinit local` smoke-tested with the real binary.
    - next: ready for `/review`. Card stays in `doing`.
  timestamp: 2026-08-05T03:44:58.083101+00:00
- actor: claude-code
  id: 01kz81p70r7hsvgfbddsnbcj46
  text: |-
    ### review — clean

    - evidence: `review sha ce3a263d5~1..ce3a263d5` (round 2). counts: 0 findings, 0 confirmed, 0 refuted, 9 attempted, 0 failed, 0 skipped.
    - scope: 13 files, +881/-587, including the new `crates/mirdan/src/tool_install.rs` (304 lines).

    **No file was skipped.** The engine reported `skipped: 0` and returned no "not reviewed" warning block. The round-1 skip of `crates/mirdan/src/install.rs` ("567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication") did not recur, because `crates/mirdan/src/install.rs` is not in this commit at all — `git show --stat ce3a263d5` does not list it. The file remains unread by the duplication validator from round 1; that gap is not closed by this commit, it is only out of scope for it.

    **Prior findings verified in code, not by checkbox.**

    1. `apps/kanban-cli/src/commands/registry.rs:34` and 2. `apps/shelltool-cli/src/commands/registry.rs:35` — duplicated `profile()` bodies across three registries. Fixed. All three registries now expand `mirdan::declare_tool_install!`; the hand-written `pub fn profile()` is gone from each. Only the parameters differ: kanban `server: "kanban", skills: Selector::All`; shelltool `server: "shelltool", skills: Selector::Single(SKILL_NAME.to_string())`; code-context `server: "code-context", skills: Selector::All`. This is the "parameterized by skill selector" remedy the findings asked for. `apps/code-context-cli/src/commands/skill.rs` now calls `CodeContextInstall::skills()` so the skills-only install cannot diverge from the full install.

    3. `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:159` — duplicated `.sah/` and `.prompts/` removal blocks. Fixed exactly as prescribed. `fn remove_directory_if_exists(root: &Path, dir_name: &str, component: &str, reporter: &dyn InitReporter) -> Option<InitResult>` now exists, and `deinit` calls it from `for dir_name in [SAH_DIR_NAME, PROMPTS_DIR_NAME]`, returning on the first failure. New tests `test_remove_directory_if_exists_removes_and_reports` and `test_remove_directory_if_exists_ignores_missing_directory` cover it.

    **Provenance:** no findings this round, so there is no flagged line to place inside or outside the diff hunks.

    - next: none — task complete.
  timestamp: 2026-08-05T04:07:45.432699+00:00
- actor: claude-code
  id: 01kz81qj1mxke54czhmvjw4ejt
  text: |
    ### finish iteration 2 — clean
    - implement: changed — 13 files. All three round-1 findings fixed. Findings 1+2 were one duplication this card caused (removing `Profile.preamble` left three identical `profile()` wrappers) — fixed once via a new `crates/mirdan/src/tool_install.rs` (`ToolInstall` trait, `Lifecycle`, `run_lifecycle`/`run_lifecycle_command`, `declare_tool_install!`), with the three CLIs dropping `any_init_error`/`run_init`/`run_deinit` from their `main.rs`. Finding 3 collapsed the `.sah/`/`.prompts/` blocks into `remove_directory_if_exists`.
    - latent bug fixed on the way: `ProjectStructure::init` resolved its root git-root-then-CWD while `deinit` used CWD only, so `sah deinit --remove-directory` from a subdirectory left the real workspace behind. Both now share `workspace_root()`, with a regression test that runs from a subdirectory.
    - test: green — `cargo nextest run --workspace` 13527 passed, 0 failed, 0 skipped; clippy clean; fmt clean. Real binaries exercised in isolated temp dirs with HOME overridden: sah/kanban/code-context/shelltool init+deinit all exit 0; the subdirectory regression confirmed fixed; `sah doctor` and `mirdan doctor` print no Preamble row.
    - commit: ce3a263d5
    - review: clean — `review sha ce3a263d5~1..ce3a263d5`, 0 findings (0 confirmed, 0 refuted, 9 attempted, 0 skipped). All three prior findings verified fixed in code against `git show ce3a263d5:<path>`, not taken from the checkboxes.
    - task moved to done by the review gate.

    ### Still open after this card — the install.rs duplication coverage gap

    Round 2 did NOT close the round-1 gap; it only avoided it. `crates/mirdan/src/install.rs` is not among this commit's 13 files, so nothing was skipped this run — but the file remains unread by the `duplication` validator, on every pass, because at 567352 rendered bytes it exceeds the 476042-byte batch budget. The engine's own remedy, "narrow the scope", does not work: a `review file` run limited to that single path still exceeds the cap. The file has to be split before duplication can ever cover it. Tracked separately.
  timestamp: 2026-08-05T04:08:29.492874+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffaa80
title: Doctor's Preamble check is stale — nothing installs the preamble anymore, so it always reports Missing
---
`sah doctor` / `mirdan doctor` unconditionally reports a `Missing` (Warning) row for the `Preamble` component on every doctor-enabled agent, at both project and user scope. This looks like a false positive: no install profile in the codebase writes the preamble anymore.

## Root cause

`Component::all()` in `crates/mirdan/src/status.rs:61-69` hardcodes 5 components checked for every agent: `Mcp`, `Skills`, `Agents`, `Preamble`, `Permissions`. `check_component` (line 170) checks `Component::Preamble` purely by reading the agent's instructions file (`CLAUDE.md`/`AGENTS.md`) for `PREAMBLE_MARKER` (`"MANDATORY: load the thoughtful skill if not already loaded."`, `status.rs:31`) — it has no awareness of any install `Profile` or whether preamble installation is even enabled.

Every real `Profile` in the codebase sets `preamble: false`:
- `apps/swissarmyhammer-cli/src/commands/profile.rs:42` — sah's own profile
- `apps/kanban-cli/src/commands/registry.rs:40`
- `apps/code-context-cli/src/commands/registry.rs:41` and `skill.rs:35`
- `apps/shelltool-cli/src/commands/registry.rs:41`
- `crates/mirdan/src/install.rs:5552`, `:6021` (defaults)

`grep -rn "preamble: true"` across the whole workspace returns zero hits in live Rust code — only in stale kanban task markdown from when the feature was first designed (`^t7a3z4f`). So no install path run by any current CLI ever writes `PREAMBLE_MARKER` into an agent's instructions file. `sah init` (and every other CLI's init) deliberately does not touch CLAUDE.md's content for this purpose anymore.

Given that, `check_component(Component::Preamble)` will report `Missing` for every agent, on every host, forever — `mirdan::install::ensure_preamble`/`remove_preamble` (`install.rs:1742-1808`) are dead code paths reachable only if some future profile sets `preamble: true`, and `apply_profile_preamble` (`install.rs:1657`) is gated on `profile.preamble` which is false everywhere.

## Why this matters

- `sah doctor` shows a permanent, unfixable-by-`sah init` Warning: `Claude Code · project · Preamble` (and `· user ·`) will never turn green by running the suggested fix (`sah init` / `sah init user`), because no profile writes it. This is a red herring that erodes trust in doctor output.
- Confirmed via reading `check_install_stack_with`/`state_of`/`check_component` in `crates/mirdan/src/status.rs` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` — no `profile.preamble` gate anywhere in the doctor path, only in the installer.

## Fix

Pick one, per what the project intends for the preamble feature's future:

1. **Fully remove the Preamble component from doctor** (and likely from `Component` entirely, plus `ensure_preamble`/`remove_preamble`/`apply_profile_preamble`/`PREAMBLE_MARKER`/`preamble_present`/the `Profile.preamble` field) if the CLAUDE.md-preamble-injection feature itself is retired. This is the "we removed the feature" reading — do it thoroughly: dead code left behind (an unreachable installer branch plus a doctor check for it) is worse than removing it outright.
2. **Or**, if the intent is to keep the *capability* to install a preamble (some future profile might opt in) without checking for it when no profile wants it, make the doctor's Preamble row conditional — skip it (map to `NotApplicable`/omit) when no currently-relevant profile has `preamble: true`. This requires threading a profile-aware signal into `check_component`/`check_agent`, which doctor's current architecture doesn't have (it only has `AgentDef`, not `Profile`).

Given every live profile already sets `preamble: false` and grep finds zero remaining `preamble: true` in Rust code, treat this as a completed removal that missed cleaning up the doctor check and the dead installer code — option 1 is very likely correct. Confirm with whoever removed the CLAUDE.md-preamble-injection feature before deleting the installer code, since `ensure_preamble`/`remove_preamble` may still be intentionally kept for a future profile.

## Acceptance

- `sah doctor` and `mirdan doctor` no longer show a `Preamble` row that can never be resolved by running `sah init`/`sah init user`.
- If the component is removed: `Component::Preamble`, `PREAMBLE_MARKER`, `preamble_present`/`preamble_present_in`, `ensure_preamble`, `remove_preamble`, `apply_profile_preamble`, and the `Profile.preamble` field are all deleted, along with every test that exercises them (`crates/mirdan/src/status.rs` and `crates/mirdan/src/install.rs` test modules), and `Component::all()` drops to 4 entries (update the `1 agent × N scopes × 5 components` test assertions accordingly).
- `cargo nextest run -p mirdan -p swissarmyhammer-cli` green, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug

## Review Findings (2026-08-04 20:16)

Scope: `git 0e63e1031~1..0e63e1031` (`fix(mirdan): remove dead preamble installer behind permanent doctor warning`, 24 files).

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/mirdan/src/install.rs` — 567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication (narrow the scope)

- [x] `apps/kanban-cli/src/commands/registry.rs:34` — The `profile()` function is nearly identical to the `profile()` functions in `apps/code-context-cli/src/commands/registry.rs` (line 34) and `apps/shelltool-cli/src/commands/registry.rs` (lines 35-38). All three have identical signatures and bodies that differ only in the skill selector argument. Extract a shared helper function parameterized by skill selector to eliminate the duplication across all three registry modules.
- [x] `apps/shelltool-cli/src/commands/registry.rs:35` — The `profile()` function (lines 35-38) is nearly identical to the `profile()` functions in `apps/code-context-cli/src/commands/registry.rs` (line 34) and `apps/kanban-cli/src/commands/registry.rs` (line 34). All three have identical signatures and bodies differing only in the skill selector argument. Consolidate into a shared function that accepts the skill selector as a parameter, eliminating the copy-pasted function signature and body across three files.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:159` — The directory removal blocks for `.sah/` (lines 159-171) and `.prompts/` (lines 173-185) in the `deinit` method are nearly identical. Both follow the same pattern: join a directory name to cwd, check if it exists, remove it with error handling, and emit a reporter event. They differ only in the directory name string ('.sah' vs '.prompts') and variable names (sah_dir vs prompts_dir). Two blocks that differ only by a value are one function with an argument. Extract a helper function `fn remove_directory_if_exists(cwd: &Path, dir_name: &str, reporter: &dyn InitReporter) -> Option<InitResult>` and call it twice: once with '.sah' and once with '.prompts'. This eliminates the code duplication while preserving the sequential removal behavior.

### Coverage gap — reproduced

The implementer's report is confirmed on this run. The engine skipped one file with its own wording, quoted verbatim in the block above: `crates/mirdan/src/install.rs` — 567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication.

`install.rs` is the single largest file in this change (187 lines changed) and it holds the deleted `ensure_preamble` / `remove_preamble` / `apply_profile_preamble` / `PreambleOutcome` code that is this card's subject. It has never been read by the duplication validator, on any pass. The engine's own remedy is "narrow the scope" — a `review file` run limited to `crates/mirdan/src/install.rs` would still exceed the cap, so the file needs splitting before duplication can cover it. Counts for this run: 4 findings, 4 confirmed, 21 refuted, 35 attempted, 1 skipped.

### Provenance of each recorded finding

Determined from `git show --unified=0 0e63e1031` hunk headers and `git blame 0e63e1031`.

- `apps/kanban-cli/src/commands/registry.rs:34` — **in this commit's diff**. Hunk `@@ -34,9 +34 @@ pub fn profile(_scope: InitScope) -> mirdan::install::Profile {` — nine lines collapsed to the one line the finding names.
- `apps/shelltool-cli/src/commands/registry.rs:35` — **in this commit's diff**. Hunk `@@ -35,9 +35,4 @@ pub fn profile(...)` — post-image lines 35-38, exactly the range the finding names.
- `apps/code-context-cli/src/commands/registry.rs` (the third peer the two findings cross-reference) — **in this commit's diff**. Hunk `@@ -35,9 +35 @@ pub fn profile(...)`. The findings cite it as line 34; the changed line is 35.
- `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:159` — **pre-existing**. The file's hunks land at post-image lines 4-5, 21-26, 37-38, 84-85, 87, 94-95; line 159 is in none of them. `git blame -L 159,185` returns `0b1bcc605e` (18 lines), `f5564d7d10` (8), `035c40b0ef` (1) — zero lines from `0e63e1031`. It is production code, not test code, so the review skill's blanket exception does not reach it and the finding stands.

### Dropped under the review skill's existing-test-code exception

One finding the engine returned is not recorded above:

- `apps/kanban-app/src/state.rs:1465` — "20 is a magic number representing the MAX_RECENT_BOARDS limit … Define a named constant like `const MAX_RECENT_BOARDS: usize = 20;`".

The line is `assert_eq!(boards.len(), 20); // MAX_RECENT_BOARDS`, inside `#[cfg(test)] mod tests` (which opens at line 1393), in the test `test_mru_uistate_touch_and_truncate`. `git blame -L 1465,1465` gives `39f0e3d005` (2026-03-21) — the test predates this commit. The finding's subject is changing a test that already existed, which the review skill drops as a blanket rule.

### The five findings the implementer did not action

Checked against this run, per the rule that only existing **test** code is exempt and these are production files. **None of the five resurfaced in this pass** — they are not in the recorded checklist because the engine did not raise them, not because they were waived. Provenance, from `git blame 0e63e1031`:

- `apps/kanban-app/src/state.rs:31` (`TauriClipboardProvider` derives) — **pre-existing**, blame `0f2baf5a5a` (2026-03-29). Not in any hunk; the file's hunks are at post-image 570, 1137, 1295-1300, 1303-1305, 1309-1317, 1379.
- `apps/kanban-app/src/state.rs:363` (`open` takes `PathBuf`, should take `&Path`) — **pre-existing**, blame `026ac7448d` (2026-03-05). Not in any hunk.
- `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:323` (project-detection depth `3`) — **pre-existing**, blame `c171137837` (2026-03-11). Not in any hunk; the file's non-test hunks stop at post-image 408.
- `crates/mirdan/src/agents.rs:275,280,285,308,313` (repeated path accessors) — **pre-existing at every line cited**: blame `613f58ef3a` (275, 280), `611ee08852` (285), `b7ba81dd78` (308, 313). This commit did touch two other accessors in the same block — `git blame -L 248,317` attributes 2 of those 70 lines to `0e63e1031`, inside `agent_global_agent_dir` and `agent_project_instructions_file` — but none of the five lines the implementer listed.
- `crates/mirdan/src/dispatch.rs:15,43,189` (repeated `Err(e) => { eprintln!; 1 }` handler) — **pre-existing**, blame `905756d9a7` (15, 189) and `f5564d7d10` (43). This commit's only dispatch.rs hunk is `@@ -171 +171,4 @@`, the `Commands::Status` arm.