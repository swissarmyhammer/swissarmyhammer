---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzcf3d6vq172ac2pf45ep7xx
  text: |-
    Research done.

    Facts found:
    - The tool-rule health decision lives once in `crates/swissarmyhammer-validators/src/doctor.rs` (`check_tool_rule` / `check_presence` / `run_shell`). The planner `review/tool_rules.rs::plan_rule_by_health` calls it. The install lifecycle must reuse `check_presence` so "installed" can never mean two things.
    - `run_review` (`review/synthesize.rs`) calls `plan_tool_rules` in stage 2, after scope and before batching. That is where an install stage goes: install first, then plan — the planner re-runs the doctor check, so a tool installed by the lifecycle is picked up with no extra plumbing.
    - No builtin tool rule ships yet (`grep -rn "^tool:" builtin/validators/` is empty). Card ^b01gtzg ("missing-docs runners for Rust and Python") adds the first ones. So version pinning must land as a guard over the builtin stack, not as an edit to spec files that do not exist.
    - `sah init` applies `sah_profile()` (`apps/swissarmyhammer-cli/src/commands/profile.rs`) through `mirdan::install::init_profile`, plus the `Initializable` components in `commands/registry.rs::register_all`.

    Placement decision for the "register the runner tools so `sah init` / mirdan install can pre-install them" bullet:
    - `mirdan` does NOT depend on `swissarmyhammer-validators`, and it must not: mirdan is the shared installer for every tool CLI (kanban, code-context, shelltool), so that edge would link the whole review engine (claude-agent, rusqlite, model-embedding, git2) into all of them.
    - mirdan cannot read a tool rule's `tool.install.commands` without that edge, so a `Profile` field would be data mirdan can neither fill nor apply.
    - The registration therefore lands on the seam `apps/swissarmyhammer-cli/src/commands/install/components/` already documents for "concerns not expressible as profile data": a `ValidatorTools` component that `register_all` registers, so `sah init` pre-installs the runner tools alongside the profile. One implementation of the lifecycle, no new crate edges.
  timestamp: 2026-08-06T21:19:06.971296+00:00
- actor: claude-code
  id: 01kzcg3da2gcrkxp72veqabwt5
  text: |-
    Implementation landed.

    What was built:
    - `crates/swissarmyhammer-validators/src/review/tool_install.rs` (new) — the whole install lifecycle. `install_tool_commands` runs each `install.commands` entry in order and re-runs the doctor check after each; a command that exits 0 and leaves the check failing is NOT a success. `ensure_tool_installed` adds one bounded agent turn when every command failed, then re-runs the doctor check — the agent's answer decides nothing. `ToolInstallAgent` is the seam; `PoolInstallAgent` is the production impl over the review's shared `AgentPool` (one turn, the pool's token cap, idle window, and turn ceiling bound it).
    - `review/tool_rules.rs` — extracted `matched_tool_rules`, the ONE matching pass. `plan_tool_rules` plans from it and `install_missing_tools` installs from it, so the engine can never install a tool for a rule it will not run.
    - `review/synthesize.rs` — `run_review` stage 2a installs before planning. The planner re-runs the same doctor check, so an installed tool is planned healthy and a still-missing one falls back on its own; no extra plumbing.
    - `doctor.rs` — extracted `project_tool_rules`, the ONE selection pass over the loader. `check_review_engine_with` diagnoses those rules and `install_project_tool_rules` installs their tools, so doctor can never report a rule the installer skipped. `check_presence` is now `pub(crate)` so the lifecycle and doctor agree on "installed".
    - Version pinning: `install_command_pins_version` plus a guard test over the whole builtin stack. No builtin tool rule ships install commands yet, so the guard becomes a hard requirement the moment ^b01gtzg adds the first ones.
    - `sah init` pre-install: the `ValidatorTools` `Initializable` component (priority 60), registered in `commands/registry.rs`. It runs the deterministic half only — `sah init` never spends an agent turn. A tool it cannot install is a Warning naming the rule, never an Error. `deinit` removes nothing: these are shared developer tools.

    Discoveries worth keeping:
    - A Rust `\`-continued string literal strips the NEXT line's leading whitespace. A test fixture built that way silently flattened the YAML nesting, so `install:` parsed as a top-level rule key instead of a `tool` key and the install list vanished. `RuleFrontmatter` has no `deny_unknown_fields`, so the misplaced key was accepted in silence. Build rule fixtures with raw multi-line literals.
    - RED verified by mutation, not by assumption: making the loop trust the command exit code and making the agent's answer decide broke 6 of the 13 lifecycle tests, each for its own reason. Reverted afterwards.

    Real-pipeline coverage:
    - `the_pool_install_agent_runs_a_real_turn_whose_claim_still_loses_to_doctor` drives a real `AgentPool` over a real ACP connection to a scripted agent that claims success; the doctor check still refuses it.
    - The two acceptance tests live in `tool_rules.rs` and run install → plan together over a real work-list and the real planner.
    - The `sah init` component tests build the rule through the real validator parser, not a hand-made struct.

    Tests: `swissarmyhammer-validators --lib` 449 passed, `swissarmyhammer-cli --lib` 295 passed, `swissarmyhammer-tools` review smoke + progress tests passed, `cargo check --workspace --all-targets` clean, clippy clean, fmt clean.
  timestamp: 2026-08-06T21:36:35.650970+00:00
- actor: claude-code
  id: 01kzcg3nw6jvk1phfme3dsnsxx
  text: |-
    ### implement — changed
    - evidence: 10 files — crates/swissarmyhammer-validators/src/review/tool_install.rs (new), review/tool_rules.rs, review/synthesize.rs, review/scope.rs, review/mod.rs, doctor.rs, apps/swissarmyhammer-cli/src/commands/install/components/mod.rs, apps/swissarmyhammer-cli/src/commands/registry.rs, builtin/validators/README.md, ARCHITECTURE.md. Tests: validators --lib 449 passed, cli --lib 295 passed, workspace check + clippy + fmt clean.
    - next: /review
  timestamp: 2026-08-06T21:36:44.422267+00:00
- actor: claude-code
  id: 01kzcgxr6cwzf6gjthvmf41tka
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13650 tests run, 13650 passed, 0 failed, 0 skipped; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
    - fix applied: `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` — `WARM_UP_MAX_ATTEMPTS` raised from 20 to 120 (10s of poll budget to 60s). Two tests (`follower_multi_step_rename_gets_real_leader_edits_under_one_lock`, `follower_request_with_document_gets_real_definition_without_leader_preopen`) failed on the first full-workspace run: each drives a real `rust-analyzer` and polls until it warms up, but the old ~13s total budget (3s initial settle + 20×500ms poll) was tuned for an idle machine. Under full `--workspace` parallelism (13650 tests across the whole suite contending CPU) the analyzer stayed cold past that window even though the `lsp-ipc-serial` nextest test-group already caps this crate's LSP tests to one real analyzer at a time — the contention is from every other package's tests sharing the CPU, not a second analyzer. The sibling test `ra_pull_readiness.rs` in the same crate already uses a 60s deadline for the identical "real rust-analyzer, cold under CI load" scenario, so the fix matches that existing, already load-tolerant precedent. Verified: isolated run passed before and after; the full `cargo nextest run --workspace` after the fix is 13650/13650 green with 0 failures.
    - next: none.
  timestamp: 2026-08-06T21:50:58.764397+00:00
- actor: claude-code
  id: 01kzchk5n1ykwbzrdne22z2d8z
  text: |-
    Check-in from the watching session (2026-08-06):

    Verified f55618b02 against the contract — every invariant lands: install.commands in order with a doctor re-check after each, exit-0-but-check-fails is not success, the install agent is one bounded turn whose claim loses to doctor (the real-AgentPool test proving that is exactly right), still-missing tools fall back with a warning, and the version-pin guard arms itself when ^b01gtzg ships the first builtin tool rules.

    Two deviations from the card's letter, both ACCEPTED as better than written:
    1. Registration via the ValidatorTools Initializable component instead of the mirdan Profile manifest. The dependency argument is decisive — mirdan must not link the review engine into every tool CLI. The card's intent (sah init pre-installs runner tools) is fully met. Reviewers: do not flag this as scope miss.
    2. Your README addition (sah init runs steps 1-2 only, never an agent turn; failure = warning naming the rule) is adopted as contract text.

    Nothing to fix from my side.
  timestamp: 2026-08-06T22:02:40.673771+00:00
- actor: claude-code
  id: 01kzcj6et7anym5887p0yfa78w
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (f55618b02) — 4 findings, 4 confirmed, 5 refuted, 18 attempted, 0 failed, 0 skipped. 3 recorded: apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:239, crates/swissarmyhammer-validators/src/doctor.rs:206, crates/swissarmyhammer-validators/src/doctor.rs:249.
    - dropped: 1 finding on crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:147 (name the literal `8` as a constant). The file is pre-existing test code modified by this commit only at `WARM_UP_MAX_ATTEMPTS`; the `0..8u32` loop was already there. The review skill's blanket exception drops any finding that asks to restyle test code that already existed.
    - no over-prompt-cap or batch-budget flags in this run, so nothing to log on ^tsram0q.
    - next: /implement the 3 findings, then re-review.
  timestamp: 2026-08-06T22:13:12.647901+00:00
- actor: claude-code
  id: 01kzcj72h5vmmf8hfzswr6c0rf
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 13 files, new crates/swissarmyhammer-validators/src/review/tool_install.rs
    - test: green — cargo nextest run --workspace 13650 passed, fmt clean, clippy clean
    - commit: f55618b02
    - review: findings — apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:239, crates/swissarmyhammer-validators/src/doctor.rs:206, crates/swissarmyhammer-validators/src/doctor.rs:249
  timestamp: 2026-08-06T22:13:32.837967+00:00
- actor: claude-code
  id: 01kzcjqrjyrf90pm0rf7pxr530
  text: |-
    Findings pass 1 — all three closed.

    Finding 1 (components/mod.rs:239, duplicated workspace-root error handling): added the module-level `resolve_workspace_root(component: &str) -> Result<PathBuf, Vec<InitResult>>` beside `workspace_root`, and deleted `ProjectStructure::root_or_error`. All three sites that resolve the root — `ProjectStructure::init`, `ProjectStructure::deinit`, `ValidatorTools::init` — now call it, so the failure-to-lifecycle-result shape lives in one place. `workspace_root`'s doc names `ValidatorTools::init` as a third caller that must agree on the root.

    Finding 2 (doctor.rs:206, duplicated `list_rulesets()` + `sort_by(name)`): the correct fix is to delete the sort, not to extract it. `ValidatorLoader::rulesets` is a `BTreeMap` keyed by `ruleset.name()` (both insert sites use that key), so `list_rulesets()` ALREADY enumerates in name order and both sorts were no-ops. Extracting a `sorted_rulesets` helper would have kept dead work and split the ordering guarantee across two modules once finding 3 moved `project_tool_rules` out of doctor.rs. Instead the order is centralized where it is decided: `list_rulesets`'s doc now states the name-order contract and that callers must read the order from there and never re-sort. `grep` confirms no `sort_by(|a, b| a.name()...)` on rulesets remains anywhere in the workspace.

    Finding 3 (doctor.rs:249, selection pass must live in one place): `ProjectToolRule` and `project_tool_rules` moved verbatim into `review/tool_rules.rs`, beside `MatchedToolRule` / `matched_tool_rules`. `doctor::check_review_engine_with` and `tool_install::install_project_tool_rules` now import the same function from that one home. The module doc names it as the third selection — the workspace-wide one, for the surfaces with no work-list.

    Shared predicate: `criteria_applies` was used by BOTH the moved selection and doctor's set-applicability row, so parking it in either module would have re-split it. It became `ValidatorMatch::criteria_applies(Option<&Self>, &[String])` in `validators/types.rs`, next to the `project_types_match` it wraps — the canonical home for "does this match criteria apply".

    New coverage: `project_tool_rules_reports_the_sets_in_name_order` loads two sets last-name-first and asserts the selection comes back in name order. It is the guard the deleted sort used to be; the sets side was already guarded by `test_sets_are_sorted_by_name`.

    Nothing changed in ARCHITECTURE.md: it names the public re-export `swissarmyhammer_validators::review::install_project_tool_rules`, which is untouched, and the documented mirdan-must-not-depend-on-the-review-engine boundary is unaffected — the move is entirely inside the review engine.

    Tests: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-cli` 1148 passed, 0 failed, 0 skipped (validators --lib 450, up one for the new test). `cargo check --workspace --all-targets` clean, `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean, `cargo fmt --all --check` clean.
  timestamp: 2026-08-06T22:22:39.710778+00:00
- actor: claude-code
  id: 01kzcjqyetm64r09e54me4c4kh
  text: |-
    ### implement — changed
    - evidence: 6 files — apps/swissarmyhammer-cli/src/commands/install/components/mod.rs, crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/review/tool_install.rs, crates/swissarmyhammer-validators/src/validators/types.rs, crates/swissarmyhammer-validators/src/validators/loader.rs. All 3 review findings checked off. Tests: nextest -p swissarmyhammer-validators -p swissarmyhammer-cli 1148 passed / 0 failed / 0 skipped; workspace check + clippy + fmt clean.
    - next: /review
  timestamp: 2026-08-06T22:22:45.722198+00:00
- actor: claude-code
  id: 01kzckq6de9z9gm1ay9agccet5
  text: |
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (9e9a49f01)
    - prior 3 findings verified closed: `resolve_workspace_root` exists at components/mod.rs:332 and serves all 3 call sites; the duplicate ruleset sorts are gone from doctor.rs and `list_rulesets` documents name order on the BTreeMap; `project_tool_rules` lives once in review/tool_rules.rs:445, imported by doctor.rs:33 and tool_install.rs:427.
    - counts: 7 findings, 7 confirmed, 1 refuted, 18 attempted, 0 failed, 0 skipped.
    - recorded: 3 findings — apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:132, :162, :233 (the residual match block around resolve_workspace_root repeats verbatim at all three sites).
    - dropped under the review skill's existing-test exception: 4 findings on crates/swissarmyhammer-validators/src/validators/types.rs:1500, :1616, :1823, :1844 (named constants for hardcoded timeouts in tests that already existed; this commit added only `criteria_applies` to that file).
    - no over-cap or batch-budget flags in this run, so nothing to log on ^tsram0q.
    - next: close the 3 mod.rs findings, then re-review.
  timestamp: 2026-08-06T22:39:49.678760+00:00
- actor: claude-code
  id: 01kzckqw96gerqmskq1h1b756m
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 6 files; the 3 prior findings are closed and verified
    - test: green — cargo nextest run --workspace 13651 passed, fmt clean, clippy clean
    - commit: 9e9a49f01
    - review: findings — apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:132, mod.rs:162, mod.rs:233 (the match block around the extracted call is repeated verbatim at three sites)
  timestamp: 2026-08-06T22:40:12.070241+00:00
- actor: claude-code
  id: 01kzckzbpvry74cgyx7qm44s3h
  text: |-
    Findings pass 2 — the three mod.rs findings are closed at the root.

    What the last pass got wrong: it extracted only the `map_err` half. `resolve_workspace_root(&str) -> Result<PathBuf, Vec<InitResult>>` returned a `Result` that the caller still had to open, so each of the three call sites kept the same 4-line `match ... { Ok(root) => root, Err(failure) => return failure }` block. Removing a duplicated expression left the duplicated control flow behind. That is the shape the finding names.

    The fix inverts the direction. `resolve_workspace_root` is deleted. In its place:

        fn with_workspace_root(
            component: &str,
            work: impl FnOnce(PathBuf) -> Vec<InitResult>,
        ) -> Vec<InitResult>

    The helper now owns BOTH halves — the resolution and the branch on its failure. Each lifecycle body passes itself in as `work` and never sees a `Result`:
    - `ProjectStructure::init`
    - `ProjectStructure::deinit`
    - `ValidatorTools::init`

    There is exactly one `match` on the root in the file, inside `with_workspace_root`. What repeats at the three sites is the single line `with_workspace_root(self.name(), |root| {` — a forwarding call with no logic in it, which cannot drift. The duplication cannot come back in another shape because there is no branch left at a call site to copy.

    Two doc repairs found while working the file:
    - The doc block above the old `resolve_workspace_root` had absorbed `workspace_root`'s own paragraph, and `workspace_root` was left with no doc at all. Each function now carries its own.
    - `ProjectStructure::init` and `ValidatorTools::init` both said "Root resolution stays here". It does not any more; both now name `with_workspace_root` as where they read the root.

    Behavior is unchanged, and the existing guard proves it: `test_project_structure_round_trips_from_a_subdirectory` drives `init` then `deinit` from a subdirectory of a real repository and asserts both halves target the repository root — it passes through the closure form.

    Tests: `cargo nextest run -p swissarmyhammer-cli` 698 passed, 0 failed, 0 skipped (24 of them the components module). `cargo clippy -p swissarmyhammer-cli --all-targets --all-features -- -D warnings` clean, `cargo fmt --all` clean. `rg resolve_workspace_root` shows no caller left in the CLI; the two remaining hits are an unrelated same-named function in swissarmyhammer-tools/src/mcp/server.rs.
  timestamp: 2026-08-06T22:44:17.243990+00:00
- actor: claude-code
  id: 01kzckzgstscs8ztbxtkyg3k3m
  text: |-
    ### implement — changed
    - evidence: 1 file — apps/swissarmyhammer-cli/src/commands/install/components/mod.rs. `resolve_workspace_root` replaced by `with_workspace_root(component, work)`, which owns the resolve-and-branch control flow; all 3 match blocks removed. All 3 review findings checked off. Tests: nextest -p swissarmyhammer-cli 698 passed / 0 failed / 0 skipped; clippy -D warnings clean; fmt clean.
    - next: /review
  timestamp: 2026-08-06T22:44:22.458962+00:00
depends_on:
- 01KZ935GJX1YS2EAD7C2HK89AJ
position_column: doing
position_ordinal: '8480'
title: Runner install lifecycle with LLM installer fallback
---
Install missing runner tools through the unified mirdan tool-install lifecycle.

The contract is the Install lifecycle section of `builtin/validators/README.md`.

Work:
- On a missing tool: try each entry in `install.commands` in order. Re-run the doctor check after each try.
- Pin tool versions in the builtin runner specs. An unpinned tool can change rules and break the gate.
- If every command fails, spawn a bounded install agent. Inputs: the runner spec, the platform, the error output. Goal: make `doctor.check_command` pass. Doctor confirms the result. The agent cannot assert success.
- If the tool is still missing, the review falls back to the prompt rule and doctor keeps a warning. A missing tool never blocks a review.
- Register the runner tools in the mirdan Profile manifest so `sah init` / mirdan install can pre-install them.

Acceptance:
- With the tool absent and a working install command, the review installs it and runs the runner.
- With all installs failing, the review completes on the prompt fallback and doctor shows the warning.

#tool-validators

## Review Findings (2026-08-06 16:52)

- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:239` — Duplicated workspace-root error-handling pattern: ValidatorTools::init inlines the same `workspace_root().map_err()` error conversion that ProjectStructure has already extracted as the `root_or_error` helper method (lines 90–92). The pattern repeats verbatim, differing only in expression form (match statement vs method wrapper). This creates maintenance burden if the error-handling shape ever changes—both sites must be kept in sync. Extract a module-level helper function `fn resolve_workspace_root<T: Initializable>(component: &T) -> Result<PathBuf, Vec<InitResult>> { workspace_root().map_err(|e| vec![InitResult::error(component.name(), e)]) }` and call it from both ProjectStructure and ValidatorTools. Alternatively, add this as a default method to the Initializable trait (though that lives in another crate). Either approach eliminates the duplicate and makes the error handling single-source.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:206` — Duplicated ruleset-fetching and sorting pattern: `check_review_engine_with` (lines 206–207) and `project_tool_rules` (lines 253–254) both begin with the identical sequence: `let mut rulesets = loader.list_rulesets();` followed immediately by `rulesets.sort_by(|a, b| a.name().cmp(b.name()));`. Both functions need sorted rulesets as their foundation, yet neither extracts this common operation. The duplication creates maintenance burden if the sort order or fetch method ever changes. Extract a module-level helper function `fn sorted_rulesets(loader: &ValidatorLoader) -> Vec<RuleSet> { let mut rulesets = loader.list_rulesets(); rulesets.sort_by(|a, b| a.name().cmp(b.name())); rulesets }` and call it from both check_review_engine_with and project_tool_rules. This eliminates the duplicate and makes the expected ordering explicit and centralized.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:249` — The new `project_tool_rules` function duplicates or closely parallels the existing `plan_tool_rules` function in tool_rules.rs. The comment explicitly states this is 'the ONE selection pass over the loader' — selection logic should exist in exactly one place, not duplicated across modules. Both doctor and installer need to see the same rules to stay in sync; this should be in tool_rules.rs where all tool-rule utilities live. Move the `project_tool_rules` logic into tool_rules.rs (or rename/generalize `plan_tool_rules` if it already exists there) so both doctor and installer import the same function. Ensure the single implementation is at the canonical location for tool-rule utilities, not split across doctor.rs and tool_rules.rs.

## Review Findings (2026-08-06 17:30)

- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:132` — Verbatim duplication: the resolve_workspace_root match pattern is repeated identically at lines 132, 162, and 233. This pattern appears three times across two structs (ProjectStructure and ValidatorTools) with identical error handling logic, inflating surface area for maintenance. Extract a helper function `fn get_initializable_root(component: &dyn Initializable) -> Result<PathBuf, Vec<InitResult>>` that wraps the call to `resolve_workspace_root(component.name())`, then replace all three blocks with a single invocation. Alternatively, check if the `?` operator can be used if the error type is compatible with the function's return type.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:162` — Verbatim duplication: the resolve_workspace_root match pattern is repeated identically at lines 132, 162, and 233. This block is the second occurrence of three identical blocks. Extract shared helper function (see line 132 finding).
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:233` — Verbatim duplication: the resolve_workspace_root match pattern is repeated identically at lines 132, 162, and 233. This block is the third occurrence of three identical blocks across different structs. Extract shared helper function (see line 132 finding).
