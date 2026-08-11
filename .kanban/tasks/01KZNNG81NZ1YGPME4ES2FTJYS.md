---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzpymsyg08tsf2xmxyxee4vf
  text: |-
    ^eedma7g added `100` to the `magic-numbers-typescript` allow-list (`ignore: [0, 1, -1, 100]`). Two items for this card:

    1. `builtin/validators/code-hygiene/rules/magic-numbers-python.md` now carries a bullet that says `magic-numbers-go` "does not name `100` yet ... the card `^s2ftjys` adds it there". When this card lands, correct that bullet to state that Go names `100` too, and remove the reference to this card.
    2. The shift half of the carve-out was measured for TypeScript: `ignore` selects a VALUE and never a POSITION, so `8` in the list silences `word === 8` beside `word << 8`, and eslint names no option for a shift operand. `mnd` reads `operation` as one of its six positions and `ignored-numbers` is a value list too, so the same answer probably holds for Go — measure it before you decide.
  timestamp: 2026-08-10T23:03:10.032234+00:00
- actor: claude-code
  id: 01kzpzbhf2c305gmea18sh7r8r
  text: |-
    Research done. Measured `mnd` through golangci-lint 2.12.2 (go 1.26.5) on a probe module.

    Baseline `ignored-numbers: ["0","1","-1"]`:
    - `100` REPORTS in three positions: `return part * 100` (`<return>`), `scaled := part * 100 / total` (`<operation>`), `Sched(part * 100)` (`<argument>`), and `usage == 100` inside a return (`<return>`).
    - With `["0","1","-1","100"]` every one of those is silent, and `4096`, `3600`, `404`, `20`, `512`, `250`, `8` still report. So the allow-list works, and it is spelled with strings.

    Shift carve-out — two levers, both measured, both refused:
    1. `8` in `ignored-numbers` silences `word << 8` AND `word >> 8` AND `status == 8`. The list selects a VALUE, never a position, so it trades a real comparison finding for the carve-out.
    2. Dropping `operation` from `checks` does NOT buy the carve-out. It silences `packed := word << 8`, but `return word << 8` and `Sched(word << 8)` still report, because `mnd` attributes a shift inside a `return` to its `<return>` check and one inside a call to its `<argument>` check. It also silences the real `n * 3600`. Strictly worse.
    3. No third lever. `golangci-lint config verify` accepts exactly `checks`, `ignored-numbers`, `ignored-files`, `ignored-functions` under `linters.settings.mnd`, and answers an invented key with `additional properties 'ignored-shifts' not allowed`. None names a shift operand.

    So Go matches TypeScript: the value half of the carve-out is expressible, the position half is not, and the recourse is the inline `//nolint:mnd // <reason>`.

    Other measured facts worth keeping:
    - `mnd` reads the DIRECT operands of an expression a `return` holds. `return part * 100 / total` is silent, because the `100` sits in a nested `BinaryExpr`. `return part * 100` reports. A fixture must use a form that reports.
    - `golangci-lint` writes the schema error to stderr, which the rule's pipe drops, so a bad settings key would read as zero findings.
  timestamp: 2026-08-10T23:15:35.010691+00:00
- actor: claude-code
  id: 01kzq0yd9pye9h56j8g78sgmxk
  text: |-
    Implementation landed. Two defects the new acceptance test found, both of which made the rule report ZERO findings and name no reason.

    **The test was RED twice, for two different reasons, and each fix is measured.**

    RED 1 — `the_shipped_go_magic_numbers_tool_rule_reports_every_fail_fixture_value` asked for `8` and the run reported the other five values. GREEN: the fail fixture now carries `packed := word << 8`.

    RED 2 — the pass fixture took `100` for percent in an operation and in a condition, and the doctor answered `the pass fixture magic-numbers-go.pass.go.tmpl produced 2 finding(s); none are allowed`. GREEN: `ignored-numbers: ["0", "1", "-1", "100"]`.

    Then the test passed alone and FAILED inside the test binary, three runs of three, with `the run reported []`. Two causes, both measured:

    1. **The golangci-lint lock.** One file lock for each run. A second instance stops with `Error: parallel golangci-lint is running` on stderr and writes nothing to stdout, and the script sends stderr to `/dev/null`, so the run reads as a clean file. Eight runs of the script started together reported `6, 6, 6, 0, 6, 0, 6, 0`. `run: allow-serial-runners: true` makes the second instance WAIT; all eight then reported `6`.
    2. **The shared cache.** `golangci-lint` answers by package content and stores each finding with the ABSOLUTE path of the run that first cached it. Directory B, holding the same fixture bytes under the same module name as an earlier run, got the earlier run's path back — `/private/var/.../T/.tmpuFxLbQ/src/magic_numbers_go_fail.go` — and the engine drops a finding it cannot place in the workspace. With `GOLANGCI_LINT_CACHE` set to a directory named for `$PWD`, the same run reported its own path.

    Both fixes are in the rule, each with its measurement in the rule body. `function-length-go` carries the same script shape and both defects; `unused-code-go` runs `staticcheck` and needs the same question asked. Card `^mms9g8d` holds that work.

    **What did not work, so the next agent does not repeat it.**
    - Dropping `operation` from `checks` does not buy the shift carve-out. It silenced `packed := word << 8`, but `return word << 8` and `f(word << 8)` still reported, because `mnd` attributes a literal to the check of the statement that holds it. It also loses `n * 3600`. Strictly worse.
    - `8` in `ignored-numbers` silences `status == 8` beside the shift.
    - Editing a rule `.md` alone does NOT rebuild the test binary. The builtin rules are compiled in, and `cargo nextest run` reused the old binary, so the rule change appeared to have no effect. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` forces the rebuild.

    **Shared helper.** `verify_shipped_fail_fixture_reports_each` took a new `support` field: the other shipped fixtures a probe repository needs, each with the path it takes. A `files`-scope probe names `NO_SUPPORT_FIXTURES`; the Go probe names the shipped `go.mod`, because a `workspace`-scope run loads a module and a lone Go file loads nothing. The copy step is now `copy_shipped_fixture`, used for the fail fixture and for each support fixture.
  timestamp: 2026-08-10T23:43:21.910436+00:00
- actor: claude-code
  id: 01kzq0ynhvwjmpx291kjfh33aj
  text: |-
    ### implement — changed
    - evidence: 6 files — builtin/validators/code-hygiene/rules/magic-numbers-go.md, builtin/validators/code-hygiene/rules/magic-numbers-python.md, builtin/validators/code-hygiene/fixtures/magic-numbers-go.fail.go.tmpl, builtin/validators/code-hygiene/fixtures/magic-numbers-go.pass.go.tmpl, crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. `cargo nextest run --workspace`: 14041 run, 14037 passed, 4 failed — the known ^bh5ncd0 set only (3 review_e2e + the stdio timeout). `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean. `every_shipped_magic_numbers_tool_rule_passes_its_fixtures` PASS, and the validators crate ran 579/579 green three times in a row.
    - next: /review
  timestamp: 2026-08-10T23:43:30.363064+00:00
position_column: doing
position_ordinal: '8480'
title: magic-numbers-go omits 100 and shift constants from ignored-numbers
---
`builtin/validators/code-hygiene/rules/magic-numbers-go.md` runs `mnd` through golangci-lint with `ignored-numbers: ["0","1","-1"]` and declares `supersedes: [magic-numbers]`.

`magic-numbers.md` carves out "`0`, `1`, `-1`, and conventional values (a `<< 8`, `100` for percent) read clearly inline and need no constant."

`ignored-numbers` covers `0`, `1` and `-1` exactly. It does not cover `100` for percent, and `operation` is one of mnd's six default checked positions, so `x << 8` and `n * 100 / total` both report. `ignored-numbers` accepts any set, so the list can be extended.

`magic-numbers-swift` already puts `100` in its allow-list, and its rule file says why: "The swiftlint default is `[0.0, 1.0, 100.0]`, which is the prompt carve-out list without `-1`, so the config states `[0, 1, -1, 100]` and the two lists then agree." The Go rule can state the same list.

The declaration carve-out IS reproduced, and the one-off carve-out is a declared and accepted split, not a defect.

Decide the allow-list, and keep it the same across the four `magic-numbers-*` rules unless a tool forces otherwise.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity