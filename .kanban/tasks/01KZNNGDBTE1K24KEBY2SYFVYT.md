---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzpta2xqpq2dkezg0xsq6p71
  text: |-
    Picked up. Measured ruff 0.14.5 myself on a probe module, `--isolated --no-cache --select PLR2004`:

    - `x == -1`, `x == 0`, `x == 1`, `x == 0.0`, `x == 1.0` — silent.
    - `x == 100`, `x == 3600`, `x == -2`, `x == -100`, `x == 8`, `x > 100.0` — reported.
    - `a << 8`, `a * 100`, `a + 3600`, `g(3600)`, `return 3600` — silent, because `PLR2004` reads comparisons only.
    - `x == (1 << 8)` — silent, because the operand is an expression and not a literal.
    - `lint.pylint.allow-magic-values` does not exist. ruff answers: "unknown field `allow-magic-values`, expected one of `allow-magic-value-types`, ...". `allow-magic-value-types = ["str","bytes","int"]` silences EVERY integer, so it is not a value carve-out.

    Every claim on the card is correct.

    Sibling rules read before the decision:
    - `magic-numbers-go` sets `ignored-numbers: ["0","1","-1"]` — `100` is NOT allowed, so mnd reports it.
    - `magic-numbers-typescript` sets `ignore: [0, 1, -1]` — `100` is NOT allowed, so eslint reports it.
    - `magic-numbers-swift` sets `allowed_numbers: [0, 1, -1, 100]` — `100` IS allowed.

    So two of the three siblings that CAN express a value allow-list deliberately leave `100` out. Python reporting `100` agrees with Go and TypeScript. Swift is the outlier, and that divergence belongs on ^xd5r1zh.
  timestamp: 2026-08-10T21:47:24.471674+00:00
- actor: claude-code
  id: 01kzptz9gsjj7b3awdbqbfj2c3
  text: |-
    Decision: option 1, the `# noqa: PLR2004` contract. Rejected the other two:

    - A post-filter in the run script is closed by a WRITTEN rule, not by my judgement. `builtin/validators/README.md` states: "Selection in the pipe is attribution, not exemption ... To exempt one code item, use an inline suppression in the code — never the pipe." All four `magic-numbers-*` rules already repeat that sentence. A `jq` step that dropped `100` would also drop a genuine `status == 100` that has nothing to do with percent, because the pipe reads the value and never the meaning.
    - A statement alone that the carve-out cannot be expressed is incomplete. A recourse exists, so a rule body that only names the gap leaves the author with no answer.

    The `# noqa` form was measured, not assumed: `# noqa: PLR2004 — <reason>` (em dash and trailing prose) suppresses, `# noqa: PLR2004` alone suppresses, and a bare `# noqa` suppresses. Unlike periphery, ruff accepts trailing text after the code.

    Sibling agreement is deliberate. Go and TypeScript can both state a value allow-list and neither puts `100` in it, so both report `x == 100` exactly as ruff does. Python now agrees with them. Swift is the one outlier; recorded as a comment on ^xd5r1zh.

    Corrections made to the rule body:
    - Removed the false claim that `PLR2004` "already matches the `magic-numbers` prompt carve-outs".
    - Stated the real value list — `0`, `1`, `-1` (and `0.0`, `1.0`) — and that `100`, `3600`, `-2` and `100.0` all report.
    - Stated that `a << 8` is silent for the WRONG reason: a shift is an operation, not a comparison. `x == 8` reports.
    - Stated that `lint.pylint.allow-magic-value-types` takes only `bytes`, `complex`, `float`, `int`, `str`, so naming `int` silences every integer. There is no `allow-magic-values` key.
    - Added a section "Where this rule is NARROWER than the rule it supersedes": `PLR2004` reads a comparison alone, so a repeated literal in a call argument, an operation or a return is never reported, and repetition is the prompt rule's primary target.

    Made the disputed facts machine-checked instead of prose. The fail fixture now holds `ratio == 100` and the pass fixture holds `WHOLE_RATIO_PERCENT = 100`. A new acceptance test, `the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value`, drives the real ruff pipeline and holds the run to exactly `404`, `4096`, `10`, `90`, `100` — the value entry proves the carve-out is absent, and the count proves no other position reports. Watched it fail RED first ("the fail fixture must report the unnamed literal `100`") before the fixture carried the literal.

    Also corrected the pass fixture: its `carved_out` docstring claimed "Compare against 0, 1, and -1" while the body compared only against 0 and -1. The body now holds all three comparisons.
  timestamp: 2026-08-10T21:58:59.353432+00:00
- actor: claude-code
  id: 01kzptzjcy4atcta36pqng6j3x
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/magic-numbers-python.md, builtin/validators/code-hygiene/fixtures/magic-numbers-python.fail.py.tmpl, builtin/validators/code-hygiene/fixtures/magic-numbers-python.pass.py.tmpl, crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs
    - `sah doctor` on a Python probe project: `code-hygiene/magic-numbers-python — tool present (ruff 0.14.5); fixtures pass`
    - `cargo nextest run --workspace`: 14039 tests, 14035 passed, 4 failed — the four known pre-existing failures of ^bh5ncd0 (three `review_e2e` on `report_has_claim(markdown, CLAIM_DUP)`, and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`). No file of `swissarmyhammer-tools` was touched.
    - `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for `/review`
  timestamp: 2026-08-10T21:59:08.446080+00:00
- actor: claude-code
  id: 01kzpvnc9qt16hdgab9055cmak
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (commit 758416086). Engine counts: 2 findings, 2 confirmed, 0 refuted, 8 attempted, 0 failed, 0 skipped, `skipped_files` empty. 1 finding recorded — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:841. 1 finding dropped by the written rule that keeps existing tests out of scope — shipped.rs:206 against shipped.rs:111.
    - verified by hand, on the four points the caller named:
      - The false claim is GONE, not softened. The old sentence "The rule already matches the `magic-numbers` prompt carve-outs" is deleted. The new body states the measured value list (`0`, `1`, `-1`) and states "**`100` REPORTS.**".
      - The rule body states where it is NARROWER, under the heading "Where this rule is NARROWER than the rule it supersedes": a repeated literal in a call argument, in an operation, or in a `return` is never reported.
      - The new test drives the real pipeline. It calls `plan_tool_rules` and `execute_tool_runs` against a real temporary repository, so `ruff` runs. It is not a fixture-only test. `cargo test ... the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value` passes.
      - The RED claim holds. `ruff 0.14.5 check --isolated --select PLR2004` on the fail fixture of HEAD~1 reports 4 values (`404`, `4096`, `10`, `90`) and finds no `100`. On the fail fixture of HEAD it reports 5 values. Without the fixture change both the `100` assertion and the count assertion fail.
    - next: correct the recorded finding at shipped.rs:841. Do NOT rewrite the test at shipped.rs:371, because that test was in the repository before this commit.
  timestamp: 2026-08-10T22:11:03.095802+00:00
- actor: claude-code
  id: 01kzpvq3gwq3kev462rm5mk16w
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 5 files. Option 1, the `# noqa: PLR2004` contract, chosen because a WRITTEN rule closes option 2: `builtin/validators/README.md` says "Selection in the pipe is attribution, not exemption ... To exempt one code item, use an inline suppression in the code — never the pipe." A filter on `100` would also drop a genuine `status == 100`, because the pipe reads the value and never the meaning. Option 3 alone leaves the author with no recourse.
    - the sibling difference is deliberate and was checked before the choice: `magic-numbers-go` sets `ignored-numbers: ["0","1","-1"]` and `magic-numbers-typescript` sets `ignore: [0, 1, -1]`. Both CAN state a value allow-list and neither puts `100` in it, so both report `x == 100` as ruff does. Python now agrees with them. `magic-numbers-swift` sets `allowed_numbers: [0, 1, -1, 100]` and is the one outlier of the four; that divergence is recorded on ^xd5r1zh.
    - the false claim is gone, not softened. The reviewer confirmed the sentence is deleted and replaced by "**`100` REPORTS.**"
    - test: `cargo nextest run --workspace` 14039 run, 14035 passed, 0 skipped; the 4 failures are the known ^bh5ncd0 set. `sah doctor` reports `magic-numbers-python — tool present (ruff 0.14.5); fixtures pass`.
    - commit: 758416086
    - review: findings — 2 findings, 2 confirmed, 8 tasks attempted, 0 failed. 1 dropped under the written exception for tests that already existed (`shipped.rs:206`, a pair that pre-dates this commit). 1 recorded: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:841`, caused by this commit.
    - IMPORTANT, and not a finding for this card: the reviewer was told to expect the two `.tmpl` fixtures to appear in `skipped_files`, and refused to confirm it. `skipped_files` came back EMPTY. `review file` on the fail fixture answers "Nothing in scope to review", which the engine prints only when the excluded list is empty. The validator-fixture exclusion of ^4cc5y9b does not fire at runtime for the builtin layer of THIS repository, although its unit test passes against the real repository root. Filed as its own card.
  timestamp: 2026-08-10T22:11:59.644175+00:00
- actor: claude-code
  id: 01kzpw2ccqkxjpn7015qb7zpxx
  text: |-
    Picked the card up again for the open finding at `shipped.rs:841`.

    Read the whole file. Two causes are present, not one:

    1. The fixture-driven pair the finding names. `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` and `the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value` share: builtin loader, tool precondition, shipped asset lookup, fixture read, temporary repository, canonicalized root, work-list, plan, run lookup, execute, no-error assertion, filter by path, loop over the expected entries, count. Only three things differ — the work-list, how one finding gives the text it is held to, and how an expected entry meets that text.

    2. The same cause reaches wider. The block that finds the planned run or panics with the plan's fallbacks is written SIX times in this file: at the Rust missing-docs test, the Rust complexity test, the Python dead-code test, the TypeScript complexity test, the Python magic-numbers test, and the Rust unused-dependency test. A finding shows one example of a cause, so this block goes too.

    No third fixture-driven pipeline test exists. `the_shipped_typescript_complexity_config_reads_its_framework_names` reads the RULE SOURCE and runs node, not the pipeline. `swift_package_root` reads a fixture template, but it copies a manifest and plans nothing. The other three pipeline tests build their own probe source and already share `verify_run_reports_one_finding`, so their remaining common part is the run lookup of cause 2.
  timestamp: 2026-08-10T22:18:09.175116+00:00
- actor: claude-code
  id: 01kzpwrc7nwrhen7fc5bcy25cy
  text: |-
    Corrected the finding. One file changed: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`.

    Two shared parts came out, because the finding names one example of a cause that reaches wider than the pair.

    1. `ShippedFailFixture` and `verify_shipped_fail_fixture_reports_each`. The struct holds the project types, the rule, the shipped fixture name, the path inside the probe repository, the expected entries and the noun for the failure messages. The function does the shared work: the builtin loader, the tool precondition, the shipped asset lookup, the fixture read, the temporary repository, the canonicalized root, the plan, the run, the execute, the no-error assertion, the filter by path, the loop over the expected entries and the count. Three callbacks carry what one language does not share with another — `build_work` for the work-list, `extract` for the text one finding is held to (it also gets the fixture source lines), and `matches` for how an entry meets that text. The two tests are now 12 and 18 lines.

    2. `required_run`. The block that finds the planned run or panics with the plan's fallbacks was written six times. It is written once now, and the six sites call it. This also serves the new helper.

    The assertions did not get weaker.
    - TypeScript still holds each guard with `starts_with` against the SOURCE LINE the finding stands on, and still panics when a reported line stands past the end of the fixture.
    - Python still holds each value with `contains` against the backquoted form inside the CLAIM.
    - Both still hold the count to the length of the expected list.

    Both still drive the real tool. Proof, by a mutation run: I changed `100` to `101` in the Python list and `get band(` to `get bandXX(` in the TypeScript list, then ran both tests. Each failed, and each failure message printed the list the run built out of `outcome.findings()`:
    - Python reported the five real ruff messages, `PLR2004 Magic value used in comparison, consider replacing `404` ... `100` with a constant variable`.
    - TypeScript reported the eight real source lines, `function foldGrid(grid: number[][], limit: number): number {` through `step("build the grid", () => {`.
    An empty list, which is what a fixture-only test would give, cannot produce either. I then put the two lists back.

    The helper did NOT absorb `dead_code_work` and `complexity_work`. `dead_code_work` is not fixture-driven, `complexity_work` still serves the Rust complexity test, and routing both through `tool_rule_work` would make the two MORE alike after normalization, not less. Both stay as they were, under the written rule that keeps existing tests out of scope. `sah tool code_context duplication find` over the file now reports that pair and nothing else — the pair the finding named is gone.

    No third fixture-driven pipeline test exists to route through the helper. `the_shipped_typescript_complexity_config_reads_its_framework_names` reads the rule source and runs node, and `swift_package_root` copies a manifest and plans nothing.
  timestamp: 2026-08-10T22:30:09.909244+00:00
- actor: claude-code
  id: 01kzpwrpgvqwc0xzb94g5n25nt
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. Added `ShippedFailFixture`, `verify_shipped_fail_fixture_reports_each` and `required_run`. Routed both fixture-driven tests through the helper, and all six run lookups through `required_run`.
    - finding at shipped.rs:841 is now `- [x]`.
    - both tests still drive their real tool, proved by a mutation run that printed the five real ruff `PLR2004` messages and the eight real eslint source lines.
    - `sah tool code_context duplication find` over the file: 1 finding, the pre-existing `dead_code_work` / `complexity_work` pair. The pair the review named is gone.
    - `cargo nextest run --workspace`: 14039 run, 14035 passed, 0 skipped, 4 failed — the known ^bh5ncd0 set (three `review_e2e`, and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`). No file of `swissarmyhammer-tools` was touched. `the_swift_package_root_restores_the_directory_before_it_removes_it` passed.
    - `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for `/review`
  timestamp: 2026-08-10T22:30:20.443751+00:00
position_column: doing
position_ordinal: '8480'
title: magic-numbers-python reports 100, and ruff exposes no value allow-list to carve it out
---
`builtin/validators/code-hygiene/rules/magic-numbers-python.md` runs `ruff check --isolated --select PLR2004` and declares `supersedes: [magic-numbers]`.

`magic-numbers.md` carves out "`0`, `1`, `-1`, and conventional values (a `<< 8`, `100` for percent)".

Measured on stdin: `x == -1`, `x == 0` and `x == 1` are silent, but `x == 100` reports `PLR2004 Magic value used in comparison, consider replacing 100 with a constant variable`. `lint.pylint.allow-magic-value-types` selects TYPES, not values, so the `100` carve-out cannot be restored through configuration.

The rule file claims `PLR2004` "already matches the `magic-numbers` prompt carve-outs". That claim is wrong for `100` and needs correcting whatever the fix is.

The declaration carve-out IS reproduced, because `PLR2004` fires only inside comparisons.

The lesser defect in the other direction: because `PLR2004` is comparison-only, a repeated literal in a call argument, an operation or a return is never reported, and repetition is the prompt rule's primary target.

Decide: a `# noqa` contract, a post-filter in the run script, or a statement on the rule that the carve-out cannot be expressed.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity

## Review Findings (2026-08-10 17:01)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:841` — Test function duplicates the fixture-based testing pattern that already exists in `the_shipped_typescript_complexity_tool_rule_measures_every_fail_fixture_guard` (line 371). Both tests follow near-identical boilerplate: load builtin loader, require tool installed, retrieve shipped asset, read fixture content, create temp directory, canonicalize repository path, plan tool rules, execute runs, assert no errors, collect/filter findings, loop through expected values to verify each appears. This 95%+ structural similarity indicates a near-match that should have been extended via parameterization rather than duplicated. Extract a shared test helper `verify_shipped_tool_rule_fixture_findings<W, E>` that accepts (1) rule name, project types, fixture metadata, (2) a work_builder callback to parameterize how the work-list is created, and (3) an extractor/verifier callback to parameterize how findings are validated. Both tests can then call the helper with their specific builders/verifiers, eliminating ~70 lines of duplicated boilerplate and making each test's intent clearer.

### Notes on this pass

- Scope reviewed: `review sha HEAD~1..HEAD`, which is commit 758416086.
- Cause of the finding above: this commit. The commit adds the test at line 841. The test at line 371 was in the repository before this commit.
- One more finding came back from the engine: `dead_code_work` (line 206) is a near-duplicate of `complexity_work` (line 111), 75 tokens, 91 percent alike. Both functions were in the repository before this commit, and this commit does not change them. The review skill drops a finding that asks to refactor test code that already exists. That written rule drops this finding.
- Engine counts: 2 findings, 2 confirmed, 0 refuted, 8 tasks attempted, 0 tasks failed, 0 files skipped, `skipped_files` empty.
- The two `.tmpl` fixture files of this commit are NOT in `skipped_files`, and the report has no "not reviewed" note. The validator-fixture exclusion did not fire on this run. Its unit test `a_changed_builtin_fixture_leaves_the_scope_and_source_stays` passes against the real repository root, so the mechanism is correct and the runtime loader resolves different fixture roots. This is engine behaviour, not a defect of this commit.