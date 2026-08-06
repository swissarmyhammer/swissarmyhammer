---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzbp5evp5tk3dherfhdnj77z
  text: |-
    ### research — done

    Discoveries:
    - Contract: Doctor section of builtin/validators/README.md. Doctor reports (1) detected project types, (2) each validator set + applies flag, (3) each tool rule for detected project types: tool present/missing, version, fixture result, (4) tool rules on prompt fallback + install commands. No builtin tool rules exist yet, so on this repo the tool-rule list is empty; tests use synthetic rulesets.
    - Pattern to copy: mirdan::status (crates/mirdan/src/status.rs) — library produces fact structs, `to_check`/`statuses_to_checks` converts to swissarmyhammer_doctor::Check, CLI checks.rs has a thin loader wrapper + `_with` test seam (checks.rs:417-463).
    - Fact source pieces: ToolSpec/ToolDoctor/ToolInstall in crates/swissarmyhammer-validators/src/validators/types.rs (~740-800); `detected_project_type_keys` is private in review/scope.rs:665 (make pub(crate) and reuse); `parse_tool_stdout` in review/tool_output.rs parses runner stdout; loader `list_rulesets()` + `load_rules()` gives the full set stack; RuleSet.base_path is a real dir (fixtures live at base_path/fixtures).
    - `categorize_check` in swissarmyhammer-doctor/src/display.rs already maps names containing "Validator" to a Validator category — new check names carry that word.
    - CLI does not depend on swissarmyhammer-validators yet; add the dep. swissarmyhammer-validators gets a swissarmyhammer-doctor dep (mirrors mirdan).

    Plan: new module crates/swissarmyhammer-validators/src/doctor.rs with ReviewEngineStatus facts (project_types, SetStatus, ToolRuleStatus with ToolPresence/FixtureOutcome), `check_review_engine(root)` + `check_review_engine_with(loader, types)` seam + `to_checks`. Fixture semantics: run script with cwd = fixtures dir, Files scope gets fixture filename as "$@"; fail fixture needs >=1 finding, pass fixture 0; nonzero exit or bad stdout = Failed; a rule that fails fixtures or lacks the tool is on prompt fallback. Wire into sah doctor pipeline as run_review_engine_checks.
  timestamp: 2026-08-06T14:03:19.798298+00:00
- actor: claude-code
  id: 01kzbpvefd041e305s4sg2y8t1
  text: |-
    Implementation landed. What was built:

    - `crates/swissarmyhammer-validators/src/doctor.rs` (new): the `mirdan::status`-style fact producer. `ReviewEngineStatus { project_types, sets: Vec<SetStatus>, tool_rules: Vec<ToolRuleStatus> }`, with `ToolPresence` (Present/Missing) and `FixtureOutcome` (Passed/Failed/MissingFixtures/Skipped). `check_review_engine(root)` loads the full stack via `load_rules()` + `detected_project_type_keys`; `check_review_engine_with(loader, types)` is the injectable test seam; `to_checks()` converts to doctor `Check` rows named "Validator Project Types" / "Validator Set · <name>" / "Validator Tool Rule · <set>/<rule>" (the "Validator" word gets the free category in `categorize_check`). A missing tool or failed fixture is a Warning with `install.commands` as the fix — a degraded review never blocks.
    - Fixture semantics: script runs with cwd = the set's `fixtures/` dir; `files` scope gets the fixture file name as `"$@"`, `workspace` scope no args; fail fixture needs >= 1 finding (via `parse_tool_stdout`), pass fixture needs 0; nonzero exit or contract-breaking stdout = Failed and the rule is not used.
    - `ValidatorMatch::project_types_match` (types.rs): public project-type-only evaluation; the private `matches_project_types` now delegates to it. `detected_project_type_keys` widened to `pub(crate)` for reuse (scope.rs).
    - CLI: `check_review_engine` in doctor/checks.rs (workspace root = git root, else cwd; load failure = one Error row) + `run_review_engine_checks` wired into `run_diagnostics_without_output`. New deps: validators -> swissarmyhammer-doctor, cli -> swissarmyhammer-validators.

    Verified on this repo: `sah doctor` shows "Validator Project Types: nodejs, rust" and one applicability row per set. No tool-rule rows appear because no builtin tool rules exist yet (they arrive with ^cbnfe97-era rules); the PATH-flip acceptance (missing binary -> Warning row + install commands) is proven by `test_missing_tool_reports_install_commands_and_skips_fixtures` and `test_to_checks_missing_tool_is_warning_with_install_fix`.

    Note for the record: ARCHITECTURE.md's Doctor Pattern section ("don't add health checks outside the Doctorable trait") predates the mirdan::status fact-producer pattern that `check_install_stack` already uses and this card mandates; the section needs an update to describe both paths.
  timestamp: 2026-08-06T14:15:20.301527+00:00
- actor: claude-code
  id: 01kzbpvntbqfns4bjmtfbtxns5
  text: |-
    ### implement — changed
    - evidence: 8 files — crates/swissarmyhammer-validators/src/doctor.rs (new), crates/swissarmyhammer-validators/src/lib.rs, crates/swissarmyhammer-validators/src/validators/types.rs, crates/swissarmyhammer-validators/src/review/scope.rs, crates/swissarmyhammer-validators/Cargo.toml, apps/swissarmyhammer-cli/src/commands/doctor/checks.rs, apps/swissarmyhammer-cli/src/commands/doctor/mod.rs, apps/swissarmyhammer-cli/Cargo.toml. TDD RED->GREEN: 12 new doctor.rs tests + 2 new CLI tests. `cargo nextest run -p swissarmyhammer-validators`: 417 passed. `cargo nextest run -p swissarmyhammer-cli`: 688 passed. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-cli --all-targets -- -D warnings`: clean. `cargo fmt` run. Manual acceptance: `sah doctor` on this repo shows "Validator Project Types: nodejs, rust" + per-set applicability rows.
    - next: test
  timestamp: 2026-08-06T14:15:27.819484+00:00
- actor: claude-code
  id: 01kzbq3yzwdw6gzq1vbvcm86w3
  text: |-
    Check-in from the watching session (2026-08-06):

    1. ^q4909tf verified done against the contract — ToolSpec is exactly scope/run/doctor/install with deny_unknown_fields, stdout parser covers both line shapes. Good foundation.

    2. Your ARCHITECTURE.md note is now card ^rd1egpe (document the fact-producer doctor path beside Doctorable). Do not fold it into this card.

    3. Your fixture semantics (cwd = fixtures dir, files scope gets the fixture name as "$@", fail >= 1 / pass = 0 via parse_tool_stdout, nonzero exit = Failed and rule unused) match the README contract. One forward pointer: ^b01gtzg ships the first real fixtures — when it lands, the "Fixture checks pass in doctor" acceptance there exercises your FixtureOutcome path end to end on this repo.
  timestamp: 2026-08-06T14:19:59.356045+00:00
- actor: claude-code
  id: 01kzbrmjxj3svw1n5951axgvgc
  text: |-
    ### review — findings
    - evidence: 8 findings kept of 11 confirmed (engine counts: 11 confirmed, 28 refuted) on `review sha HEAD~1..HEAD` (a3ec46719) — crates/swissarmyhammer-validators/src/doctor.rs:410, src/lib.rs:29, src/lib.rs:30, src/lib.rs:31, src/lib.rs:32, src/lib.rs:33, src/validators/types.rs:298, src/validators/types.rs:374. 3 findings dropped by the review skill rule: they ask for a refactor of test code that existed before this commit (types.rs:1019, 1487, 1603).
    - next: implement the 8 open findings, then run /review ^2hk89aj again
  timestamp: 2026-08-06T14:46:32.626453+00:00
- actor: claude-code
  id: 01kzbrn5zr1mk2wygxfxaz8b0c
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 8 files (validators: doctor.rs new, lib.rs, types.rs, review/scope.rs, Cargo.toml; cli: doctor/checks.rs, doctor/mod.rs, Cargo.toml)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3155 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: a3ec46719
    - review: findings — 8 open: doctor.rs:410 (named constant), lib.rs:29-33 (module doc comments), types.rs:298 (apply_defaults duplicates), types.rs:374 (matches near-duplicates)
  timestamp: 2026-08-06T14:46:52.152802+00:00
- actor: claude-code
  id: 01kzbsa8394tsjmn110y4wdjrs
  text: |-
    Worked all 8 review findings:

    - doctor.rs — the capacity increment `1` is now the named constant `PROJECT_TYPES_ROWS`, defined with the other module constants and used in `to_checks`.
    - lib.rs — all five public modules (`builtin`, `doctor`, `error`, `review`, `validators`) now carry doc comments at their declarations. The `doctor` comment names the status API: `check_review_engine` -> `ReviewEngineStatus` -> `to_checks`.
    - types.rs — `ValidatorFrontmatter::apply_defaults` and `RuleFrontmatter::apply_defaults` now call one shared helper, `apply_name_and_description(name, description, path, kind)`. `Validator::matches` and `RuleSet::matches` now call one shared helper, `matches_trigger_and_criteria(trigger_matcher, match_criteria, owner, ctx)`. The now-unused private method `Validator::matches_trigger_regex` is deleted.

    Discovery for the next agent: outer doc comments on a `pub mod` declaration make rustdoc re-resolve the module's inner `//!` doc links in the parent (crate-root) scope. Six inner links did not resolve there and produced new `cargo doc` warnings. Fix: give those links explicit crate-anchored targets, e.g. `[`Check`](swissarmyhammer_doctor::Check)` in doctor.rs and `[`probes`](crate::review::probes)` in review/mod.rs, plus `[`ValidatorPartialAdapter`](crate::validators::ValidatorPartialAdapter)` in validators/mod.rs. The explicit target does not trip the redundant-explicit-link lint in this dual-scope case. Verified with a warning-list diff: the `cargo doc` warning set is byte-identical to the pre-change baseline (35 pre-existing warnings, zero new).
  timestamp: 2026-08-06T14:58:22.441312+00:00
- actor: claude-code
  id: 01kzbsaejk9vsgq6szwpca1eq9
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-validators/src/doctor.rs (PROJECT_TYPES_ROWS constant + inner-doc link anchor), src/lib.rs (doc comments on the 5 public modules), src/validators/types.rs (shared apply_name_and_description + shared matches_trigger_and_criteria, private matches_trigger_regex method deleted), src/validators/mod.rs and src/review/mod.rs (crate-anchored inner-doc links so the new module docs add zero cargo doc warnings). All 8 findings flipped to [x]. cargo nextest run -p swissarmyhammer-validators: 417 passed. cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-cli --all-targets -- -D warnings: clean. cargo fmt run. cargo doc warning set identical to baseline.
    - next: test
  timestamp: 2026-08-06T14:58:29.075317+00:00
depends_on:
- 01KZ934SNEJ1TXNS2G9Q4909TF
position_column: doing
position_ordinal: '8280'
title: Make the review engine doctorable per project and project type
---
`sah doctor` must report the review engine state for the current project.

The contract is the Doctor section of `builtin/validators/README.md`.

Work:
- Report the detected project types (from PROJECT_TYPE_SPECS detection).
- Report each validator set and whether it applies to this project.
- For each runner of a detected project type: tool present or missing, tool version (`check_version_command`), and fixture result.
- Run each available runner against its `fixtures/*.fail.*` and `fixtures/*.pass.*` files. A runner that fails its fixtures is reported and not used.
- Report each language on the prompt fallback, with the `install.commands` to fix it.
- Follow the agent-agnostic status pattern: `mirdan::status` style facts that `sah doctor` and `mirdan doctor` both consume.

Acceptance:
- On this repo, doctor shows the rust project type, the applicable sets, and each runner row.
- Removing a tool from PATH flips its row to missing with install commands shown.

#tool-validators

## Review Findings (2026-08-06 09:22)

- [x] `crates/swissarmyhammer-validators/src/doctor.rs:410` — Hardcoded capacity increment `1` for the project-types row should be a named constant to prevent off-by-one errors if the count of always-included rows changes. Define `const PROJECT_TYPES_ROWS: usize = 1;` near the top of the module (e.g., near line 35 with other constants) and use it: `Vec::with_capacity(PROJECT_TYPES_ROWS + status.sets.len() + status.tool_rules.len())`.
- [x] `crates/swissarmyhammer-validators/src/lib.rs:29` — Public module `builtin` has no doc comment. All public items require documentation. Add a doc comment explaining the module's purpose.
- [x] `crates/swissarmyhammer-validators/src/lib.rs:30` — Public module `doctor` has no doc comment. All public items require documentation. Add a doc comment explaining the module's purpose and how to use the review-engine status API.
- [x] `crates/swissarmyhammer-validators/src/lib.rs:31` — Public module `error` has no doc comment. All public items require documentation. Add a doc comment explaining the module's purpose.
- [x] `crates/swissarmyhammer-validators/src/lib.rs:32` — Public module `review` has no doc comment. All public items require documentation. Add a doc comment explaining the module's purpose.
- [x] `crates/swissarmyhammer-validators/src/lib.rs:33` — Public module `validators` has no doc comment. All public items require documentation. Add a doc comment explaining the module's purpose.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:298` — ValidatorFrontmatter::apply_defaults (lines 298-299) and RuleFrontmatter::apply_defaults (lines 846-847) are verbatim duplicates that differ only by the string literal passed to default_description. Both call the same two helper functions in the same sequence with different kind strings ('Validator' vs 'Rule'). This is code that could drift if the initialization pattern needs to change. Extract a shared method that takes the kind string as a parameter: `fn apply_name_and_description(&mut self, path: &Path, kind: &str)` that both frontmatter types call, eliminating the duplication.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:374` — Validator::matches and RuleSet::matches are near-verbatim duplicates. Both follow identical logic: check trigger_matcher, check match_criteria, return success. They differ only in field access (frontmatter vs manifest) and the method vs function call pattern for trigger_matcher. This is one function with type-specific field access that could drift if bugs are fixed in one place but not the other. Extract a shared helper function that accepts the trigger_matcher and match_criteria as parameters, or implement a trait that both Validator and RuleSet conform to, so the matching logic lives in one place.

Note: The engine reported 3 more findings about `timeout` constants in the test module of `types.rs` (lines 1019, 1487, 1603). That test code existed before this commit. The review skill rule drops findings that ask for a refactor of tests that already existed. These 3 findings are dropped for that written rule.