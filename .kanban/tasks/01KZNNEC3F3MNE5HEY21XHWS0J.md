---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzscm8h5zd2wxdapm31ahw29
  text: |-
    Measured with ruff 0.14.5 and complexipy 7.0.0 before any edit. Two statements on the card are wrong, and a third defect the card does not state is the decisive one.

    CORRECTION 1 — the card says "The Sonar metric the prompt rule names charges a flat chain far less than McCabe does". It does not. Measured over the same 21-arm flat `if`/`elif` dispatch chain at nesting depth 1: `C901` scores 22, complexipy (Sonar cognitive complexity) scores 21. The prompt rule states the reason itself: "An `if` / `else if` / `else` chain is flat. Each branch adds 1." So the `cognitive-complexity` probe also scores that chain 21 against its gate of 15, and the "Configuration parsing with many options" carve-out is an exemption ON TOP of the score, not a property of the metric.

    Where the two metrics part is a `match` and a table. Measured over one function each:

    | the shape | C901 | complexipy |
    |---|---|---|
    | 21-arm flat `if`/`elif` | 22 | 21 |
    | 21-case `match` | 22 | 1 |
    | dict table plus `TABLE.get(mode, -1)` | 1 | 0 |
    | six nested `if` blocks | 7 | 21 |
    | test function of 20 flat `if`s | 21 | 20 |

    CORRECTION 2 — the card says "ruff offers no option for it" and leaves the author with nothing. It does not state the suppression an author CAN write. Measured over the 21-arm chain: `# noqa: C901` on the `def` line gives no finding; `# noqa:C901` with no space gives none; `# noqa: C901 keep, config table` with text after gives none; a bare `# noqa` gives none; `# noqa: C901` on the line ABOVE the `def` gives one finding.

    THE DECISIVE DEFECT, which the card does not state — `C901` is silent on the shape every sibling rule reports. Measured over one function of six nested `if` blocks, the same shape the Rust, Go and TypeScript probes use: `C901` scores it 7 against the gate of 15 and reports nothing; complexipy scores it 21 and reports. So `C901` INVERTS the prompt rule it supersedes: it reports the flat chain the prompt rule carves out, and it stays silent on the deep nesting the prompt rule is for.

    TOOL SURVEY, over the same flat chain and the same nested function:
    - ruff 0.14.5 rule set: `ruff rule --all` names `C901 complex-structure` (McCabe), `PLR0912 too-many-branches`, `PLR0915 too-many-statements`, `PLR0911 too-many-return-statements` and `PLR1702 too-many-nested-blocks`. No cognitive-complexity rule. `PLR1702` is preview-only — measured, `--select PLR1702` writes `warning: Selection PLR1702 has no effect because preview is not enabled`.
    - complexipy 7.0.0 (pypi, Rust, actively released): implements the published Sonar algorithm, `--max-complexity-allowed` is the gate, SARIF output carries the row and the function name. Measured numbers in the table above.
    - flake8-cognitive-complexity: last release 0.1.0, 2020, and it needs a flake8 host.
    - radon: cyclomatic, the same metric as C901.

    DECISION: complexipy replaces ruff C901 for this gate. It is the metric the prompt rule names, and it is the metric the Rust, Go, Swift and TypeScript siblings of this roster already measure.
  timestamp: 2026-08-11T21:46:01.125495+00:00
- actor: claude-code
  id: 01kzsdmqbpqxewwzjwcrwpgamd
  text: |-
    Implementation landed. `complexity-python` now runs `complexipy 7.0.0` at `--max-complexity-allowed 15`.

    HOW THE SCRIPT IS SHAPED, and what each part is measured against:
    - The run stands in the `mktemp -d` directory and names each file by an absolute path. Measured: complexipy reads `[tool.complexipy]` out of the `pyproject.toml` of its WORKING DIRECTORY — from the project directory with `max-complexity-allowed = 100` it reported 0 findings, and from a directory of its own it reported 1. It also writes a `.complexipy_cache` directory into its working directory on every run; from the workspace root that lands in the repository, and from the temporary directory the trap removes it.
    - The loop gives complexipy one file at a time. Measured over one ordinary file that scores 21 beside one file whose body never closes, in each order: one call reported the ordinary finding and exited 1, so a one-call script reported 1 finding and exited 0 and the engine read the unparsable file as clean. The loop reports no finding and exits 1.
    - The status gate accepts status 0 with a report on disk, and status 1 with a report holding one result or more. Measured, one file at each run: a finding = status 1 with one result; a clean file and an empty `.py` file = status 0 with no result; an absent path, a syntax error and a file with no read permission = status 1 with no result. complexipy writes its diagnosis to STDOUT and 0 bytes to stderr, so the script captures the console text and forwards it on a break.

    THE THREE CARVE-OUTS:
    - Tests — REPRODUCED IN THE RUN. The filter reads `logicalLocations[0].name` of the SARIF result, which is the bare name of a function and `Class::method` for a method, and drops a name whose last `::` segment starts with `test`. That is the DEFINITION's own name, which is the mark the prompt rule states, and it keeps a helper in a test file listed. Sources: pytest 9.1.1 `python_functions = ["test"]`, and `unittest.TestLoader.testMethodPrefix`. Measured over one `suite/staged_test.py` holding `TestThing.test_method` at row 2 and `build_request` at row 13, each scoring 21: without the filter 2 rows, with the filter row 13 alone. The sibling `missing-docs-python` reads the same two sources.
    - Configuration parsing — THE AUTHOR ANSWERS IT. complexipy holds no flag for a flat list of simple cases, and the metric charges the chain 21. Two answers, and the first is measured: the same 21 arms written as a `match` score 1, and written as a dict beside `TABLE.get(mode, -1)` score 0. The second is `# complexipy: ignore` on the `def` line.
    - Generated code — UNREACHABLE, named on the rule with the measured reason. complexipy reads no file header: the file whose head carries `# Generated by the protocol buffer compiler.  DO NOT EDIT!`, the file whose head carries `# @generated`, and the plain file each reported. Its one file filter, `--exclude <glob>`, reads the PATH and reaches no file named on the command line: over the three files named as arguments, `--exclude 'DO NOT EDIT'`, `--exclude '*_pb2.py'` and `--exclude 'marked_pb2.py'` each dropped none; over the same three walked as a directory the exclude dropped one. Python states no generated-file header convention, so a header test would name one generator and never the convention. The author cannot annotate it either, because the generator writes the file again.

    THE ANNOTATION, measured over one function that scores 21. No finding: `# complexipy: ignore` on the `def` line; the same on the line directly above the `def`; `# noqa: complexipy`; `#complexipy:ignore`; `# Complexipy: Ignore`; the directive with text after it. One finding: a blank line between the directive and the `def`; the directive on the first line of the body; the directive under the docstring; a bare `# noqa`; `# noqa: C901`.

    RED to GREEN, with the real tool over the SHIPPED bytes: with the old ruff rule restored in place and `builtin/mod.rs` touched, the 5 Python complexity acceptance tests all FAIL; with the new rule they all PASS.

    INSTALL, both commands measured: `uv tool install complexipy==7.0.0` and `pipx install complexipy==7.0.0` (run with `PIPX_HOME`/`PIPX_BIN_DIR` under a scratch directory) each land a working `complexipy 7.0.0`.

    FIXTURES unchanged. The shipped pair still holds: the fail fixture's `classify_reading` scores 21 and the pass fixture's `classify_reading` scores 12 beside `_sample_step` at 3.

    Two coverage counts moved because the new script makes a temporary directory: `TEMP_DIRECTORY_RULE_COUNT` 16 to 17, and the zero-argument split 10/6 to 9/7. `FILES_SCOPE_RULE_COUNT` and `SHIPPED_TOOL_RULE_COUNT` are unchanged.

    DOCS that named the old tool are corrected: `builtin/validators/README.md` (the one-pipe example moved to `function-length-python`, whose frontmatter it now quotes word for word), `code-hygiene/VALIDATOR.md` (three of five gates are now the Sonar metric, and Python moves to the group that keeps the nesting gate inside the one score), and the parser unit test that mirrored the README example.
  timestamp: 2026-08-11T22:03:44.886411+00:00
- actor: claude-code
  id: 01kzsdnrqdsnjydkkh89ape312
  text: |-
    ### implement — changed
    - evidence: 8 files — builtin/validators/code-hygiene/rules/complexity-python.md, builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/README.md, crates/swissarmyhammer-validators/src/validators/parser.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/temp_directory.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/zero_argument.rs
    - tests: `cargo nextest run -p swissarmyhammer-validators` 652 run, 652 passed. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` 3415 run, 3411 passed, 4 failed — the three `review_e2e` tests missing the duplication claim and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`, all four red before this change. `cargo fmt --all` and `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - RED to GREEN: the 5 Python complexity acceptance tests fail against the old ruff rule and pass against the shipped complexipy rule, both runs after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`.
    - next: /review
  timestamp: 2026-08-11T22:04:19.053634+00:00
- actor: claude-code
  id: 01kzsgdnfxdsmkkjyyvs7qmy25
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit 862ad273f) — 16 validators attempted, 0 failed, 0 skipped; 3 findings raised, 3 confirmed, 8 refuted. All 3 confirmed findings ask to change `.unwrap()` to `.expect()` at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:347`, `:472` and `:731`. The delta touches that file only at lines 6-9 and 815-852 (`git diff --unified=0` hunks `@@ -6,3 +6,3 @@` and `@@ -814,0 +815,38 @@`), so all three lines are test code that already existed. The review skill drops a finding that asks to restyle test code that already existed. 0 open findings.
    - measurements made against the real tools, all confirmed:
      - Metric table in the rule file. Measured with ruff 0.14.5 and complexipy 7.0.0 over one function of each shape: 21-arm flat `if`/`elif` — C901 22, complexipy 21. The same arms as a `match` — C901 22, complexipy 1. The same arms as a dict beside `TABLE.get(mode, -1)` — C901 1, complexipy 0. Six nested `if` blocks — C901 7, complexipy 21. A `for` holding an `if` holding an `if`/`elif`/`else` — complexipy 8. `ruff --select PLR1702` writes `warning: Selection PLR1702 has no effect because preview is not enabled` and reports nothing.
      - Fixtures. `complexity-python.fail.py.tmpl` `classify_reading` scores 21; `complexity-python.pass.py.tmpl` `classify_reading` scores 12 and `_sample_step` scores 3. The shipped script over the pair reports `fail.py:9` alone.
      - Gate. `--max-complexity-allowed 15` reports a function at 16 and stays silent at 15, which is the wording of the rule file ("over the gate"). The command-line gate wins over a project `pyproject.toml` that states `max-complexity-allowed = 100`.
      - Working directory. complexipy reads `[tool.complexipy]` out of the `pyproject.toml` of its working directory, and it writes `.complexipy_cache` into that directory. The shipped script left no `.complexipy_cache` in the repository root.
      - Fail-closed behaviour, one file at each run of the shipped script: a finding — 1 row, exit 0. A clean file — no row, exit 0. An empty `.py` file — no row, exit 0. A path holding a space — 1 row, exit 0. A missing path, a syntax error, a file with no read permission, a dangling symlink, a latin-1 file and a binary file — no row, exit 1 with the tool's console text on stderr. Two files with the unparsable one first and last — no row, exit 1 in each order. No input reached the engine as a clean tree.
      - Name filter. Measured over one file holding each shape, each scoring 21: dropped `test_method`, `testify` and `test_outer`; kept `_deep`, `build_request`, `TestCase::Test_helper`, `TestCase::build_request` and `outer_plain`. complexipy names a method `Class::method` and folds a nested function into the enclosing definition, so `test_inner` inside `outer_plain` reports as `outer_plain`. `Test_helper` stays listed because `startswith("test")` reads case, and unittest's `testMethodPrefix` is lowercase `test`.
      - Annotation. No finding: `# complexipy: ignore` on the `def` line, the same on the line above, `# noqa: complexipy`, `#complexipy:ignore`, `# Complexipy: Ignore`, and the directive with text after it. One finding: a blank line between the directive and the `def`, the directive on the first body line, the directive under the docstring, a bare `# noqa`, and `# noqa: C901`.
      - Install and doctor. `uv tool install complexipy==7.0.0` (with `UV_TOOL_DIR`/`UV_TOOL_BIN_DIR` under a scratch directory) and `pipx install complexipy==7.0.0` (with `PIPX_HOME`/`PIPX_BIN_DIR` under a scratch directory) each landed a working `complexipy 7.0.0`. `check_version_command` prints `7.0.0`, the measured version. `check_command` names `complexipy`, `jq` and `mktemp`, which are the three tools the script runs.
      - Path contract. `resolve_file` states that a `review file` path is repo-relative and rejects an absolute path, so the `"$root/$file"` the script builds is safe.
      - Set-wide guards. `temp_directory.rs` raised `TEMP_DIRECTORY_RULE_COUNT` 16 to 17 and changed one doc word; every assertion is unchanged, and complexity-python now makes a temporary directory, so the count is correct. `zero_argument.rs` changed one doc sentence alone (10/6 to 9/7); measured over the shipped set, 16 files-scope rules write the guard, 9 on the first line and 7 under `set -e`. No guard was weakened.
      - Docs. The README quotes the frontmatter of `function-length-python.md` word for word, and that frontmatter is 25 lines, the number the README states.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` — 652 run, 652 passed. The 5 named Python complexity acceptance tests pass.
    - next: none — the card is done.
  timestamp: 2026-08-11T22:52:19.325071+00:00
- actor: claude-code
  id: 01kzsgezgpvtwzsgm1hxsgvt2z
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 9 files; complexipy replaces ruff C901, because C901 scored a six-deep nested function 7 and stayed silent at the gate of 15, where complexipy scores it 21
    - test: green — cargo nextest -p swissarmyhammer-validators 652 run, 652 passed; rdeps 3415 run, 3411 passed with only the 4 failures that came before
    - commit: 862ad273f
    - review: clean — 16 validators attempted, 3 findings, all 3 dropped as test code that already existed, outside both hunks of the delta; the card moves to done
  timestamp: 2026-08-11T22:53:02.358238+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe780
title: complexity-python supersedes cognitive-complexity but C901 reports flat config chains and tests
---
`builtin/validators/code-hygiene/rules/complexity-python.md` ran `ruff check --isolated --select C901` at `max-complexity=15` and declared `supersedes: [cognitive-complexity]`.

`C901` is McCabe cyclomatic complexity. The prompt rule it supersedes states the published Sonar cognitive complexity. Measured with ruff 0.14.5 and complexipy 7.0.0, the two metrics disagree in BOTH directions, and the second direction is the defect:

- A 21-arm flat `if`/`elif` dispatch chain at nesting depth 1: `C901` scores 22, Sonar scores 21. Both are over the gate of 15. The card first claimed Sonar charges a flat chain far less; measured, it does not.
- One function of six nested `if` blocks — the shape the Rust, Go and TypeScript probes of this roster use: `C901` scores 7 and reports NOTHING; Sonar scores 21 and reports. `C901` therefore stayed silent on the deep nesting the prompt rule exists for.

`cognitive-complexity` exempts three things, and the rule states each one:

- **Tests.** The prompt rule marks a test at the DEFINITION, never by the file name.
- **Generated code and macro expansions.**
- **Configuration parsing with many options**, where the score comes from a long flat list of simple cases rather than from nesting.

Decide whether ruff is the right tool for this gate, and state each carve-out as reproduced in the run, reachable by an annotation the author writes, or unreachable with the measured reason.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity