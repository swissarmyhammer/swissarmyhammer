---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzptyvy52v5d1qvfag0jm1y6
  text: |-
    Found while working ^2syfvyt (the Python sibling). This card says "The numeric list agrees with the prompt list." That is true of the prompt list, but the four `magic-numbers-*` rules do NOT agree with each other about `100`:

    - `magic-numbers-go` — `ignored-numbers: ["0", "1", "-1"]`. `100` reports.
    - `magic-numbers-typescript` — `ignore: [0, 1, -1]`. `100` reports.
    - `magic-numbers-swift` — `allowed_numbers: [0, 1, -1, 100]`. `100` is silent.
    - `magic-numbers-python` — `ruff` `PLR2004` hardcodes `0`, `1`, `-1` and gives no value allow-list. `100` reports. Measured on ruff 0.14.5.

    Three of the four report `100`; Swift alone allows it. `^2syfvyt` decided Python keeps the majority behaviour and states the gap in the rule body. So the `100` question is now open on Swift alone. Decide it here beside the shift question, because both are the same carve-out sentence: "conventional values (a `<< 8`, `100` for percent)".
  timestamp: 2026-08-10T21:58:45.445216+00:00
- actor: claude-code
  id: 01kzpyn441d3qe90vf2xscda5k
  text: |-
    This card asks whether a shift constant is carved out, and whether the answer is the same for `magic-numbers-go` and `magic-numbers-typescript`. ^eedma7g answered the TypeScript half by measurement on eslint 10.8.0 with `@typescript-eslint/no-magic-numbers` 8.66.0:

    The shift carve-out CANNOT be expressed, for the same reason it cannot in Swift. `ignore` selects a VALUE and never a POSITION: `word << 8`, `word >> 8` and `word === 8` each report `No magic number: 8.`, and `8` added to `ignore` makes all three silent. A list that carried `8` would drop a genuine `status === 8` to keep the shift silent.

    No option answers it either. eslint names the whole option set the rule accepts — `detectObjects`, `enforceConst`, `ignore`, `ignoreArrayIndexes`, `ignoreDefaultValues`, `ignoreClassFieldInitialValues`, `ignoreEnums`, `ignoreNumericLiteralTypes`, `ignoreReadonlyClassProperties`, `ignoreTypeIndexes` — and refuses an eleventh key.

    The TypeScript answer is therefore: the shift operand reports, the rule body states that plainly, and the recourse is the inline suppression `// eslint-disable-next-line @typescript-eslint/no-magic-numbers`. The fail fixture carries `word << 8` and the acceptance test `the_shipped_typescript_magic_numbers_tool_rule_reports_every_fail_fixture_value` holds eslint to reporting it. `swiftlint` has the same shape of lever (`allowed_numbers` is a value list), so the same answer is likely here — the equivalent recourse is `// swiftlint:disable:next no_magic_numbers`.
  timestamp: 2026-08-10T23:03:20.449997+00:00
- actor: claude-code
  id: 01kzq1vvzfj3jad290dg6mvj9r
  text: |-
    Measured swiftlint 0.65.0 (`/opt/homebrew/bin/swiftlint`). **The card's claim is REFUTED, and Swift does not match Go or TypeScript.**

    `no_magic_numbers` reads the OPERATOR, not only the value. With the shipped `allowed_numbers: [0, 1, -1, 100]`:

    - `return word << 8` — silent.
    - `return word >> 8` — silent.
    - `return 4096 << width` — silent, so BOTH operands of a shift are carved out.
    - `return status == 8` — reports.

    So the shift carve-out IS expressed on Swift, and it is expressed WITHOUT the trade Go and TypeScript refused: a genuine `status == 8` still reports. `8` must stay OUT of `allowed_numbers` — adding it silences `status == 8` and buys nothing.

    The carve-out is the shift operator alone. In one identical shape (`return word <OP> 8`), silent for `<<` and `>>`; reports for `&<<` (masking shift), `*`, `+`, `&`, `|`, `^` and `==`.

    **The residual gap, measured.** The carve-out holds when the shift is the WHOLE expression at its position. It does not reach a shift that is one link of a longer unparenthesised operator chain:

    | Written | Reported |
    |---|---|
    | `return word << 8` | no |
    | `let packed = word << 8` | no |
    | `schedule(value: word << 8)` | no |
    | `acc = (word << 8)` | no |
    | `return (word << 8) \| 1` | no |
    | `if (word << 8) > 0` | no |
    | `acc = word << 8` | yes |
    | `return word << 8 \| 1` | yes |
    | `if word << 8 > 0` | yes |
    | `return flag ? word << 8 : word` | yes |

    Two recourses, both measured: put parentheses around the shift (silent in all four reported shapes), or write `// swiftlint:disable:next no_magic_numbers` above the line (silences `return word << 8 | 1`).

    **No option answers the rest.** `swiftlint rules no_magic_numbers` names the whole option set the rule accepts — `severity`, `test_parent_classes`, `allowed_numbers`. None names a shift. An added `allowed_shifts` makes swiftlint answer `Configuration for 'no_magic_numbers' rule contains the invalid key(s) 'allowed_shifts'.`

    **The `100` question the earlier comment left open is now closed and needs no change.** `magic-numbers-go` now states `ignored-numbers: ["0","1","-1","100"]` (^s2ftjys) and `magic-numbers-typescript` now states `ignore: [0, 1, -1, 100]` (^eedma7g). Three of the four agree, and Python is the one that cannot state a value allow-list at all. Swift is no longer the outlier.
  timestamp: 2026-08-10T23:59:27.215990+00:00
- actor: claude-code
  id: 01kzq24gf7m0t8nwbjtdv9nxnm
  text: |-
    **Decision on the second defect — the temp `swiftlint.yml` discards the project's own `.swiftlint.yml`. LEFT here, and it belongs with ^xv57pf8 and ^h2ezbs7.**

    Why it belongs there and not here:

    - It is not a defect of `no_magic_numbers`. It is a defect of the SHAPE all three Swift rules share: each writes `only_rules:` plus its own thresholds into a temporary file and passes `--config`, so each drops the project's `excluded:` list the same way. Fixing one gives three Swift rules three different config-build shapes, and the next reader cannot tell which is the pattern.
    - `^xv57pf8` already names all three rules in its body and states the two candidate answers — give the temp config an `excluded:` list of its own, or merge the project's. That choice must be made once and applied to all three.
    - The choice needs a measurement this card does not make: whether swiftlint merges a `parent_config`/`child_config`, and whether an `excluded:` list read out of the project's `.swiftlint.yml` is safe to copy without also copying the rule set beside it. That measurement is the body of ^xv57pf8's work.

    `magic-numbers-swift` therefore keeps the same discard as its two siblings, and the fix lands with them. A note naming this rule as the third site is added to ^xv57pf8.
  timestamp: 2026-08-11T00:04:10.343238+00:00
- actor: claude-code
  id: 01kzq2fhhqfcrxmedb01ckdnnb
  text: |-
    ### implement — changed
    - evidence: 5 files.
      - `builtin/validators/code-hygiene/rules/magic-numbers-swift.md` — new section "The shift carve-out is expressed, and Swift alone expresses it", with the operator measurement, the ten-shape table of the residual gap, the two recourses, and the option-set refusal. `allowed_numbers` is UNCHANGED at `[0, 1, -1, 100]`, because the carve-out is already reached and `8` would lose `status == 8`.
      - `builtin/validators/code-hygiene/fixtures/magic-numbers-swift.pass.swift.tmpl` — added `return word << 8` and `return word >> 8`, which hold swiftlint to the carve-out.
      - `builtin/validators/code-hygiene/fixtures/magic-numbers-swift.fail.swift.tmpl` — added `return word << 8 | 1`, which holds swiftlint to the edge of it.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` — new `SWIFT_MAGIC_NUMBERS_RULE` const, used in the roster in place of the written name.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — new `SWIFT_MAGIC_NUMBERS_FAIL_PROBE` and `the_shipped_swift_magic_numbers_tool_rule_reports_every_fail_fixture_line`, through `verify_shipped_fail_fixture_reports_each`. It holds each finding to the SOURCE LINE, not to the claim, because `no_magic_numbers` writes one message for every literal and never spells the value.
    - RED→GREEN, watched both ways. With `return word << 8 | 1` cut out of the fail fixture the new test failed with `the fail fixture must report the line holding an unnamed literal 'return word << 8 | 1'; the run reported ["if status == 404 {", "case 20:", "return size * 4096", "return schedule(delayMillis: 250)"]`. With `return word << 8` turned into `return word * 8` in the pass fixture, `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` failed with `the pass fixture magic-numbers-swift.pass.swift.tmpl produced 1 finding(s); none are allowed`. Both fixtures restored, both tests green.
    - `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` PASSES, which is the evidence about the builtin rules — `sah doctor` was not used.
    - `cargo nextest run --workspace --no-fail-fast`: 14042 tests, 14038 passed, 4 failed. The 4 are the known ^bh5ncd0 set — three `review_e2e` plus the stdio timeout — and none of them is Swift. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` was run before every measurement run, so each ran against the rebuilt compiled-in builtins.
    - next: `/review`.
  timestamp: 2026-08-11T00:10:11.895618+00:00
- actor: claude-code
  id: 01kzq2v37j3mmxyfbd8x5amy43
  text: |-
    ### review — clean
    - scope: `review sha HEAD~1..HEAD` (commit aaf01c890).
    - counts: 1 finding, 1 confirmed, 0 refuted, 8 attempted, 0 failed, 0 skipped. `skipped_files` is empty — the fixture exclusion did NOT fire on this range.
    - The one finding is DROPPED under the written exception for tests that already exist: `shipped.rs:207` `dead_code_work` against `shipped.rs:121` `complexity_work`. Both functions are byte-identical at the same lines in HEAD~1, so this is pre-existing debt, and this commit does not cause it.
    - Verified the refutation by an independent run of swiftlint 0.65.0 with the shipped config `allowed_numbers: [0, 1, -1, 100]`. Every line of the rule table agrees with the measurement. Silent: `return word << 8`, `return word >> 8`, `return 4096 << width`, `let packed = word << 8`, `return (word << 8) | 1`. Reports: `return status == 8`, `return word &<< 8`, `return word * 8`, `acc = word << 8`, `return word << 8 | 1`, `if word << 8 > 0`, `return flag ? word << 8 : word`. The claim is also internally consistent: `status == 8` reports while `word << 8` is silent, which is possible only if the rule reads the operator.
    - Verified `8` is NOT in `allowed_numbers`. The committed rule states `allowed_numbers: [0, 1, -1, 100]`, so a true `status == 8` finding stays.
    - Verified the rule body states the measurement, the residual edge for a shift in a longer unparenthesised chain, and both recourses — parentheses, or `// swiftlint:disable:next no_magic_numbers`.
    - Verified the new acceptance test drives the real tool. `verify_shipped_fail_fixture_reports_each` copies the shipped fixture, calls `plan_tool_rules`, then `execute_tool_runs`, and holds the count to an exact match. It does not read pre-computed data. `cargo test -p swissarmyhammer-validators --lib the_shipped_swift_magic_numbers_tool_rule_reports_every_fail_fixture_line` passed after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`.
    - next: none. The card moves to done.
  timestamp: 2026-08-11T00:16:30.450241+00:00
- actor: claude-code
  id: 01kzq2wfc5zznh71t98dpb5brw
  text: |
    ### finish iteration 1 — clean
    - THE CARD'S CLAIM IS REFUTED. The card says `x << 8` reports "because `8` is not in `allowed_numbers` and swiftlint reads the position, not the operator". Measured on swiftlint 0.65.0 with the shipped `allowed_numbers: [0, 1, -1, 100]`: `word << 8` and `word >> 8` are SILENT, `4096 << width` is silent (both operands), and `status == 8` still REPORTS. swiftlint reads the OPERATOR. In one identical shape `word <op> 8`, only `<<` and `>>` are silent; `&<<`, `*`, `+`, `&`, `|`, `^` and `==` all report.
    - so Swift is the ONE rule of the four that expresses the shift carve-out, and it already did. Go and TypeScript cannot, for the structural reason both of those cards measured: their allow-list selects a value and never a position.
    - `8` stays OUT of `allowed_numbers`. Adding it would buy nothing and would silence a genuine `status == 8`.
    - the residual edge is measured and stated: the carve-out reaches a whole shift, not one link of a longer unparenthesised chain. `acc = word << 8`, `return word << 8 | 1`, `if word << 8 > 0` and a ternary all report; parentheses silence all four, and so does `// swiftlint:disable:next no_magic_numbers`. The option set is exactly `severity`, `test_parent_classes`, `allowed_numbers`, and a fourth key is refused by name.
    - the discarded `.swiftlint.yml` defect was LEFT here on purpose. It belongs to the config-build shape all three Swift rules share, so fixing one would give three rules three shapes. A note naming `magic-numbers-swift` as the third site is on ^xv57pf8, with the measurements the fix needs.
    - an end-to-end acceptance test now drives the real swiftlint pipeline, holding each finding to its source line because swiftlint never spells the value. RED proved both ways.
    - test: `cargo nextest run --workspace --no-fail-fast` 14042 run, 14038 passed, 0 skipped; the 4 failures are the known ^bh5ncd0 set.
    - commit: aaf01c890
    - review: clean — 1 finding, 8 tasks attempted, 0 failed. The one finding is the pre-existing `dead_code_work` pair, dropped under the written exception for the fourth time.
    - the reviewer re-ran swiftlint itself rather than trusting the rule body, and reproduced all ten rows of the table. Its decisive check: `status == 8` reports while `word << 8` is silent — the same value with a different operator, which is only possible if the rule reads the operator.
  timestamp: 2026-08-11T00:17:15.653208+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffde80
title: magic-numbers-swift reports a conventional shift constant
---
`builtin/validators/code-hygiene/rules/magic-numbers-swift.md` runs `swiftlint` with `only_rules: [no_magic_numbers]` and `allowed_numbers: [0, 1, -1, 100]`, and declares `supersedes: [magic-numbers]`.

`magic-numbers.md` carves out "`0`, `1`, `-1`, and conventional values (a `<< 8`, `100` for percent)".

This is the closest of the four `magic-numbers-*` rules. The numeric list agrees with the prompt list. What is left is the shift form: `x << 8` reports, because `8` is not in `allowed_numbers` and swiftlint reads the position, not the operator.

The declaration carve-out IS reproduced, measured: "it reported nothing for a variable declaration, a stored property, a `static let`, an enumeration raw value, or a default parameter".

Note that the temp `swiftlint.yml` discards the project's own `.swiftlint.yml`, so a project's `excluded:` list for generated Swift is lost here too. That is the same defect the `complexity-swift` and `missing-docs-swift` cards name.

Decide whether a shift constant is carved out, and whether the answer is the same for `magic-numbers-go` and `magic-numbers-typescript`.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity