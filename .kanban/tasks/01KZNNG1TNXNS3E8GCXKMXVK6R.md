---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m01ecsfj00e8hyhcdtav1e6b
  text: |-
    ### Research — the ruff tool space, surveyed

    Enumerated every ruff 0.14.5 configuration option with `ruff config`, walking each group to its leaves: **166 leaf options**. Grepped the whole list for a name, decorator or base-class mechanism.

    - **No option exempts a function by NAME.** The name-reading options each belong to their own linter and reach no other rule: `lint.pep8-naming.ignore-names` (N rules), `lint.flake8-self.ignore-names` (SLF), `lint.pydocstyle.ignore-decorators` (D), `lint.flake8-type-checking.runtime-evaluated-decorators` (TC), `lint.pylint.allow-dunder-method-names` (PLW3201 alone).
    - **No option exempts a class by its BASE.** No option reads a base class at all.
    - **No generated-file heuristic.** No option holds the word.
    - `lint.pylint` holds 11 settings; `max-statements` is the only one PLR0915 reads.
    - Every exemption ruff offers for PLR0915 is a PATH (`per-file-ignores`, `extend-per-file-ignores`, `exclude`) or an in-code `# noqa`.

    **The one thing ruff DOES name: the diagnostic range.** ruff writes no function name into the message — `Too many statements (200 > 180)` — but it anchors each `PLR0915` finding on the function's NAME token. Measured: `    def test_dense(self):` reports at row 2, column 9 to column 19, which is exactly `test_dense`; `async def test_async_thing()` under two decorators reports at column 11 to 27, which is `test_async_thing`; `def café_été(é)` reports at column 5 to 13, so columns count CHARACTERS. The text before the name on a `def` line is always ASCII, so the slice is exact.

    ### What `--isolated` costs and what it buys — measured

    One probe of `src.py` and `tests/test_thing.py`, three functions of 200 statements, against the shipped command line:

    | the project's own `pyproject.toml` | `--isolated` | the project's configuration |
    |---|---|---|
    | none | 3 | 3 |
    | `[tool.ruff.lint] ignore = ["PLR0915"]` | 3 | 3 |
    | `[tool.ruff.lint.pylint] max-statements = 500` | 3 | 3 |
    | `[tool.ruff.lint] select = []` | 3 | 3 |
    | `[tool.ruff] exclude = ["src.py", "tests"]` | 3 | 3 |
    | `[tool.ruff.lint.per-file-ignores] "tests/*" = ["PLR0915"]` | 3 | **1** |
    | `[tool.ruff.lint.per-file-ignores] "*" = ["PLR0915"]` | 3 | **0, exit 0** |
    | `[tool.ruff.lint.extend-per-file-ignores] "*" = ["PLR0915"]` | 3 | **0, exit 0** |
    | `[project` — not TOML at all | 3 | **nothing, exit 2** |

    `--select` on the command line beats the project's `ignore` and `select`, and `--config` beats the project's `max-statements`, so `per-file-ignores` is the ONE project setting that reaches this gate. That is exactly the setting the card names, and it is also the setting that silences the WHOLE gate at exit 0 with `"*"`. `--isolated` stays: it is the sibling `dead-code-python` hazard (^bdh09pb) in ruff's own form.

    ### The corpus, and the three carve-outs measured

    Seven repositories, 9335 `.py` files, at the gate of 180: **14 findings**.

    - Carve-out 1, TESTS: 6 of the 14 carry a name that starts with `test`. 8 stand in a test PATH — so a path exclusion drops 2 more, and both are helpers the prompt rule still lists: `ansible` `_validate_argument_spec` (371 statements) and `importer.py` `main` (246). Same trade the Go sibling refused.
    - Carve-out 2, FIELD-SETTING INITIALIZERS: 1 `__init__` reports, `cpython312/Lib/idlelib/editor.py:74`, 206 statements of which **23** set a field. A census of every function of the corpus puts the widest field setter at **55** `self.x = ...` (`libregrtest/main.py`). The gate stands 3.3 times above it. No field-setting initializer in 9335 files reaches the gate.
    - Carve-out 3, GENERATED CODE: 0 findings in a file whose head carries a generated mark. Python states no convention, which is the verdict `complexity-python` already records.

    ### Discovered: ruff writes parse failures onto the same stream

    `ruff check --select PLR0915 --output-format json` emits `"code": "invalid-syntax"` rows beside the `PLR0915` rows. The shipped `jq` filter selects EVERY row, so today a file that does not parse is reported as a function-length finding — `invalid-syntax Simple statements must be separated by newlines or semicolons` — while the rule body claims "Selection in the pipe is attribution". 7 files of the corpus do this, every one a deliberate fixture of a test suite (`cpython Lib/test/tokenizedata/badsyntax_3131.py`, `django tests/test_runner_apps/tagged/tests_syntax_error.py`, 5 lib2to3 Python-2 grammar files). The name filter has to select `PLR0915` to read a name at all, so the run now names the parse failure on stderr and breaks, the way `complexity-python` does.

    Also measured, NOT fixed here: ruff exits **0** and writes only `warning: Failed to lint <path>` to stderr for an absent path, a non-UTF-8 file and a file with no read permission, so such a file reads as clean. That is a separate defect of the same script; a card follows.
  timestamp: 2026-08-15T00:50:48.946912+00:00
- actor: claude-code
  id: 01m01fv7p9ay5yth1w7hbz1jsm
  text: |-
    ### The three decisions

    **Carve-out 1, TESTS — the run drops it, by the DEFINITION.** ruff offers no name mechanism, so the script reads the name ruff itself anchored the finding on: `location.column` to `end_location.column` on `location.row` is exactly the function's NAME token. The filter drops a name that starts with `test`, which is what pytest (`python_functions = ["test"]`) and unittest (`testMethodPrefix`) each collect a test by, and it is the same predicate the sibling `complexity-python` already ships for this language. No mandatory marker, no path.

    Go's rule reads BOTH halves of `go test` — the name and `_test.go` — because `go test` requires both. Python requires neither half: pytest's `python_files` and unittest's discovery pattern are configuration, not a language rule, and the prompt rule refuses the file name outright. So this rule reads the name and nothing else.

    **Carve-out 2, A FIELD-SETTING `__init__` — no answer of its own, and the measurement says why.** Every function of the 9335 corpus files was parsed and its `self.x = ...` statements counted. The widest field setter in the whole corpus sets **55** fields (`cpython Lib/test/libregrtest/main.py`), against a gate of 180 — the gate stands 3.3 times above the shape. The one initializer that does report, `cpython Lib/idlelib/editor.py:74`, holds 23 field sets in 206 statements and is a long procedure, not the exempt shape. So the initializer reports and the author writes `# noqa: PLR0915`, which is the verdict `function-length-go` and `complexity-swift` each record for the same carve-out.

    **Carve-out 3, GENERATED CODE — nothing answers it.** ruff has no heuristic and Python states no convention. Same verdict as `complexity-python`, and now held by an acceptance test so the gap stays measured.

    **`--isolated` stays.** It costs the project's `per-file-ignores`, which is the one project setting that reaches this gate — and it is also the setting that turns the WHOLE gate off at exit 0 with `"*"`. The rule now makes the test carve-out itself, so the cost is paid.

    ### The carve-out measurement, before and after

    Over the corpus, at the gate of 180:

    | the run | findings | in a test path |
    |---|---|---|
    | no test carve-out (before) | 14 | 8 |
    | the shipped name filter (after) | 8 | 2 |
    | a path exclusion instead | 6 | 0 |

    The path exclusion drops 2 helpers the prompt rule still lists — `ansible` `_validate_argument_spec` at 371 statements and `importer.py` `main` at 246. Verified by driving the SHIPPED script over the corpus: ansible 6, django 0, fastapi 0, flask 0, pandas 0, requests 0, cpython 2 — 8 total, and the 2 it keeps in a test path are exactly those helpers.

    ### RED first

    `cargo nextest run -p swissarmyhammer-validators -E 'test(the_shipped_python_function_length)'` before the rule changed:

    - `..._reads_a_test_from_its_definition` FAILED — reported `["suite/staged_test.py:1", "suite/staged_test.py:194"]`, the test beside the helper.
    - `..._breaks_on_a_file_it_cannot_parse` FAILED — reported the parse failure as a finding: `invalid-syntax Expected an indented block after `if` statement`.
    - `..._reports_a_field_setting_initializer` and `..._reports_a_generated_file` passed already; each is a guard the change must not break.

    After: 5 of 5 pass. Whole crate: 753 of 753.

    ### The fixture pair

    The fail fixture gains `Editor.__init__` of 190 field-setting statements, which the tool must report. The pass fixture gains `test_mix_state_rounds` of 190 statements, which the name carve-out drops, and the same `Editor.__init__` behind `# noqa: PLR0915`. `every_shipped_complexity_tool_rule_passes_its_fixtures` covers the pair and passes.

    ### One README correction

    `builtin/validators/README.md` quoted this rule's frontmatter as its example of "all 25 lines of it, and its `run` is ... plus one pipe". That is no longer true of this rule, so the example now quotes `rules/magic-numbers-python.md`, which is the same shape and the same 25 lines.

    ### Left for another card

    `^d3j6sbt` — ruff exits 0 and writes only `warning: Failed to lint <path>` for an absent path, a non-UTF-8 file and a file with no read permission, so such a file still reads as clean. The status gate this card added accepts 0 and 1, so it admits all three.

    ### implement — changed
    - evidence: 8 files — `builtin/validators/code-hygiene/rules/function-length-python.md`, `builtin/validators/code-hygiene/fixtures/function-length-python.fail.py.tmpl`, `builtin/validators/code-hygiene/fixtures/function-length-python.pass.py.tmpl`, `builtin/validators/README.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity_python.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/function_length_go.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/temp_directory.rs`. `cargo nextest run -p swissarmyhammer-validators`: 753 passed, 0 failed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-15T01:16:10.825681+00:00
position_column: doing
position_ordinal: '8280'
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