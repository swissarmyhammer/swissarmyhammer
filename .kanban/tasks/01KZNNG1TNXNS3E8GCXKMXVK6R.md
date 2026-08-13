---
assignees:
- claude-code
position_column: todo
position_ordinal: ffbc80
title: function-length-python reports test functions and wide field-setting __init__ methods
---
`builtin/validators/code-hygiene/rules/function-length-python.md` runs `ruff check --isolated --select PLR0915` at `max-statements=180` and declares `supersedes: [function-length]`.

Two carve-outs of `function-length.md` are dropped.

- "Functions explicitly marked as tests". `PLR0915` applies to any `def`, and `--isolated` discards the `per-file-ignores` entry a project holds for tests. A long `def test_end_to_end` reports.
- "Initialization functions that set many fields". Each `self.x = ...` is one statement, so a 200-field `__init__` reports at 200 > 180.

"Generated code" is dropped as well; ruff has no generated-file heuristic.

"Functions that are mostly configuration/data" IS reproduced, by accident of the metric: a 400-line dict or list literal is one statement.

`# noqa: PLR0915` works. Decide how the rule states the test carve-out, and whether a field-setting `__init__` needs its own answer.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity