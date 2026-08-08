---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgng5qatw5w2qr9s5ba0c5k
  text: |-
    Picked up. Research done — measured every candidate tool before choosing a threshold.

    ## Tool measurements (real runs, this machine)

    `ruff 0.14.5` — `PLR2004` is `magic-value-comparison`. It reports a literal **only in a comparison**, and it already ignores `0`, `1`, `""`, `"__main__"`. On a probe holding a comparison, an operation, an argument, a default, and an index it reported the comparison alone. Nothing to tune.

    `eslint 10.8.0` — the base `no-magic-numbers` rule is loud: it reported the array index, the default value, and the enum member. `@typescript-eslint/no-magic-numbers` adds the options that switch those off. With `ignore: [0, 1, -1]`, `ignoreArrayIndexes`, `ignoreDefaultValues`, `ignoreClassFieldInitialValues`, `ignoreEnums`, `ignoreNumericLiteralTypes`, `ignoreReadonlyClassProperties`, `ignoreTypeIndexes` the same probe left three findings: the comparison, the operation, and the call argument. `typescript-eslint` is already an install dependency of `missing-docs-typescript`.

    `swiftlint 0.65.0` — `no_magic_numbers` is opt-in and already quiet. It has an `allowed_numbers` option, default `[0.0, 1.0, 100.0]`. It never reported a variable declaration, a stored property, an enum raw value, or a default parameter. `-1` is not in the default allow list, so the rule sets `allowed_numbers: [0, 1, -1, 100]` to match the prompt carve-outs exactly.

    `mnd v2.5.1` — **the standalone binary is dead on the Go 1.26 toolchain.** `go install github.com/tommy-muehle/go-mnd/v2/cmd/mnd@v2.5.1` builds, then segfaults on every input: `panic: runtime error: invalid memory address or nil pointer dereference` in `go/types.(*StdSizes).Sizeof`. It panics on `./...`, on a single file, with `go 1.21` and `go 1.26` in `go.mod`, and with `GO111MODULE=off`. Its `golang.org/x/tools` requirement is `v0.0.0-20200329025819`, six years old, and that copy hands `go/types` a nil `Sizes`. Rebuilding it in a scratch module against `x/tools@v0.39.0` makes it work, which confirms the cause — but `v2.5.1` is the newest tag and `@master` resolves to the same commit, so no pinned single-command install can produce a working binary.

    `golangci-lint v2.12.2` carries the same `mnd` analyzer, is maintained, builds and runs on Go 1.26, installs with one pinned command, and accepts `--config <path>` so the rule owns its whole invocation. Verified: with `default: none`, `enable: [mnd]`, `ignored-numbers: ["0","1","-1"]` it reported the comparison, the condition, the switch case, the operation, the argument, and the bare return, and left the `const` and the `:=` alone. **The Go rule runs `mnd` through golangci-lint.** The lint is the one the card names; golangci-lint is the carrier that still works.

    ## Residual difference from the prompt carve-outs

    Every tool reports a **one-off** literal, where the prompt rule carves one out. That is the whole point of the split: repetition needs a judge, position does not. The calibration above removes every context the prompt rule names — `0`/`1`/`-1`, a value a declaration already names, an enum member, an array index, a default — and leaves the contexts where nothing names the number: a comparison, a switch case, an operation, a call argument.

    ## Blast radius

    - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — `CODE_HYGIENE_PROMPT_RULES` and the roster count test. `magic-numbers` joins the prompt roster; a new tool-rule roster const carries the four tool rules.
    - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — `SHIPPED_MAGIC_NUMBERS_RULES` plus a third `verify_shipped_tool_rules_pass_fixtures` test. This is the only test that runs shipped fixtures.
    - `crates/mirdan/src/builtin_validators.rs` — `test_tool_rule_fixtures_are_embedded` names every fixture file.
    - The tool rules supersede `magic-numbers`, never `data-driven`, so `review_e2e.rs` item 3 (`CLAIM_DATA`, the tier if-chain) keeps firing.
    - The Go fixtures join the existing `fixtures` package under `go.mod.tmpl`. No Rust fixtures, so `lib.rs.tmpl` does not change.
  timestamp: 2026-08-08T12:27:54.474467+00:00
- actor: claude-code
  id: 01kzgp8ft3kcd0085r69yrf7k5
  text: |-
    Implementation landed. Files:

    Prompt rules — `builtin/validators/code-hygiene/rules/`
    - `data-driven.md` — rewritten. Keeps the table check alone. Adds a "What This Rule Does Not Own" section that hands the unnamed literal to `magic-numbers`, and a second carve-out for a chain over an *open* set.
    - `magic-numbers.md` — new. Carries the repeated-literal and repeated-configuration checks with the same carve-outs, plus one more: a literal a declaration already names. It states which languages a tool owns and why a tool reports the one-off it carves out.

    Tool rules — same directory, each `supersedes: magic-numbers`
    - `magic-numbers-python.md` — ruff `PLR2004`, `--isolated --no-cache`, `files` scope.
    - `magic-numbers-typescript.md` — eslint `@typescript-eslint/no-magic-numbers`, `files` scope.
    - `magic-numbers-go.md` — `mnd` through golangci-lint, `workspace` scope.
    - `magic-numbers-swift.md` — swiftlint `no_magic_numbers`, `files` scope.

    Fixtures — `builtin/validators/code-hygiene/fixtures/`, eight files, one fail/pass pair per rule.

    Rust
    - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — `magic-numbers` joins `CODE_HYGIENE_PROMPT_RULES`; new `CODE_HYGIENE_MAGIC_NUMBERS_TOOL_RULES`; the roster count and the `supersedes` map cover it.
    - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — `MAGIC_NUMBERS_PROMPT_RULE`, `SHIPPED_MAGIC_NUMBERS_RULES`, and `every_shipped_magic_numbers_tool_rule_passes_its_fixtures`.
    - `crates/mirdan/src/builtin_validators.rs` — the eight new fixture names join `test_tool_rule_fixtures_are_embedded`.

    Docs
    - `builtin/validators/code-hygiene/VALIDATOR.md` — a new "Hardcoded values: two rules, one concern each" section states the split and the four tool rules, matching the precedent the dead-code section set.

    ## Two decisions the measurements forced

    **The Go rule runs `mnd` through golangci-lint.** The standalone `mnd` binary segfaults on the Go 1.26 toolchain and no pinned install command can fix it. The full evidence is in the earlier comment and in the rule body.

    **The Go rule must set `max-same-issues: 0` and `max-issues-per-linter: 0`.** The golangci-lint defaults are 3 and 50, and `max-same-issues: 3` keeps three findings that share a message and silently drops the rest. On a probe file holding eight identical comparisons the default reported 3 of 8. A gate that drops findings without saying so is worse than no gate.

    ## Verification that each rule really runs

    The doctor test passes, but a rule whose tool is absent is skipped rather than failed, so a green run alone does not prove the rule ran. Each pass fixture was given one planted unnamed literal in turn, and each time the test went red naming that rule and that fixture:

    ```
    magic-numbers-python's tool is installed, so its fixtures must pass; doctor says: fixtures failed: the pass fixture magic-numbers-python.pass.py.tmpl produced 1 finding(s); none are allowed
    magic-numbers-typescript's tool is installed, so its fixtures must pass; ... magic-numbers-typescript.pass.ts.tmpl produced 1 finding(s)
    magic-numbers-go's tool is installed, so its fixtures must pass; ... magic-numbers-go.pass.go.tmpl produced 1 finding(s)
    magic-numbers-swift's tool is installed, so its fixtures must pass; ... magic-numbers-swift.pass.swift.tmpl produced 1 finding(s)
    ```

    Every plant was reverted and the baseline is green again. All four tools therefore run their fixture pair through the real doctor path.

    The fail fixtures were also read by hand through each rule's exact pipe. Findings per fail fixture: Python 4 (two comparison kinds plus both sides of a chain), TypeScript 3 (comparison, operation, argument), Swift 4 (condition, case, operation, argument), Go 5 (condition, case, operation, return, argument). Every pass fixture reported 0.

    One fixture correction along the way: `mnd` attributes a literal inside `return size * 4096` to its `return` check, so the first Go fail fixture never covered the `operation` check. Both Go fixtures now bind the product to a name first, and a comment on the fail fixture records why.

    ## Doctor rows

    `sah doctor` emits one `Validator Tool Rule · code-hygiene/<rule>` row per tool rule of the detected project types, from `to_checks`. No code was needed for the new rows. Note that on a machine that has run `sah init`, doctor reads `~/.validators/`, so the new rows appear after the next `sah init` refreshes the user store — the same as every rule before them.
  timestamp: 2026-08-08T12:41:11.235656+00:00
- actor: claude-code
  id: 01kzgpa2fknpkjtbtnpxcczztx
  text: |-
    Discovered work, filed as `^6c3ry21` — two validator sets, `magic-numbers` and `naming`, were deleted from `builtin/validators/` without being registered as retired, so `sah init` never prunes them from a user store. `~/.validators/` on this machine holds 14 sets against 12 built-ins, and `sah doctor` reports each orphan as "applies to this project (user)".

    The `magic-numbers` orphan touches this card directly. A machine that carries it will run the old standalone `magic-numbers` set beside the new `code-hygiene/magic-numbers` rule and review the same concern twice with two different rule bodies. That is a store-pruning defect, not a rule defect, so it stays on its own card.
  timestamp: 2026-08-08T12:42:03.123192+00:00
- actor: claude-code
  id: 01kzgpvqmxaw5dxzck70s4dre4
  text: |-
    Test results, and one failure investigated to root cause.

    ## Green

    - `cargo test -p swissarmyhammer-validators --all-targets` — 524 passed, 0 failed. This carries the new `every_shipped_magic_numbers_tool_rule_passes_its_fixtures`, the roster test, `every_builtin_validators_suffix_fits_the_framings_authored_share` (the prompt-byte budget the new prompt rule enlarges), `no_rule_matches_a_shipped_fixture_template`, and `every_builtin_tool_rule_pins_its_install_commands`.
    - `cargo test -p swissarmyhammer-skills --all-targets` — 128 passed, 0 failed.
    - `cargo clippy -p swissarmyhammer-validators -p mirdan --all-targets -- -D warnings` — clean.
    - `cargo test -p mirdan --lib builtin_validators` — 7 passed, five runs in a row.

    ## The mirdan failure is a pre-existing race, filed as `^x2a3zg7`

    `cargo test -p mirdan --lib` failed once for the test agent and passed on rerun. I did not accept "flaky" — I measured it.

    Eight consecutive runs on this tree: **4 failed, 4 passed**, and the test that failed was different nearly every time:

    - `install::tests::test_install_tool_from_tool_md_content` (twice)
    - `install::tests::test_e2e_all_four_types_coexist`
    - `install::tests::test_deploy_and_uninstall_plugin`
    - `install::tests::test_deploy_and_uninstall_tool`

    Then I reverted my one mirdan edit — `git checkout -- crates/mirdan/src/builtin_validators.rs`, leaving `git status` clean for the whole crate — and ran six more times: **1 failed**, this time `list::tests::test_scan_scoped_store_reads_the_project_validator_directory`. The failure survives the removal of my change, so my change is not its cause. My edit is a list of eight strings inside `builtin_validators.rs`'s test module and reaches none of these tests.

    Root cause: the process working directory is global, and three mirdan modules write it without the lock that would make that safe — `install/tests.rs` has 44 `set_current_dir` calls against 29 `#[serial]` markers, `list.rs` 8 against 4, `new.rs` 12 against 6. A test that is not `#[serial]` runs beside one that is, so two threads own the directory at once, and a deleted temporary directory leaves the next test standing nowhere. That is the reported error exactly: `test_deploy_and_uninstall_tool` fails on its own first line, `let old_dir = std::env::current_dir().unwrap()`, with `Io(Os { code: 2, kind: NotFound })`.

    `crates/swissarmyhammer-common/src/test_utils.rs::CurrentDirGuard` already exists for this, and mirdan already depends on both it and `serial_test`. Fixing it means touching 64 call sites in three files, which is an unrelated refactor, so it stays on its own card.

    The `builtin_validators.rs` edit was restored after the experiment; `git status` shows it modified again.
  timestamp: 2026-08-08T12:51:41.853796+00:00
- actor: claude-code
  id: 01kzgq2xppy2vdb6jtr6ct13fx
  text: |-
    ### implement — changed
    - evidence: 17 files. New prompt rule `builtin/validators/code-hygiene/rules/magic-numbers.md`; new tool rules `magic-numbers-python.md`, `magic-numbers-typescript.md`, `magic-numbers-go.md`, `magic-numbers-swift.md`; eight new fixtures under `builtin/validators/code-hygiene/fixtures/` (`magic-numbers-{python,typescript,go,swift}.{fail,pass}.*.tmpl`); rewritten `rules/data-driven.md`; `VALIDATOR.md`; `crates/swissarmyhammer-validators/src/builtin/mod.rs`; `crates/swissarmyhammer-validators/src/review/tool_rules.rs`; `crates/mirdan/src/builtin_validators.rs`.
    - tests: `cargo test -p swissarmyhammer-validators --all-targets` 524 passed, 0 failed. `cargo test -p swissarmyhammer-tools --test tools_tests` 163 passed, 0 failed, including all four `review_e2e` tests. `cargo test -p swissarmyhammer-skills --all-targets` 128 passed, 0 failed. `cargo clippy -p swissarmyhammer-validators -p mirdan -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - each of the four tool rules proven live: planting one unnamed literal in its pass fixture turns `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` red and names that rule; every plant reverted.
    - pre-existing failure, not caused by this card: `cargo test -p mirdan --lib` fails about one run in two from a process-working-directory race in `install/tests.rs`, `list.rs`, and `new.rs`. Reproduced with this card's mirdan edit fully reverted. Filed as `^x2a3zg7`.
    - also filed: `^6c3ry21`, the orphaned `magic-numbers` and `naming` validator sets that `sah init` never prunes.
    - next: `/review`.
  timestamp: 2026-08-08T12:55:37.430351+00:00
- actor: claude-code
  id: 01kzgqpyyydtnhthm1tpjqkh8b
  text: |
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (ab778d1dc3ceae037c2231b37a0de9edf0ecc1a4) — 0 findings, 0 confirmed, 38 refuted, 9 validators attempted, 0 failed, 0 skipped
    - next: task moved to done
  timestamp: 2026-08-08T13:06:34.078202+00:00
- actor: claude-code
  id: 01kzgqqsqc8smaw85739pzr47h
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 17 files (data-driven narrowed to the table check; new magic-numbers prompt rule; 4 tool rules with fail/pass fixtures)
    - test: green — cargo nextest run --workspace, 13858 passed, 0 failed, 0 skipped; clippy clean; fmt clean
    - commit: ab778d1dc3ceae037c2231b37a0de9edf0ecc1a4
    - review: clean — 0 findings, 38 candidates refuted, 9 validators attempted
    - next: none — task is in done
  timestamp: 2026-08-08T13:07:01.484834+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc980
title: magic-numbers tool rules + split the data-driven prompt rule
---
Step 1 — split the `data-driven` prompt rule in `builtin/validators/code-hygiene`:
- `data-driven` keeps the table check (a match/if chain over a known set is a table). No tool can make that judgment.
- A new `magic-numbers` prompt rule takes the repeated-literal and repeated-configuration checks. It keeps the same carve-outs: 0, 1, -1, conventional values, and one-off literals in an obvious context.

Step 2 — tool rules that supersede `magic-numbers`:
- Python: ruff PLR2004 with `--isolated`.
- TypeScript/JavaScript: eslint `no-magic-numbers` with ignore list [0, 1, -1] in a temporary config.
- Swift: swiftlint `no_magic_numbers` — an opt-in rule; turn it on in the temporary config.
- Go: `mnd`.
- Rust: no healthy lint exists. Rust keeps the `magic-numbers` prompt rule.
- Dart: the check needs a custom_lint package. Dart keeps the prompt rule.

Compare each tool's default ignore behavior with the prompt carve-outs before you set thresholds. A tool that flags every inline literal makes noise, and noise kills the gate.

Every tool rule ships a fail/pass fixture pair and shows doctor rows.

#tool-validators