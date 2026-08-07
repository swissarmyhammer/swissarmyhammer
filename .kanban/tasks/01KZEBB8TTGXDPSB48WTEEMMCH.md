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