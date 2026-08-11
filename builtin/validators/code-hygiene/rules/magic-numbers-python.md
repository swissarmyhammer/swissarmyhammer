---
name: magic-numbers-python
description: Unnamed Python literals need constants — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: magic-numbers
tool:
  scope: files
  run: |
    ruff check --isolated --no-cache --select PLR2004 --output-format json "$@" |
      jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'
  doctor:
    check_command: "which ruff jq"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Magic Numbers — Python

`ruff` reports every unnamed numeric literal a comparison reads. `PLR2004` is the
one rule that names that check, and its own name — `magic-value-comparison` —
states the narrow scope: a literal in a comparison, and nothing else.

That scope is why this rule needs no threshold of its own. `PLR2004` has no
threshold to set: it carries a fixed value list of its own, and `ruff` gives no
option that adds a value to it.

## Which carve-outs the tool reproduces, and which it does not

Measured against a probe module on `ruff` 0.14.5:

- **The value list is `0`, `1` and `-1`.** `x == 0`, `x == 1`, `x == -1`,
  `x == 0.0` and `x == 1.0` are all silent. That is the first half of the
  `magic-numbers` prompt carve-out, word for word.
- **The declaration carve-out is reproduced.** `PLR2004` reads a comparison and
  nothing else, so it never reads a literal a declaration names, a default
  parameter, or an index.
- **`100` REPORTS.** The prompt rule carves out "conventional values (a `<< 8`,
  `100` for percent)", and `x == 100` reports:

      PLR2004 Magic value used in comparison, consider replacing `100` with a
      constant variable

  `x == 3600`, `x == -2` and `x > 100.0` report the same way.
- **`a << 8` is silent, and not for the prompt rule's reason.** It is silent
  because a shift is an operation and not a comparison. `x == 8` reports. So the
  shift form of the conventional carve-out survives by accident, and the percent
  form does not survive at all.

`ruff` cannot restore the `100` carve-out. `lint.pylint.allow-magic-value-types`
selects TYPES, never values — it takes `str`, `bytes`, `int`, `float` and
`complex`, and naming `int` there silences EVERY integer, which turns the rule
off rather than carving one value out. There is no `allow-magic-values` key;
`ruff` answers a config that names one with `unknown field
'allow-magic-values'`.

## The exemption is a `# noqa` on the comparison

A conventional value the review must not report carries `# noqa: PLR2004` on the
line the tool reports, which is the line the comparison stands on. Write the
reason after the code:

    if usage == 100:  # noqa: PLR2004 — a whole ratio, in percent

Measured on the same probe: that line is silent, `# noqa: PLR2004` with no
reason is silent, and a bare `# noqa` is silent. The reason is not decoration.
It says which conventional value the number is, which is the one thing `ruff`
cannot read.

That is the exemption, and it is the only one. `builtin/validators/README.md`
states the contract this rule keeps: "Selection in the pipe is attribution, not
exemption ... To exempt one code item, use an inline suppression in the code —
never the pipe." A `jq` step that dropped the `100` findings would be exemption
in the pipe, and it would drop a genuine `status == 100` along with the percent
one, because the pipe reads the value and never the meaning. The `# noqa` reads
the meaning, because the author writes it at the one site that has one.

## This rule and `magic-numbers-dart` are the two of five that cannot allow `100`

Three of the five tools take a usable value allow-list, and the allow-list is
where the percent carve-out goes:

- `magic-numbers-swift` states `allowed_numbers: [0, 1, -1, 100]`.
- `magic-numbers-typescript` states `ignore: [0, 1, -1, 100]`.
- `magic-numbers-go` states `ignored-numbers: ["0", "1", "-1", "100"]`. The
  `mnd` key takes strings, and the values are the same four.

`ruff` and `solid_lints` are the two tools of the five that give no usable value
allow-list, and each fails in its own way. `ruff` states no allow-list key at
all, as the paragraph above measures. `solid_lints` 0.3.3 states an `allowed`
key that its own parameter parser cannot read, so `magic-numbers-dart` keeps the
built-in default `[-1, 0, 1]` and that file records the measurement.

So `x == 100` reports here, `part * 100` reports in Dart, and the divergence
belongs to the tool in each case. The `# noqa: PLR2004` above is the recourse
here: a percent comparison carries the marker and the reason, and the review
then stays silent on it. `magic-numbers-dart` states its own marker for the same
purpose.

## Where this rule is NARROWER than the rule it supersedes

`supersedes: magic-numbers` is a claim, so state its limit. `PLR2004` reads a
comparison and nothing else. A repeated literal in a call argument, in an
operation, or in a `return` is never reported — measured: `a * 100`,
`a + 3600`, `g(3600)` and `return 3600` are all silent. Repetition is the prompt
rule's primary target, so for Python this tool answers the position question and
leaves the repetition question unanswered.

That gap is real and it is the price of the trade. `mnd` reads six positions,
`no-magic-numbers` reads three, and `no_magic_number` reads every position its
own carve-out list does not exempt; `PLR2004` reads one, which makes this the
narrowest of the five `magic-numbers-*` rules. A Python reviewer gets one
comparison verdict every reviewer gets the same, in place of a repetition count
an agent makes by eye.

## How the run is shaped

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration.
`--no-cache` keeps ruff from writing a cache directory into the workspace.

The scope is `files` because ruff reads the files it is given.

The fixture pair holds every statement above that a run can check. The failing
fixture carries `404`, `4096`, `10`, `90` and `100`, and the acceptance test
`the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value`
holds the run to exactly those five: `100` proves the carve-out is absent, and
the count proves no other position reports.
