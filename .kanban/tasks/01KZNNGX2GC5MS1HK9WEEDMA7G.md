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
position_column: doing
position_ordinal: '8480'
title: magic-numbers-typescript omits 100 from ignore although the option supports it
---
`builtin/validators/code-hygiene/rules/magic-numbers-typescript.md` runs `@typescript-eslint/no-magic-numbers` with `ignore: [0,1,-1]` and seven `ignore*` position options, and declares `supersedes: [magic-numbers]`.

`magic-numbers.md` carves out "`0`, `1`, `-1`, and conventional values (a `<< 8`, `100` for percent)".

`ignore` takes an arbitrary number list — `magic-numbers-swift` proves it by naming `100` — but this rule omits it, so `pct = n * 100 / total` reports. `x << 8` reports as well.

The declaration carve-out is the most thorough of the four rules: seven `ignore*` options each name a declaration position, and `enforceConst` and `detectObjects` are left off so a `const` binding and an object property both name their value.

The one-off carve-out is the declared and accepted split, not a defect.

Decide the allow-list, and keep it the same across the four `magic-numbers-*` rules unless a tool forces otherwise.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity