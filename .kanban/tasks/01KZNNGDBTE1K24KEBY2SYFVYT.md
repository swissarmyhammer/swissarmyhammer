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