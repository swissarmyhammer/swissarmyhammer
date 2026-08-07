---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzedg5589agkjgxxtd2a59y6
  text: |-
    Picked up. Research on the `code-hygiene` tool-rule contract, read from the code, not guessed:

    - `Rule::matches` (crates/swissarmyhammer-validators/src/validators/types.rs) is narrow-only: it returns false unless `ruleset.matches(ctx)` is true first. A rule can never match a file its set does not match.
    - The `code-hygiene` set match is `@file_groups/source_code`. `builtin/file_groups/source_code.yaml` lists source extensions only. It has no `*.toml` and no `Cargo.toml` pattern.
    - `execute_tool_runs` (review/tool_rules.rs) keeps a `workspace`-scope finding only when its normalized path is in the matched changed-file list.
    - `find_fixture` (doctor.rs) accepts a FILE only, named `<rule>.<kind>.<ext>`. A directory fixture is not found.
    - `run_fixture` attributes a finding to the fixture under test by FILE NAME equality after normalization.

    Consequence for any manifest-scoped tool: a finding whose path is `Cargo.toml` can never reach a report from the `code-hygiene` set, under either scope.

    Tool presence on this machine: `cargo-machete` 0.9.2 present. `staticcheck`, `knip`, `vulture`, `periphery` absent.
  timestamp: 2026-08-07T15:29:36.424004+00:00
- actor: claude-code
  id: 01kzedvs3t645x0vsjxextcc6n
  text: |-
    Evaluated all five candidates by running them. Installed what was missing: staticcheck 2025.1.1, vulture 2.14, knip 6.32.0, periphery 3.8.0. cargo-machete 0.9.2 was already present.

    VERDICTS

    1. `cargo machete` — REJECT. Two independent proven reasons.
       - Structural: every machete finding names a `Cargo.toml`. The `code-hygiene` set matches `@file_groups/source_code`, which declares no manifest pattern, and `Rule::matches` is narrow-only. Under `workspace` scope `execute_tool_runs` keeps only findings whose path is in the matched changed-file list. The rule could never report anything.
       - Accuracy: `--with-metadata`, the accurate mode, reports `tauri-build` unused for `kanban-app` and `mirdan-app`. Both `build.rs` files call `tauri_build::build()`. A false positive.

    2. `staticcheck -checks U1000` — ACCEPT, `workspace` scope. Proved `files` scope is unusable: given a package where `a.go` defines `sharedHelper` and `b.go` calls it, passing `a.go` alone makes staticcheck report `sharedHelper` unused. Whole-module `./...` reports only the truly dead item. Kinds at U1000: type, struct field, method, const, var, func. `-f json` emits one object per line with `location.file` and `location.line`.

    3. `knip` — REJECT. Ran it zero-config on `apps/kanban-app/ui` with a full `npm install` tree. Proven false positives: it reports `src/test/stubs/tauri-plugin-dialog.ts` an unused file, but `vite.config.ts` names that exact path as a `resolve.alias` target; it reports `info`, `debug`, `trace` unused exports, but `src/lib/log.ts` is a one-line re-export facade. It also cannot analyze a loose fixture file — it needs package.json, tsconfig, and node_modules.

    4. `vulture` — ACCEPT at `--min-confidence 100`. Measured the tiers: 60% is unused func/method/attribute and produced 14 findings on this repo's Python, every one a false positive (`forward` is the framework-invoked torch override, `author`/`short_description`/`version` are coremltools metadata). 90% adds unused imports and false-positives on a `TYPE_CHECKING` import used in a string annotation. 100% is unreachable code only — four forms, after `return`, `raise`, `continue`, `break` — and is clean on this repository.

    5. `periphery` — REJECT. Ran `periphery scan` on a directory holding a loose `.swift` file: "Failed to identify project in the current directory. For Xcode projects use the '--project' option, and for SPM projects change to the directory containing the Package.swift." It needs an Xcode or SPM project and builds it. A loose-file fixture is impossible.
  timestamp: 2026-08-07T15:35:57.306282+00:00
- actor: claude-code
  id: 01kzeep585yx7teq3tk2q348wp
  text: |-
    Implementation landed. TDD order: wrote `every_shipped_dead_code_tool_rule_passes_its_fixtures` first and watched it fail with "unused-code-go must be reported for a go project", then added the rules.

    Shipped, both with no `supersedes`:
    - `builtin/validators/code-hygiene/rules/unused-code-go.md` — `staticcheck -checks U1000 -f json ./...`, `workspace` scope, install pinned to `@2025.1.1`. `check_command` names `go` as well as `staticcheck` and `jq`, because `./...` package loading shells out to the go command.
    - `builtin/validators/code-hygiene/rules/unreachable-code-python.md` — `vulture --min-confidence 100`, `files` scope, install pinned to `==2.14` on both uv and pipx.

    Fixtures, each fail file holding one offending item of every kind its pass file covers:
    - `unused-code-go.fail.go` / `.pass.go` — six kinds: type, struct field, method, const, var, func. A new `fixtures/go.mod` makes the directory one module, the way `fixtures/Cargo.toml` makes it one crate, because U1000 reads a whole package.
    - `unreachable-code-python.fail.py` / `.pass.py` — four kinds: after `return`, `raise`, `continue`, `break`.

    Verified by running, not by reading:
    - Both pipelines run by hand in `fixtures/` exactly as doctor invokes them. Go: 6 findings, all in the fail file, 0 in the pass file. Python: 4 in the fail file, 0 in the pass file. Both pipelines exit 0.
    - Instrumented the acceptance test to prove neither rule took the tool-missing branch: `PROBE unused-code-go: presence=Present usable=true` and `PROBE unreachable-code-python: presence=Present usable=true`. The instrumentation was removed.
    - Added `the_shipped_python_dead_code_tool_rule_reports_without_suppressing_dead_code`, which drives `plan_tool_rules` and `execute_tool_runs`. Confirmed it really reports by mutating its expected count from 1 to 2 and reading the failure: `Finding { file: "src/stops_early.py", line: 7, validator: "code-hygiene", rule: Some("unreachable-code-python"), claim: "unreachable code after 'return'" }`, `confirmed: true`. The mutation was reverted. It also asserts `dead-code` is never in the suppressed set.

    Rejections recorded in `builtin/validators/code-hygiene/VALIDATOR.md`, each with the measurement behind it: `cargo machete`, `knip`, `periphery`.

    `crates/swissarmyhammer-validators/src/builtin/mod.rs`: split `CODE_HYGIENE_TOOL_RULES` into `CODE_HYGIENE_MISSING_DOCS_TOOL_RULES` and `CODE_HYGIENE_DEAD_CODE_TOOL_RULES`, because the old constant also asserted every tool rule supersedes `missing-docs`. The roster test now checks each group against the `supersedes` its group promises — `Some("missing-docs")` for documentation, `None` for dead code.

    Discovered work, filed as cards: ^qh5fnpd (mirdan lib tests fail at random under parallel execution — pre-existing, passes at `--test-threads=1`) and ^sx92jmt (whether a manifest-scoped validator set should host unused-dependency tools).
  timestamp: 2026-08-07T15:50:21.701018+00:00
- actor: claude-code
  id: 01kzeepgk1bbw7r0dyn3bh75yb
  text: |-
    ### implement — changed
    - evidence: 11 files. New: builtin/validators/code-hygiene/rules/unused-code-go.md, builtin/validators/code-hygiene/rules/unreachable-code-python.md, builtin/validators/code-hygiene/fixtures/unused-code-go.fail.go, builtin/validators/code-hygiene/fixtures/unused-code-go.pass.go, builtin/validators/code-hygiene/fixtures/unreachable-code-python.fail.py, builtin/validators/code-hygiene/fixtures/unreachable-code-python.pass.py, builtin/validators/code-hygiene/fixtures/go.mod. Modified: builtin/validators/code-hygiene/VALIDATOR.md, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs, crates/mirdan/src/builtin_validators.rs. Tests: `cargo test -p swissarmyhammer-validators` 505 passed 0 failed, doc-tests 2 passed 0 failed; `cargo test -p mirdan --lib -- --test-threads=1` 432 passed 0 failed; `cargo clippy -p swissarmyhammer-validators -p mirdan --all-targets` no warnings.
    - next: /review
  timestamp: 2026-08-07T15:50:33.313143+00:00
- actor: claude-code
  id: 01kzeghve858ksb5yv0kr0p4ys
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (af211dd8b, parent a1acf07a6). counts: findings 1, confirmed 1, refuted 0, attempted 12, failed 0, skipped 0. Open finding: crates/swissarmyhammer-validators/src/review/tool_rules.rs:1473.
    - engine route: the session MCP server carries an older image whose probe catalog lacks `assertion-census`, so the call failed with "unknown probe". The review ran instead against a fresh `sah serve` process of the on-disk binary, driven over stdio. That image's catalog does declare `assertion-census` for `test-integrity`. The `sah tool review ...` CLI route cannot run these ops — it wires no agent factory.
    - warning carried in the report: tool rule `code-hygiene/missing-docs-go` was unavailable (tool missing), so the prompt rule `missing-docs` ran instead.
    - test-refactor exception NOT applied to the open finding. The blanket exception covers a finding whose subject is test code that already existed. This finding's anchor, line 1473, is `every_shipped_dead_code_tool_rule_passes_its_fixtures`, which this commit ADDED. The pre-existing `every_shipped_missing_docs_tool_rule_passes_its_fixtures` sat at line 1302 in the parent and this commit did not change its body. The finding is therefore against new code and it stands.

    REJECTION AUDIT — all three stated reasons hold. Each was checked by measurement, not by reading.

    1. `cargo machete` — HOLDS, both reasons.
       - Structural: `builtin/file_groups/source_code.yaml` declares no manifest pattern. `Rule::matches` (validators/types.rs:757-766) gates on `ruleset.matches` first, so a rule narrows and never widens. `scope.rs:701-716` never admits a `Cargo.toml` to the work list, `matched_tool_rules` drops a rule with no matched files (tool_rules.rs:398-400), and the workspace retain (tool_rules.rs:639-644) keeps only findings whose normalized path is a matched changed file. A `Cargo.toml` finding cannot reach a report.
       - Accuracy: ran `cargo machete --with-metadata`. It reports `tauri-build` unused for `kanban-app` and `mirdan-app`. `apps/kanban-app/build.rs` and `apps/mirdan-app/build.rs` both call `tauri_build::build()`, and both declare `tauri-build` under `[build-dependencies]`. A false positive.
       - The adjacent sentence "its default mode misses that dependency kind" was challenged and then confirmed by direct experiment. In a temp crate carrying one unused regular dependency and one unused build-dependency, plain `cargo machete` reports the regular one and stays silent on the build-dependency. Default mode does not analyze build-dependencies. The sentence is correct.

    2. `knip` — HOLDS. `apps/kanban-app/ui/vite.config.ts:53-56` names `./src/test/stubs/tauri-plugin-dialog.ts` as a `resolve.alias` target, and that alias is the only reference to the stub, so the file is reachable only through it. `apps/kanban-app/ui/src/lib/log.ts` is one code line, `export { error, warn, info, debug, trace } from "@tauri-apps/plugin-log";` — a re-export facade that does export the three names.

    3. `periphery` — HOLDS. Reproduced at version 3.8.0 in a temp directory holding one loose `.swift` file. `periphery scan` exits 1 with the error quoted in VALIDATOR.md, character for character.

    - next: clear the open finding at tool_rules.rs:1473, then re-run /review.
  timestamp: 2026-08-07T16:22:57.736126+00:00
- actor: claude-code
  id: 01kzegm5xybvgfw1dhja7bq4vr
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 11 files; two dead-code tool rules accepted (staticcheck U1000 workspace scope, vulture at confidence 100), three rejected with measured reasons (cargo machete, knip, periphery). No rule supersedes the dead-code prompt rule. New cards ^qh5fnpd and ^sx92jmt.
    - test: green — cargo nextest run --workspace 13763 passed, doc tests 0 failed, fmt clean, clippy clean. The fixtures go.mod is inert to the Rust build. ^qh5fnpd reproduced: cargo test -p mirdan --lib failed 1 of 3 runs, cargo nextest passed 3 of 3.
    - commit: af211dd8b (parent a1acf07a6)
    - review: findings — crates/swissarmyhammer-validators/src/review/tool_rules.rs:1473 (the new fixture test reimplements the missing-docs one; one shared helper is wanted). All three rejection reasons were measured and hold.
  timestamp: 2026-08-07T16:24:14.014642+00:00
- actor: claude-code
  id: 01kzeh31a7959r2heyx55thn42
  text: |-
    Cleared the open finding at `tool_rules.rs:1473`, and swept the file for the same cause.

    The finding, word for word: `every_shipped_dead_code_tool_rule_passes_its_fixtures` reimplements the structure and logic of `every_shipped_missing_docs_tool_rule_passes_its_fixtures`; extract one shared test helper. The test-refactor exception does not release it — the reviewer proved line 1473 is code this commit ADDED, and the pre-existing function's body did not change.

    A finding shows one example of a cause. The cause is a new test that copies an existing test's body instead of sharing a helper. The file holds that cause TWICE, so both copies are gone:

    1. The doctor-fixture shape. New helper `verify_shipped_tool_rules_pass_fixtures(rules, expected_supersedes, rule_kind)`. Both fixture tests are now three lines each: the roster, the `supersedes` the group promises, and the group name the failure messages carry. `Some(MISSING_DOCS_PROMPT_RULE)` for documentation, `None` for dead code.

    2. The pipeline shape. `the_shipped_python_dead_code_tool_rule_reports_without_suppressing_dead_code` (added by this commit) copied the execute-and-assert tail of `the_shipped_rust_tool_rule_reports_an_undocumented_public_item`: run the plan, assert no tool errors, filter findings to the file, assert exactly one, confirmed, the right validator and rule, and the tool's own claim. New helper `verify_run_reports_one_finding(run, repo_root, path, rule, claim_fragment)` holds that whole tail. What stays in each test is what genuinely differs — the probe repository, the work-list, whether a missing tool panics or returns early, and whether the plan must suppress the prompt rule or must NOT.

    Also named two rule-name literals that each stood in three places: `RUST_MISSING_DOCS_RULE` and `PYTHON_UNREACHABLE_CODE_RULE`.

    Proved the shared helpers still assert, by mutation, not by reading:
    - Changed the dead-code call to `Some(DEAD_CODE_PROMPT_RULE)`. RED: "unused-code-go must supersede dead-code, the contract every dead-code tool rule keeps; left: None, right: Some(\"dead-code\")".
    - Changed the shared expected finding count from 1 to 2. RED in BOTH pipeline tests through the one helper, each printing its real finding — `unreachable code after 'return'` and `missing documentation for a struct`.
    - Reverted both mutations; back to GREEN.

    Blast radius: every changed item is private to `mod tests` in `tool_rules.rs`. No production symbol changed signature or behavior.
  timestamp: 2026-08-07T16:32:20.807405+00:00
- actor: claude-code
  id: 01kzeh37k9a0drr8p61ddy4g5t
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/tool_rules.rs. Tests: `cargo test -p swissarmyhammer-validators` 505 passed 0 failed, doc-tests 2 passed 0 failed; `cargo clippy -p swissarmyhammer-validators --all-targets` no warnings; `cargo fmt --check` clean. Mutation check drove both new helpers RED, then reverted to GREEN.
    - next: /review
  timestamp: 2026-08-07T16:32:27.241554+00:00
- actor: claude-code
  id: 01kzek6cykkdq1r5nbbydw7640
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (978a4eef3, parent af211dd8b)
    - prior finding verified closed: `verify_shipped_tool_rules_pass_fixtures` exists at tool_rules.rs:1439 and both `every_shipped_missing_docs_tool_rule_passes_its_fixtures` and `every_shipped_dead_code_tool_rule_passes_its_fixtures` call it.
    - counts: 3 findings, 3 confirmed, 2 refuted, 9 attempted, 0 failed, 0 skipped
    - evidence: crates/swissarmyhammer-validators/src/review/tool_rules.rs:347, :462, :1171
    - environment: the long-lived MCP server image lacks the `assertion-census` probe. Ran the engine through a fresh `sah serve` stdio process with /tmp/sah_review_driver.py.
    - next: fix the three findings, then re-review.
  timestamp: 2026-08-07T17:09:08.179810+00:00
- actor: claude-code
  id: 01kzek91wb8xptc1v1p8kj3y20
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — crates/swissarmyhammer-validators/src/review/tool_rules.rs; the prior finding is closed and verified (verify_shipped_tool_rules_pass_fixtures serves both fixture tests)
    - test: green — cargo nextest run --workspace 13763 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 978a4eef3 (parent af211dd8b)
    - review: findings — crates/swissarmyhammer-validators/src/review/tool_rules.rs:347, :462 (project_types takes &[String] where &[&str] fits), :1171 (the two new rule-name constants are not applied in SHIPPED_MISSING_DOCS_RULES and SHIPPED_DEAD_CODE_RULES)
    - guardrail: no repeat. Each round names a different cause.
  timestamp: 2026-08-07T17:10:35.147118+00:00
- actor: claude-code
  id: 01kzem7z5w632bt515rg8rj9y9
  text: |-
    Cleared all three open findings from the 12:03 review, and swept each cause across the whole file.

    FINDING :347 and :462 — `project_types: &[String]` must become `&[&str]`.

    The callers were checked before the signature changed, as instructed. No true conflict: `detected_project_type_keys` still returns `Vec<String>`, and each surface that owns that vector now collects one `Vec<&str>` at its boundary. Nothing at any caller got worse; the test call sites got shorter.

    One example is a cause, so all FOUR `&[String]` parameters in `tool_rules.rs` changed, not only the two the findings name: `plan_tool_rules`, `matched_tool_rules`, `matched_rule_files`, `project_tool_rules`.

    The ripple, carried through every caller:
    - `tool_install.rs` — `install_missing_tools` and `install_project_tool_rules` pass the same value straight through, so both take `&[&str]` too.
    - `doctor.rs` — `check_review_engine_with` takes `&[&str]`; `check_review_engine` collects once from `detected_project_type_keys`.
    - `synthesize.rs` — `run_review` collects once and uses the slice for both the install stage and the plan stage.
    - `apps/swissarmyhammer-cli/.../install/components/mod.rs` — `install_tool_rules_with` takes `&[&str]`; `ValidatorTools::init` collects once.

    Downward, `project_tool_rules` calls `ValidatorMatch::criteria_applies`, which took `&[String]`. Two ways to make that accept a borrowed slice, and the first was rejected by measurement of its cost:

    1. REJECTED — change `criteria_applies`/`project_types_match` to `&[&str]`. `matches_project_types` (types.rs) reads `ctx.project_types`, which is a `Vec<String>` the context owns, so every per-file match would have to allocate a `Vec<&str>` first. That is the per-file matching path of every review run.
    2. TAKEN — make both generic over `S: AsRef<str>`. `&[String]` and `&[&str]` both fit, no allocation anywhere, and no churn at the two existing callers. This is the same rule the finding cites: accept generics, not concrete types.

    FINDING :1171 — the new rule-name constants are not applied in the rosters.

    `SHIPPED_MISSING_DOCS_RULES` now reads `("rust", RUST_MISSING_DOCS_RULE)` and `SHIPPED_DEAD_CODE_RULES` reads `("python", PYTHON_UNREACHABLE_CODE_RULE)`.

    Swept the same cause over the whole file: for each constant this task added, every surviving literal of its value. `"missing-docs-rust"` and `"unreachable-code-python"` each had exactly one survivor, both in the rosters, both gone. No other literal in the file duplicates a constant this task added. The `"missing-docs"` literals in the planner tests are a DIFFERENT thing — they name a fabricated prompt rule inside the synthetic `docs` set, not the shipped roster, and binding them to the roster constant would create a coupling that does not exist.

    Also moved `DEAD_CODE_PROMPT_RULE` and `PYTHON_UNREACHABLE_CODE_RULE` up beside the other roster-name constants. A constant declared 150 lines below the array that reads it compiles, but it does not read.

    Incidental to the signature change: the doctor test helper `rust_types()` existed only to widen `RUST_TYPES` (`&[&str]`) into a `Vec<String>` for the old parameter. With the parameter borrowed, the twelve call sites pass `RUST_TYPES` and the helper is gone.

    No new test. Both findings are shape, not behavior: the signature change is proved by the compiler across every caller in the workspace, and the roster change substitutes a constant for the identical literal. A test asserting the roster holds the constant would assert nothing the compiler does not.

    Verified: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-cli` 1207 passed 0 failed 0 skipped; `cargo test --doc -p swissarmyhammer-validators` 2 passed; `cargo clippy --workspace --all-targets` no warning; `cargo fmt --all --check` clean. The blast radius is closed by `cargo check --workspace --all-targets`, which compiles every caller of the six changed signatures.
  timestamp: 2026-08-07T17:27:28.188327+00:00
- actor: claude-code
  id: 01kzem86nhx5ehr0w5ayr2b4qz
  text: |-
    ### implement — changed
    - evidence: 6 files — crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/review/tool_install.rs, crates/swissarmyhammer-validators/src/review/synthesize.rs, crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/src/validators/types.rs, apps/swissarmyhammer-cli/src/commands/install/components/mod.rs. Tests: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-cli` 1207 passed 0 failed 0 skipped; `cargo test --doc -p swissarmyhammer-validators` 2 passed; `cargo clippy --workspace --all-targets` no warning; `cargo fmt --all --check` clean.
    - next: /review
  timestamp: 2026-08-07T17:27:35.857640+00:00
position_column: doing
position_ordinal: '8480'
title: 'dead-code tools: evaluate narrow deterministic checks'
---
Evaluate deterministic dead-code tools. For each tool, decide: add a rule, or reject it and record why.

Do NOT supersede the `dead-code` prompt rule. Its carve-outs (entry points, exported public API, work-in-process scaffolding) need judgment, and the `callers` probe already gives that rule machine facts. A tool that supersedes it would flag staged work as dead.

Candidates for NEW narrow tool rules with no `supersedes`:
- Rust: `cargo machete` — unused dependencies. Low false-positive rate. Workspace scope.
- Go: `staticcheck -checks U1000` — unused code the compiler misses.
- JS/TS: `knip` — unused files and exports. Check the zero-config behavior first; a tool that demands per-project config does not fit the temporary-config contract.
- Python: `vulture` — known high false-positive rate. Reject it unless a confidence threshold makes it run clean on real code.
- Swift: `periphery` — needs a full project build. Likely too heavy for a review pass; reject if so.

Acceptance for each accepted tool: it runs clean on this repository, or every finding it reports is a real defect. Record each rejection in the code-hygiene VALIDATOR.md.

#tool-validators

## Review Findings (2026-08-07 11:12)

> tool rule 'code-hygiene/missing-docs-go' is unavailable (tool missing: exited with exit status: 1); prompt rule 'missing-docs' ran instead.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1473` — This function reimplements the structure and logic of the existing `every_shipped_missing_docs_tool_rule_passes_its_fixtures` function (lines 1422–1456). Both functions iterate over shipped rules, install tools, check the review engine, verify rule properties, and track exercised count with nearly identical implementation. The only differences are the constants used and the assertion on the `supersedes` field. Extract a common test helper `verify_shipped_tool_rules_pass_fixtures(rules, expected_supersedes, rule_type_name)` that both test functions call with their specific parameters, eliminating 40+ lines of code duplication.

## Review Findings (2026-08-07 12:03)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:347` — Function parameter `project_types: &[String]` accepts concrete String type instead of generic `&[&str]`; reduces flexibility for callers who have borrowed strings or literals. Change parameter to `project_types: &[&str]` and update internal conversions (line 429 already does `.iter().cloned()` to create owned Strings, which works with borrowed input).
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:462` — Function parameter `project_types: &[String]` accepts concrete type instead of generic `&[&str]`; reduces API flexibility in a crate-visible function. Change parameter to `project_types: &[&str]`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1171` — New constant `RUST_MISSING_DOCS_RULE` (line 1167) centralizes the rule name to replace repeated literal strings. The constant is used in three updated test locations (1228, 1304, 1323), but the same string `"missing-docs-rust"` remains hardcoded in `SHIPPED_MISSING_DOCS_RULES` array. If the rule name changes, this entry becomes inconsistent with the constant. Update `SHIPPED_MISSING_DOCS_RULES` to use the constant: `("rust", RUST_MISSING_DOCS_RULE)` and `SHIPPED_DEAD_CODE_RULES` to use `("python", PYTHON_UNREACHABLE_CODE_RULE)`. This is achievable in Rust const expressions and makes the centralization complete.