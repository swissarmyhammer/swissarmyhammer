---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzpymn5c3k9rjy1m9wzhgsdj
  text: |-
    Measured `eslint` 10.8.0 with `@typescript-eslint/no-magic-numbers` 8.66.0 on a probe module, through a copy of the shipped `tool.run` script. Each claim of the card is true:

    - With `ignore: [0, 1, -1]`, both `(n * 100) / total` and `usage === 100` report `No magic number: 100.`
    - With `ignore: [0, 1, -1, 100]`, both are silent, and `size * 4096` still reports.
    - `word << 8`, `word >> 8` and `word === 8` each report `No magic number: 8.`
    - `8` added to `ignore` makes all three silent. So `ignore` selects a VALUE and never a POSITION.

    The shift carve-out CANNOT be expressed. `eslint` validates the option object against the rule schema and names the whole set the rule accepts: `detectObjects`, `enforceConst`, `ignore`, `ignoreArrayIndexes`, `ignoreDefaultValues`, `ignoreClassFieldInitialValues`, `ignoreEnums`, `ignoreNumericLiteralTypes`, `ignoreReadonlyClassProperties`, `ignoreTypeIndexes`. None of the ten names an operand of a shift, and an eleventh key is refused — an added `ignoreShift` makes eslint answer `Unexpected property "ignoreShift"` and stop.

    The rule body now states the gap plainly and names the recourse: the inline suppression `// eslint-disable-next-line @typescript-eslint/no-magic-numbers`. The fail fixture carries `word << 8` and an acceptance test holds eslint to reporting it, so the gap stays measured.
  timestamp: 2026-08-10T23:03:05.132888+00:00
- actor: claude-code
  id: 01kzpyp4d59m7j4gye8rjwkvpy
  text: |-
    ### implement — changed

    - evidence: 6 files.
      - `builtin/validators/code-hygiene/rules/magic-numbers-typescript.md` — `ignore: [0, 1, -1, 100]`; the bullet now names `100` as the percent half of the carve-out with the measured before/after; a new section "The shift carve-out cannot be expressed" states the gap and names the inline suppression as the recourse.
      - `builtin/validators/code-hygiene/rules/magic-numbers-python.md` — the false section "The sibling rules report `100` too" is rewritten as "This is the one rule of the four that cannot allow `100`".
      - `builtin/validators/code-hygiene/fixtures/magic-numbers-typescript.pass.ts.tmpl` — a `percent()` function reads `(part * 100) / total`.
      - `builtin/validators/code-hygiene/fixtures/magic-numbers-typescript.fail.ts.tmpl` — a `shift()` function reads `word << 8`.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` — new `TYPESCRIPT_MAGIC_NUMBERS_RULE` constant, used by the roster.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — new acceptance test `the_shipped_typescript_magic_numbers_tool_rule_reports_every_fail_fixture_value`, through the shared `verify_shipped_fail_fixture_reports_each` helper.

    - test-first, both halves seen RED then GREEN:
      1. The percent line in the pass fixture made `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` fail with "the pass fixture magic-numbers-typescript.pass.ts.tmpl produced 1 finding(s); none are allowed". `ignore: [0, 1, -1, 100]` made it pass.
      2. The new acceptance test failed with "the fail fixture must report the unnamed literal `8`; the run reported [...404, 4096, 250]". The shift line in the fail fixture made it pass.

    - commands: `cargo nextest run --workspace` — 14040 tests, 14036 passed, 4 failed, all four the known pre-existing failures (three `review_e2e` and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`, filed as ^bh5ncd0). `cargo fmt` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    - health check: `sah doctor` shows `Validator Tool Rule · code-hygiene/magic-numbers-typescript ┆ tool present (v10.8.0); fixtures pass`. That row is NOT evidence for this change: every set row reads "applies to this project (user)", so the CLI doctor serves the `~/.validators` snapshot and not the builtins. The builtin-backed doctor runs inside `every_shipped_magic_numbers_tool_rule_passes_its_fixtures`, which loads the compiled-in copy and passes for all four `magic-numbers-*` rules.

    - out of scope, recorded not fixed: `magic-numbers-go` still reads `ignored-numbers: ["0", "1", "-1"]`, so `(part * 100) / total` is silent for TypeScript and Swift and reports for Go until ^s2ftjys lands. ^s2ftjys carries a note to correct the Python bullet that names it.

    - not touched, as the card orders: the seven `ignore*` position options, `enforceConst` and `detectObjects` at their defaults, and the one-off split.

    - next: ready for `/review`.
  timestamp: 2026-08-10T23:03:53.509947+00:00
- actor: claude-code
  id: 01kzpz0fdzt2tpzkxtmfa0ty1k
  text: |
    ### review — clean

    - scope: `review sha HEAD~1..HEAD` (commit `0612f0429`).
    - counts: findings 1, confirmed 1, refuted 0, attempted 8, failed 0, skipped 0, skipped_files [].
    - the one engine finding is dropped by the written exception of the review skill: it asks to deduplicate test code that already existed.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:207` — `dead_code_work` is a near-duplicate of `complexity_work` at `shipped.rs:121` (75 tokens, 91% alike).
      - Both functions are pre-existing debt, not caused by this commit. `git blame` gives `complexity_work` to `da41103e63` and `dead_code_work` to `087d052b43`. This commit touches only `shipped.rs` lines 923-991.
    - the four sibling claims of the `magic-numbers-python` rule are each TRUE against the rule files:
      - `magic-numbers-swift.md` line 15 writes `allowed_numbers: [0, 1, -1, 100]`.
      - `magic-numbers-typescript.md` line 29 writes `ignore: [0, 1, -1, 100]`.
      - `magic-numbers-go.md` line 22 writes `ignored-numbers: ["0", "1", "-1"]`, so it does not name `100`. The card `^s2ftjys` ("magic-numbers-go omits 100 and shift constants from ignored-numbers") is open in `todo`.
      - `magic-numbers-python.md` lines 56-61 measure that `ruff` has no value allow-list, and lines 63-72 measure `# noqa: PLR2004` as the recourse.
    - the shift statement is honest. The eslint `ignore` option holds numbers, so it selects a value. `8` in the list therefore silences `word << 8`, `word >> 8` and `word === 8` together. The stated recourse also works: the `tool.run` script passes `--no-config-lookup --config`, and neither the flags nor the config turn inline comments off, so `// eslint-disable-next-line @typescript-eslint/no-magic-numbers` applies.
    - the new acceptance test drives the real tool. `verify_shipped_fail_fixture_reports_each` reads the shipped fixture, writes it into a temporary repository, plans the rule from `builtin_loader()`, and calls `execute_tool_runs`. `require_tool_installed` panics when eslint is absent, so the test cannot pass silently.
    - next: the card is clean. Move to done.
  timestamp: 2026-08-10T23:09:32.479157+00:00
- actor: claude-code
  id: 01kzpz20g7xhcsmxypvggvvmbb
  text: |
    ### finish iteration 1 — clean
    - the allow-list decision: restore the `100` carve-out wherever the tool can express it. The reason is the governing principle of this card series — a rule that declares `supersedes` must reproduce the carve-outs of the rule it supersedes, and dropping one is the defect the survey exists to remove.
    - implement: changed — 6 files. `ignore: [0, 1, -1, 100]`. Measured with eslint 10.8.0 and `@typescript-eslint/no-magic-numbers` 8.66.0 through a copy of the shipped run script: before, `(n * 100) / total` and `usage === 100` each reported "No magic number: 100."; after, both are silent and `size * 4096` still reports.
    - the SHIFT carve-out cannot be expressed, and the rule now says so instead of leaving it silent. `word << 8`, `word >> 8` and `word === 8` all report, and putting `8` in `ignore` would silence all three, because `ignore` selects a value and never a position. eslint refuses an `ignoreShift` key and names its ten accepted options, none for a shift operand. The recourse is the inline `// eslint-disable-next-line`, and the fail fixture plus a new acceptance test hold eslint to reporting the shift, so the gap is measured rather than asserted.
    - THE CROSS-CARD CORRECTION: adding `100` here made a paragraph of `magic-numbers-python.md` false. ^2syfvyt had written "The sibling rules report `100` too ... Python agreeing with them is deliberate", which was true when it landed two commits earlier. That paragraph is replaced by "This is the one rule of the four that cannot allow `100`". Leaving a false claim about a sibling in a rule body is the exact defect this series removes, so the correction belongs in this commit and not in a later one.
    - test: `cargo nextest run --workspace` 14040 run, 14036 passed, 0 skipped; the 4 failures are the known ^bh5ncd0 set.
    - commit: 0612f0429
    - review: clean — 1 finding, 8 tasks attempted, 0 failed. The one finding is the pre-existing `dead_code_work` pair, dropped under the written exception; the reviewer confirmed by `git blame` that both functions pre-date the commit and that the commit's only hunk in that file is elsewhere.
    - the reviewer checked each of the four sibling claims against the actual rule files rather than reading the prose: Swift `allowed_numbers: [0, 1, -1, 100]`, TypeScript `ignore: [0, 1, -1, 100]`, Go `ignored-numbers: ["0", "1", "-1"]` with ^s2ftjys open to add it, and ruff with no value allow-list at all. All four true.
    - CAUTION carried forward: `sah doctor` serves the `~/.validators` snapshot, so every set row reads `(user)`. Its rows are NOT evidence about the builtin rules. The builtin-backed check is `every_shipped_magic_numbers_tool_rule_passes_its_fixtures`.
    - the validator-fixture exclusion of ^4cc5y9b did not fire again: `skipped_files` empty, both `.tmpl` fixtures in scope. Tracked on ^07pmgmx.
  timestamp: 2026-08-10T23:10:22.727401+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffdc80
title: magic-numbers-typescript omits 100 from ignore although the option supports it
---
`builtin/validators/code-hygiene/rules/magic-numbers-typescript.md` runs `@typescript-eslint/no-magic-numbers` with `ignore: [0,1,-1]` and seven `ignore*` position options, and declares `supersedes: [magic-numbers]`.

`magic-numbers.md` carves out "`0`, `1`, `-1`, and conventional values (a `<< 8`, `100` for percent)".

`ignore` takes an arbitrary number list — `magic-numbers-swift` proves it by naming `100` — but this rule omits it, so `pct = n * 100 / total` reports. `x << 8` reports as well.

The declaration carve-out is the most thorough of the four rules: seven `ignore*` options each name a declaration position, and `enforceConst` and `detectObjects` are left off so a `const` binding and an object property both name their value.

The one-off carve-out is the declared and accepted split, not a defect.

Decide the allow-list, and keep it the same across the four `magic-numbers-*` rules unless a tool forces otherwise.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity