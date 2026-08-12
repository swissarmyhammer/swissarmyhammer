---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzqex18jg0tdbssz5r6ty2zh
  text: |
    ### Measurement first — the card premise HOLDS, with three refinements

    Reproduced on ruff 0.14.5 before any edit. Each of the three claims is true.

    1. **Tests.** One file that holds no module docstring, `def test_foo()`, `def helper_for_tests()`, `class TestThing` and `def test_method`: ruff reports 5 — `D100` r1, `D103` r4, `D103` r8, `D101` r12, `D102` r13. The card named the first two; the class and the method report as well.
    2. **Magic methods.** `D105` fires on `def __str__`. Refinement the card did not hold: `D105` does NOT cover every dunder. Measured on one class with 13 undocumented magic methods — `__init__` reports `D107`, `__new__` and `__call__` each report `D102`, and the other ten (`__str__`, `__repr__`, `__eq__`, `__hash__`, `__len__`, `__iter__`, `__enter__`, `__exit__`, `__getattr__`, `__add__`) each report `D105`. The three that keep a code of their own are the three that take the author's own parameters. So leaving `D105` out exempts a magic method whose signature the language fixes, and no other.
    3. **Getters.** `D102` fires on `@property def paired`. Two refinements: ruff never reports the setter under it (`@paired.setter def paired` and `@gone.deleter def gone` each report nothing, while `@other.setter def renamed` and `@a.b.setter def deep` each report — the decorator must name the function it stands above); and ruff DOES hold a setting, `lint.pydocstyle.ignore-decorators`, which the card did not name.

    ### Three defects the card did not name — the run could answer 0 for a broken tool

    - **ruff exits 2** for a bad selector (`--select ZZ999`). The old pipe ended in `jq`, so the run exited 0 with no output.
    - **ruff exits 0 for a file it cannot read.** A path that is not there prints `[]`, exits 0, and puts `warning: Failed to lint ...` on stderr alone. That reads as a clean file.
    - **ruff reports an unparsable file under the code `invalid-syntax`.** The old pipe turned it into a FINDING with the claim `invalid-syntax unexpected EOF while parsing`, so a review reported a parse failure as a documentation defect.
  timestamp: 2026-08-11T03:47:16.882782+00:00
- actor: claude-code
  id: 01kzqexfwkrn1wdvfabvv2dbgg
  text: |
    ### The two decisions the card asked for

    **The selector.** `--select D100,D101,D102,D103,D104,D106,D107`. Seven of the eight `D1` codes. `D105` is left out, and that is the whole obvious-implementation carve-out: ruff itself keeps `__init__`, `__new__` and `__call__` outside `D105`, so the exemption reaches only a magic method whose signature the language fixes. No hand-written name list, because the tool's own group is the boundary.

    **The test carve-out.** Stated as the ITEM'S OWN NAME, never a path. The parent card ^h7garpc set the constraint word for word: "Do NOT exclude test code by path or by glob as a substitute for judgment", and `missing-docs.md` asks for the same test. ruff has no name filter, so the script reads the DEFINITION LINE each finding stands on and drops a `D102`/`D103` whose line reads `def test...` and a `D101`/`D106` whose line reads `class Test...`.

    The prefixes are read, not written: pytest 9.1.1 holds `python_classes = ["Test"]` and `python_functions = ["test"]`, and `unittest.TestLoader.testMethodPrefix` is `test`. The row is safe to read — measured, `D101`, `D102`, `D103`, `D106` and `D107` each report at the row of the `def` or `class` line, and a decorator above it does not move the row.

    `--per-file-ignores` on pytest's file patterns was refused. It is path-shaped, and it would silence `helper_for_tests` in a test file — the one shape `missing-docs.md` names word for word as still needing a docstring.

    ### The getter keeps its requirement, as it does for Go and Dart

    `lint.pydocstyle.ignore-decorators=['property']` was measured and declined. It silences the `@property` getter and leaves the `@functools.cached_property` getter beside it reporting, because it matches a whole decorator name. The carve-out asks for a SIMPLE getter, and the setting has no form for "simple". The fail fixture carries a getter, and the acceptance test holds ruff to reporting it.

    ### Two facts the rule now states rather than leaving to be found

    - `D100` and `D104` ask more than the prompt rule, whose list names no module. PEP 257 asks for a docstring on every module and package, and the prompt rule yields to a stricter language rule.
    - A test module keeps its `D100`. A module has no name marker on the line the finding stands on, and its only test marker is the FILE NAME, which the prompt rule refuses.
  timestamp: 2026-08-11T03:47:31.859481+00:00
- actor: claude-code
  id: 01kzqexzjqvwv1ghekpwa03w0d
  text: |
    ### RED to GREEN, proved with the real tool

    Round 1 — new tests against the OLD script and the NEW fixtures. All 4 fail on the fixture pair:

        the shipped tool rule `missing-docs-python` must plan a run; fallbacks:
        [... detail: "fixtures failed: the pass fixture missing-docs-python.pass.py.tmpl
        produced 6 finding(s); none are allowed"]

    Round 2 — OLD script AND OLD fixtures, so the rule plans healthy and each assertion is what fails:

    - `..._breaks_on_a_file_it_cannot_read` — `the run must report exactly one tool error; got []`, left 0, right 1.
    - `..._reports_every_fail_fixture_item` — `the fail fixture must report the undocumented item 'class UndocumentedNested:'; the run reported ["class UndocumentedClass:", "def undocumented_method(self) -> None:", "def undocumented_function() -> None:"]`.
    - `..._breaks_on_a_file_it_cannot_parse` — `a file the tool never judged must report no finding; got [... claim: "invalid-syntax unexpected EOF while parsing" ...]`.
    - `..._reads_the_item_name_and_not_the_path` — 18 findings against the 6 the new rule makes; the 12 extra are the test class, the test method and the test function at each of the four staged positions.

    GREEN — the new rule and fixtures restored: `cargo nextest run -p swissarmyhammer-validators` 591 tests run, 591 passed, 0 skipped. `cargo nextest run -p mirdan` 503 passed. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` both clean.

    ### One shared helper rather than a near-copy

    `ShippedBrokenRun` + `verify_shipped_run_breaks` now hold "the run reports no finding and one error that names what broke". The Go test `the_shipped_go_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse` was moved onto it, and the two new Python probes use it. `ShippedRun`, `ShippedFailFixture` and `ShippedStagedPositions` are reused as they stand.

    ### One more file the change touched

    `builtin/validators/README.md` quoted `rules/missing-docs-python.md` as its schema example, and that quote is no longer the file's content. The example now quotes `rules/complexity-python.md`, the shortest shipped tool rule, and it points at `missing-docs-python.md` as the shape a tool that needs several steps takes.
  timestamp: 2026-08-11T03:47:47.927021+00:00
- actor: claude-code
  id: 01kzqey8f3e9e7mm98b77rq4vd
  text: |
    ### implement — changed
    - evidence: 6 files. Rule: `builtin/validators/code-hygiene/rules/missing-docs-python.md`. Fixtures: `builtin/validators/code-hygiene/fixtures/missing-docs-python.{fail,pass}.py.tmpl`. Tests: `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`. Contract: `builtin/validators/README.md`. Commands: `cargo nextest run -p swissarmyhammer-validators` 591 passed 0 failed; `cargo nextest run -p mirdan` 503 passed; `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED proved twice with the old rule, GREEN with the new one.
    - next: /review
  timestamp: 2026-08-11T03:47:57.027867+00:00
- actor: claude-code
  id: 01kzqfh0950paehg1h3ypesryh
  text: |-
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (b51f054ef), plus the three checks the diff-scoped engine cannot make.
    - engine: 1 finding, 1 confirmed. It named `dead_code_work` and `complexity_work` in `tests/shipped.rs` as near-duplicates. Both lines come from `087d052b43`, which is before this change. The written rule drops a finding that asks to change test code that was there before. The finding is dropped.
    - new helpers: this change adds 404 lines to `tests/shipped.rs`. The added lines are `const` probe data and 4 test functions that use the `ShippedFailFixture`, `ShippedStagedPositions` and `ShippedBrokenRun` shapes. No new duplicate helper function.

    Check 1, claims and names. Each claim in the rule body was measured again with ruff 0.14.5. All agree.
    - 13 magic methods give 1 `D107`, 2 `D102` (`__new__`, `__call__`) and 10 `D105`. This agrees with the body word for word.
    - `_private_method` and `_private_function` report nothing.
    - `# ruff: noqa: D1` silences nothing.
    - The three test names the body cites are real: `the_shipped_python_missing_docs_tool_rule_reports_every_fail_fixture_item`, `..._breaks_on_a_file_it_cannot_read`, `..._breaks_on_a_file_it_cannot_parse`.
    - The `supersedes` target `missing-docs` is real.

    Check 2, exemptions. All are objective. The script was run end to end.
    - The name filter reads the name of the item, not the path. Proof: `helper_for_tests` in `tests/test_thing.py` keeps its finding, and `test_connection` in an ordinary module loses its finding.
    - On that file ruff reports 5 findings and the rule reports 2. This agrees with the body.
    - The body states where the prefixes come from: pytest 9.1.1 `python_classes` and `python_functions`, and `unittest.TestLoader.testMethodPrefix`.
    - `D105` is out of the selector, and the body explains the reason.
    - The three silent-zero defects are corrected. A bad selector exits 1. An unparsable file exits 1 and reports no finding. A path that is not there exits 1 and names the file. The script holds no shell pipe.

    Check 3, stale text. 2 findings, in the description.
    - `missing-docs.md` names Swift and Rust with "e.g.", so the sentence is an example and not a limit. It is not stale.
    - next: correct the two items, then review again.
  timestamp: 2026-08-11T03:58:11.237079+00:00
- actor: claude-code
  id: 01kzqfhv0bw6qtw1na4khjyx1q
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 6 files; the selector drops D105 for the obvious-implementation carve-out; the test carve-out reads the item name, not the path; three silent-zero defects fixed
    - test: green — cargo nextest run -p swissarmyhammer-validators, 591 passed; RED proved twice, GREEN after; clippy -D warnings clean
    - commit: b51f054ef — 6 files
    - review: findings — builtin/validators/README.md:71, crates/swissarmyhammer-validators/src/validators/parser.rs:1655
  timestamp: 2026-08-11T03:58:38.603707+00:00
- actor: claude-code
  id: 01kzqhd8f30fxar81myrjvtvxp
  text: |-
    ### review — findings (second pass, deeper audit)

    A second audit of the rule body found 8 more items. I measured each one myself before I wrote it.

    - `README.md:108` and `:117` still teach the pipe. This is the cause behind the three silent-zero defects the change corrects. The rule was corrected; the instruction that made the defect was not.
    - `README.md:68` names `rules/complexity-python.md` but the `commands:` line is not the same. The line came through unchanged from the earlier example.
    - `missing-docs.md:33` does not agree with the Python rule. Python keeps the obvious-implementation carve-out on purpose, because `D105` is out of the selector. The note says the carve-outs "do not apply". This corrects my earlier comment, which said the note is not stale. I read it then as an example list with "e.g.", and I missed the true problem: the note is wrong for a Python magic method.
    - `missing-docs-python.md:23` answers for files nobody gave it. Measured with no argument: 4 findings over 2 files that were never given, exit 0. Same class as the three defects the change corrects.
    - `missing-docs-python.md:15` leaves a temp directory after each run. No `trap` is in the file.
    - `missing-docs-python.md:126` omits `async`. Measured: `async def test_async` is dropped, `async def helper` keeps its `D103`.
    - `missing-docs-python.md:194` measures the selector half only. The configuration half cannot occur, because the script gives `--isolated`.
    - `missing-docs-python.md:198` states a mechanism that is not correct. Measured: on `--select ZZ999` ruff writes 0 bytes to stdout, so the `cat` writes nothing. Ruff writes the 93-byte diagnostic to stderr itself.

    The result of the change is still good. The selector, the name filter and the three exit-status corrections all behave as the body says. These 8 items are about text that does not agree with the code, and 2 defects in the script.

    - evidence: 10 open findings, in the description.
    - next: correct the 10 items, then review again.
  timestamp: 2026-08-11T04:31:05.699752+00:00
- actor: claude-code
  id: 01kzqhm0nwnm1w5f3rpjqy3c6p
  text: |
    ### Both findings corrected, and the cause swept from each file

    **Finding 1 — `builtin/validators/README.md`.** The superlative is removed, and the sentence now states two counts I measured.

    The measurement agrees with the finding. Over each shipped rule that holds a `tool:` block, `complexity-python.md` is 59 lines, `missing-docs-rust.md` is 43 and `missing-docs-swift.md` is 47. By frontmatter, `complexity-python.md` is 22 lines, equal to four other rules. The rule is the shortest by neither measure.

    The new sentence: "That is the frontmatter of `rules/complexity-python.md`, all 22 lines of it, and its `run` is one pipe. A rule whose tool needs several steps writes a script rather than one pipe; `rules/missing-docs-python.md` is one of those, and its script is 47 lines."

    Two counts, each measured. `complexity-python.md` frontmatter is lines 1 to 22. The `missing-docs-python.md` `run` script is lines 13 to 59, which is 47 lines.

    The clause "so it shows the whole schema and no more" is dropped as well. It is not true either: `doctor.fix_hint` is a schema key the example does not hold. The sentence now names what the example does show, which is a `run` of one pipe. That is the reason this rule is the example, and it is the contrast the next sentence needs.

    Swept the whole file. `shortest`, `longest`, `smallest`, `largest`, `simplest`, `fewest` and `the only` over `builtin/validators/README.md` report one line, and that is the line corrected. No other unmeasured superlative stands in the file.

    **Finding 2 — `crates/swissarmyhammer-validators/src/validators/parser.rs`.** The fixture in `test_parse_tool_rule_full_block` is now the README example, word for word.

    - `name: missing-docs-python` becomes `name: complexity-python`, with the description the README shows.
    - `supersedes: missing-docs` becomes `supersedes: cognitive-complexity`. The scalar form is kept, so the test still covers a one-name `supersedes`.
    - The old `ruff check --select D1` pipe becomes the README's `ruff check --isolated --no-cache --config "lint.mccabe.max-complexity=15" --select C901` pipe.
    - `commands: ["uv tool install ruff", "pipx install ruff"]` becomes the pinned pair `ruff==0.14.5`.
    - The file name given to `parse_rule_plain` becomes `complexity-python.md`.

    The assertions follow the data. `tool.run.contains("ruff check --select D1")` becomes two assertions: the run holds `ruff check --isolated --no-cache`, and it holds `--select C901`.

    The doc comment needs no change. It says the block is "The README's tool-rule example shape", and that is true again.

    Swept both files. `select D1` over `crates/` reports the two lines named in the finding and no other. `tool-rule example` and `shortest shipped` over `crates/` and `doc/` report the one doc comment, which is correct now. `missing-docs-python` elsewhere in the tests names the real shipped rule, not the README example, so those references stand.

    ### RED to GREEN

    RED, proved on the assertion and not on a guess. I changed the fixture data alone and left the old assertions:

        thread '...test_parse_tool_rule_full_block' panicked at parser.rs:1667:9:
        assertion `left == right` failed
          left: ["cognitive-complexity"]
         right: ["missing-docs"]

    1 test run, 0 passed, 1 failed. Then I moved the three assertions onto the README example.

    GREEN — `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators`: 591 tests run, 591 passed, 0 skipped. `cargo nextest run -p mirdan`: 503 passed, 0 skipped. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-08-11T04:34:47.100884+00:00
- actor: claude-code
  id: 01kzqhm6vs83x81x23gst2xk9e
  text: |
    ### implement — changed
    - evidence: 2 files — `builtin/validators/README.md`, `crates/swissarmyhammer-validators/src/validators/parser.rs`. Both review findings corrected and checked. Commands: `cargo nextest run -p swissarmyhammer-validators` 591 run, 591 passed, 0 skipped; `cargo nextest run -p mirdan` 503 passed, 0 skipped; `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED proved on the `supersedes` assertion, GREEN after.
    - next: /review
  timestamp: 2026-08-11T04:34:53.433864+00:00
- actor: claude-code
  id: 01kzqjexfvjfp96zpyedvx5k4k
  text: |
    ### The eight open findings, each measured then corrected

    I measured each finding myself with ruff 0.14.5 before I changed anything. All eight hold.

    **`missing-docs-python.md:15` — zero arguments.** Reproduced. Probe tree of `top.py` and `deep/nested/other.py`, script run with no argument: 5 findings over those two files, exit 0. The finding says 4; my probe holds one more undocumented item, so the count is 5. The class of defect is the same. `ruff check` with no path falls back to a default target of `.` and walks the whole tree.

    Fix: `if [ "$#" -eq 0 ]; then exit 0; fi` at the head of the script.

    **`missing-docs-python.md:23` — no `trap`.** Reproduced. One run raised the count of directories under `TMPDIR` from 13196 to 13197.

    Fix: `trap 'rm -rf "$work"' EXIT` under the `mktemp`. Measured after: five runs over one file leave the count unchanged, and a run that exits 1 on an unparsable file leaves it unchanged too. The trap covers every exit.

    **`missing-docs-python.md:198` — the stated mechanism.** Reproduced. `--select ZZ999`: ruff writes 0 bytes to stdout and 93 bytes to stderr, the text `error: invalid value 'ZZ999' for '--select <RULE_CODE>'`. The `cat` of the stdout file writes nothing. The body now says so, and it says what the `cat` does stand for: a partial report ruff wrote before it stopped.

    **`missing-docs-python.md:194` — the configuration half.** Reproduced. Beside a `pyproject.toml` that holds `[[[ not toml`: with `--isolated` ruff exits 1 and judges the Python file; without `--isolated` ruff exits 2 and writes `Failed to parse ... pyproject.toml`. The bullet now claims the selector alone, and a paragraph under it states why the configuration status cannot reach this script.

    **`missing-docs-python.md:126` — `async`.** Reproduced. Over one documented module holding `async def test_async`, `async def helper` and `def test_plain`: the rule reports `D103` on `helper` alone. The prose now reads `def test...` or `async def test...`.

    **`missing-docs.md:33` — the carve-out note.** The note asserted one outcome for every language rule. It is wrong for a Python magic method. The note now states the contract instead: each language rule decides which carve-outs hold, a rule can KEEP one (Python keeps obvious-implementation, because `D105` is out of the selector), a rule can DROP one (the same rule reports a `@property` getter), and never cite a carve-out against a language-rule finding.

    I did not enumerate every language. Go and Dart also keep the obvious-implementation carve-out and drop the getter one, so a roster would go stale the same way the old sentence did. The note names the rule to read.

    **`README.md:108` and `:117` — the pipe.** The `run` guidance now states the trap: a pipeline takes the exit status of its LAST command, so a pipe that ends in `jq` exits 0 whatever the tool did, and the engine reads exit 0 as "the tool judged the code". It names the two other shapes of the same trap — a tool that exits 0 for a file it could not open, and a `files`-scope script given no file. The `doctor` bullet now says "everything the script needs" rather than "the pipe".

    I sweept the whole file for the same cause and found a third place: the stdout-contract paragraph told an author NOT to add `pipefail`, with no caveat. That paragraph now states that the same exit status hides a linter that broke, and that a pipe is safe only where the tool exits nonzero for findings alone.

    ### One count in the README went stale as a result

    The README states the length of this rule's script. The guard and the trap made it 51 lines, not 47. Corrected, and re-measured: the `run` block runs lines 13 to 63.

    ### The whole-file sweep of the rule

    One `mktemp` in the file, now trapped. One `"$@"` site beside the loop, both covered by the one guard at the head. Each acceptance test the body names exists in `tests/shipped.rs`.
  timestamp: 2026-08-11T04:49:28.571377+00:00
- actor: claude-code
  id: 01kzqjfhf4p5rm9f5y2cx4ye6f
  text: |
    ### RED to GREEN on the zero-argument defect, with the real tool

    I wrote the test first, against the UNCHANGED script.

    RED — `cargo nextest run -p swissarmyhammer-validators the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given`:

        assertion `left == right` failed: the script judges the files it is given
        and no other: given none, it reports none and exits 0, and the staged tree
        stays unread
          left: ["deep/nested/other.py:1", "deep/nested/other.py:1",
                 "deep/nested/other.py:2", "top.py:1", "top.py:1"]
         right: []

    1 test run, 0 passed, 1 failed. That is the defect: 5 findings over two files the script was never given, and an exit status of 0.

    GREEN after the guard: 5 tests run, 5 passed over the whole `the_shipped_python_missing_docs` filter.

    The test drives the SHIPPED script through `run_script_findings`, the one function both the engine and the doctor run a rule's script with, and it builds the argument list with `script_args(ToolScope::Files, &[])` — the same call the engine makes. So the run reads the argument list a `files`-scope rule with no matched file would really receive.

    ### One new shared shape, reusing `ShippedRun`

    The planner drops a `files`-scope rule that matched no file, so no plan can carry a zero-file run and none of the four existing shapes fits. `ShippedEmptyRun` holds a `ShippedRun` and the tree the script must not read, and `verify_shipped_run_reads_only_its_arguments` drives it. `shipped_run_script` reads the `run` out of the loader, so the run measures the shipped bytes.

    `ShippedRun`, `ShippedFailFixture`, `ShippedStagedPositions` and `ShippedBrokenRun` are reused as they stand. No near-copy.

    ### The sweep of the sibling rules, and the card it produced

    The finding names one file. I measured every shipped `files`-scope rule for the same two defects, with each script read out of its own rule and run with no argument in a probe tree of deeply nested files.

    **Eight siblings answer for files they were never given, each exiting 0**: `magic-numbers-swift` 34 findings, `magic-numbers-typescript` 34, `magic-numbers-python` 16, `complexity-python` 1, `complexity-swift` 1, `complexity-typescript` 1, `missing-docs-swift` 1, `missing-docs-typescript` 1. Six more hold no guard either; the probe tree simply does not carry the defect each reports.

    **Eleven siblings leak a temporary directory**: `complexity-swift`, `complexity-typescript`, `function-length-go`, `magic-numbers-dart`, `magic-numbers-go`, `magic-numbers-swift`, `magic-numbers-typescript`, `missing-docs-dart`, `missing-docs-go`, `missing-docs-swift`, `missing-docs-typescript`. Three rules already hold the trap and are the pattern: `complexity-rust`, `dead-code-rust`, `unused-dependencies-rust`.

    Filed as ^6585731 rather than widening this card.

    ### Commands

    `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` before each cargo run, so the compiled-in builtins hold the changed rule.

    - `cargo nextest run -p swissarmyhammer-validators` — 592 run, 592 passed, 0 skipped.
    - `cargo nextest run -p mirdan` — 503 run, 503 passed, 0 skipped.
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean.
  timestamp: 2026-08-11T04:49:49.028137+00:00
- actor: claude-code
  id: 01kzqjfsc7jrdvg3ppenafbjfh
  text: |
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/missing-docs-python.md`, `builtin/validators/code-hygiene/rules/missing-docs.md`, `builtin/validators/README.md`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`. 222 insertions, 15 deletions. All 8 open findings corrected and checked. Commands: `cargo nextest run -p swissarmyhammer-validators` 592 run, 592 passed, 0 skipped; `cargo nextest run -p mirdan` 503 passed, 0 skipped; `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED proved on the zero-argument defect (5 findings over 2 files never given, exit 0), GREEN after the guard.
    - next: /review
  timestamp: 2026-08-11T04:49:57.127063+00:00
- actor: claude-code
  id: 01kzqjzhr9v5wee3kprmzr36bc
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (46ca76b00), task mode
    - engine counts: 2 findings, 2 confirmed, 0 refuted, 8 attempted, 1 file skipped
    - dropped: the `dead_code_work` / `complexity_work` near-duplicate in `tests/shipped.rs`. The written rule drops a finding that asks to refactor test code which already existed. Origin `087d052b43`.
    - prior findings: all 10 items in the two earlier sections are `[x]`. Each one was re-measured with ruff 0.14.5 and the shipped script:
      - zero-argument guard: the script exits 0 and reports nothing. Without the guard ruff reads its own default target and reports 5 findings over `top.py` and `deep/nested/other.py`.
      - `trap ... EXIT`: five runs and one failing run each leave the count of entries under `TMPDIR` at 122253.
      - `--select ZZ999`: 0 bytes to stdout, 93 bytes to stderr. ruff makes the diagnostic.
      - `async def test_async` and `def test_plain` are dropped; `async def helper` reports `D103` at row 8.
      - script length is 51 lines, which is the count `README.md` states.
      - the four test names the rule body states each resolve in `tests/shipped.rs`.
    - audit check 1 (claims and names): pass.
    - audit check 2 (exemptions are a tool setting, an annotation or a structural fact): fail at `missing-docs.md:35`. See the finding.
    - audit check 3 (no stale sentence about a sibling rule): fail at `missing-docs.md:35`. Dart and Go agree with the new wording. Swift, Rust and TypeScript state nothing.
    - new findings: `builtin/validators/code-hygiene/rules/missing-docs.md:35`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1`
    - next: correct `missing-docs.md`, and split `tests/shipped.rs` below the prompt cap.
  timestamp: 2026-08-11T04:58:33.609575+00:00
- actor: claude-code
  id: 01kzqk0ebyfwjagkgfbdw1n8jc
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 2 files then 4 files; the zero-argument defect fixed (5 findings over 2 files never given, exit 0), a trap added, three claims corrected, three cross-references corrected; sibling sweep filed ^6585731
    - test: green — cargo nextest run -p swissarmyhammer-validators, 592 passed; RED proved for the zero-argument guard; clippy -D warnings clean
    - commit: 910d11fca then 46ca76b00 — 6 files
    - review: findings — builtin/validators/code-hygiene/rules/missing-docs.md:35, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1
  timestamp: 2026-08-11T04:59:02.910787+00:00
- actor: claude-code
  id: 01kzqkd2m4yf2hte20b0rddz7x
  text: |-
    ### Finding 1 — the two carve-outs measured against all SIX shipped rules

    I ran each rule's own `run` script, read out of the shipped rule file, over a probe that holds one obvious implementation and one simple getter and setter. I did not read a sibling rule's prose.

    | rule | obvious implementation | simple getter or setter |
    |---|---|---|
    | dart | KEPT — `@override toString()`, `operator ==` and `hashCode` each report nothing | DROPPED — `get name` (row 5) and `set name` (row 7) each report |
    | go | KEPT — `String()` and `Error()` each report nothing | DROPPED — `Name()` (row 9) and `SetName()` (row 13) each report |
    | python | KEPT — `__str__` and `__repr__` each report nothing | DROPPED — `@property def name` reports `D102` at row 12 |
    | rust | KEPT — `impl Display for Thing { fn fmt }` reports nothing | DROPPED — `pub fn name` (row 15) and `pub fn set_name` (row 19) each report |
    | swift | KEPT — `public var description` in a conforming type reports nothing | DROPPED — `public var name` (row 7) and a `get`/`set` pair (row 9) each report |
    | typescript | DROPPED — `toString()` (row 16) and `valueOf()` (row 20) each report | DROPPED — `get name` (row 8) and `set name` (row 12) each report |

    Two results hold over all six:

    - No shipped language rule keeps the simple getter carve-out. Every one reports an undocumented public getter.
    - A carve-out holds exactly where the rule's RUN stays silent about the item, and nowhere else.

    The Swift result has a cause worth recording: swiftlint's `missing_docs` default `excludes_inherited_types: true` silences EVERY member of a type that conforms to a protocol, not the conformance member alone. Measured: with `excludes_inherited_types: false` the same probe reports rows 19, 21 and 23; with the shipped default it reports none of them. So the Swift rule keeps the carve-out, and it keeps more beside it.

    Each of the six rules already states the annotation that exempts one item: `#[allow(missing_docs)]`, `// swiftlint:disable:next missing_docs`, `// eslint-disable-next-line jsdoc/require-jsdoc`, `//revive:disable-next-line:exported`, `# noqa: D103`, `// ignore_for_file: public_member_api_docs`. That is checkable in every rule body today.

    ### Finding 2 — the cap breach is THIS card's, not pre-existing

    `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` measured at each commit:

    - `b51f054ef~1` (before this card): 86135 bytes, 2047 lines
    - `b51f054ef`: 99663 bytes, 2367 lines
    - `910d11fca`: 99663 bytes, 2367 lines
    - `46ca76b00` (now): 105534 bytes, 2503 lines

    The engine reported 274957 rendered bytes against a 262144-byte cap, so the render is 2.605 times the raw bytes. At that ratio the raw cap is about 100600 bytes. The file was 86135 bytes before this card, which renders at about 224400 bytes and is under the cap. This card raised it to 105534 bytes and crossed the cap.

    So the written exception does NOT release this item. The file is over the cap because of this change. I will split it.
  timestamp: 2026-08-11T05:05:56.868979+00:00
- actor: claude-code
  id: 01kzqmrm81shmg51z92ge5p18w
  text: |-
    ### Finding 1 corrected — `missing-docs.md` now states the default, and needs no reading of a sibling rule

    The note asked the reader to read the language rule. Three of the six rules say nothing, so the reader had to judge. The note now states the DEFAULT, which holds for every rule and for a rule that states nothing:

    > A carve-out holds exactly where the tool stays silent, and nowhere else. So a language-rule finding is never released by a carve-out above: the tool reported the item, so the carve-out does not hold for that item. This answer needs no reading of the language rule, and it holds for a language rule that states nothing about the carve-outs.

    That answers the case the finding named. A reader who meets a Swift `missing_docs` finding on an undocumented `description` property now has a written answer: the tool reported it, so it is a finding.

    The measured result is stated as two claims a reader can check by running one rule, not as a per-language roster that goes stale:

    - "Simple getters/setters with self-explanatory names" holds for none of the six. Each reports an undocumented public getter.
    - "Obvious implementations" is not uniform. Most stay silent on it, and at least one reports an undocumented `toString()`. Never carry this carve-out over from another language.

    The closing line now names the exemption that DOES work: write the annotation the tool reads, on the item, in the code. Each of the six rules already states its own annotation, so that line is checkable today.

    I did not edit the Swift, Rust or TypeScript rule bodies. Those are ^302hw8c, ^xv57pf8 and ^739encr. The new note stays true whether or not they land, because it states a rule rather than a roster.

    ### Finding 2 corrected — the file is split, and the engine now reads every part

    The engine could not read `tests/shipped.rs`. It is split into one module per rule family, under `tests/shipped/`:

    | file | lines | bytes |
    |---|---|---|
    | `tests/shipped.rs` (the shared shapes) | 664 | 26835 |
    | `tests/shipped/missing_docs.rs` | 853 | 36609 |
    | `tests/shipped/complexity.rs` | 406 | 16311 |
    | `tests/shipped/magic_numbers.rs` | 391 | 17811 |
    | `tests/shipped/dead_code.rs` | 108 | 4566 |
    | `tests/shipped/unused_dependencies.rs` | 121 | 4931 |

    The parent keeps only what every test shares: `required_run`, `ShippedAssetKind`, `ShippedRun`, `ShippedFailFixture`, `ShippedStagedPositions`, `ShippedBrokenRun`, `ShippedEmptyRun`, the five `verify_*` drivers, and the Swift package root. Each family module opens with `use super::*` and holds its roster test first, then one test for each language.

    `execute_emits_no_planned_event_when_there_are_no_runs` moved to `tests/execute.rs`. It measures `execute_tool_runs` on an empty plan, not a shipped rule, and the other `execute_*` tests live there. It gained the doc comment the file's other tests carry.

    No test was lost and no line was lost. Measured: 29 test functions before, 29 after, and a sorted line diff of the old file against the six new files reports only the moved test and the new `//!`, `mod` and `use` lines.

    ### Proved with the real engine, not by arithmetic

    `review file` over `tests/shipped*.rs` with the `duplication` validator: `skipped: 0`, `skipped_files: []`. All six files are read. Before the split the same validator skipped the one file.

    That run also reports 11 duplication items on the shared-shapes module, which the engine could never reach before. I traced each with `git log -S`. Eight name shapes other cards wrote: `ShippedAssetKind` (e79853df9, ^h7garpc), `ShippedRun` and `ShippedStagedFile` and `verify_shipped_staged_positions_report` (a060e5391 and 467cfe2c9, ^s2056e1), `ShippedFailFixture` (5a20b44b1, ^2syfvyt), `copy_shipped_fixture` (7cf57eeda, ^s2ftjys), `swift_package_root` and its test (615593ecb, ^a4ebnw3), `verify_shipped_tool_rules_pass_fixtures` (978a4eef3, ^teemmch).

    Two name shapes this card wrote, and I measured what each match points at:

    - `verify_shipped_run_breaks`, said to be 0.94 similar to `swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs:697`. That line is `#[cfg(test)] mod tests {`, a module header and not a helper. The advice "call the existing version in the diagnostics module" cannot be followed either: `swissarmyhammer-tools` depends on `swissarmyhammer-validators`, so the reverse edge would invert the dependency.
    - `ShippedEmptyRun`, said to be 0.88 similar to `shipped/missing_docs.rs:825`. That line is `const PYTHON_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {`, the struct's only instance. A struct and its instance name the same fields.

    I did not act on these 11. They came from my own cap check, not from the review step, and the review step owns what lands on the card.
  timestamp: 2026-08-11T05:29:43.937452+00:00
- actor: claude-code
  id: 01kzqmrymgy3xhj065jv564e97
  text: |
    ### implement — changed
    - evidence: 3 files changed, 5 files added. `builtin/validators/code-hygiene/rules/missing-docs.md`; `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` split into `tests/shipped/{missing_docs,complexity,magic_numbers,dead_code,unused_dependencies}.rs`; one test moved to `crates/swissarmyhammer-validators/src/review/tool_rules/tests/execute.rs`. Both open findings corrected and checked. Commands, each after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`: `cargo nextest run -p swissarmyhammer-validators` 592 run, 592 passed, 0 skipped; `cargo nextest run -p mirdan` 503 run, 503 passed, 0 skipped; `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. Cap proved with the real engine: `review file` over `tests/shipped*.rs` with the `duplication` validator reports `skipped: 0`, `skipped_files: []`. Carve-outs measured by running each of the six shipped rules' own `run` script over a probe.
    - next: /review
  timestamp: 2026-08-11T05:29:54.576376+00:00
- actor: claude-code
  id: 01kzqpnbk8tn1611vqgteksvhh
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (b24837b16), task mode, plus the three checks the diff-scoped engine cannot make.
    - engine counts: 1 finding, 1 confirmed, 22 refuted, 16 attempted, 0 failed, **0 skipped**, `skipped_files: []`.

    **The cap breach is corrected, and I proved it with the engine.** The `HEAD~1..HEAD` run reports `skipped: 0`. A second run, `review file` over `tests/shipped*.rs` with the `duplication` validator alone, also reports `skipped: 0` and `skipped_files: []`, and it made a finding INSIDE `tests/shipped/dead_code.rs`. So the validator now reads every part. Sizes: `shipped.rs` 26835 bytes, `shipped/missing_docs.rs` 36609, `shipped/complexity.rs` 16311, `shipped/magic_numbers.rs` 17811, `shipped/unused_dependencies.rs` 4931, `shipped/dead_code.rs` 4566. The largest is 36609 bytes, which agrees with the commit message.

    **The one engine finding is dropped, and I traced it myself.** The finding names `dead_code_work` at `tests/shipped/dead_code.rs:43` as a near-duplicate of `complexity_work` at `tests/shipped/complexity.rs:73`. I did not trust the earlier trace.

    - `git log -S 'fn dead_code_work' -- crates/swissarmyhammer-validators/` gives three commits. The earliest is `af211dd8b` (^teemmch), which is before this card.
    - `git log -S 'fn complexity_work' -- crates/swissarmyhammer-validators/` gives three commits. The earliest is `bf5b5fc1e` (^3dfhnxg), which is before this card.
    - Both other commits are the two split commits, which only moved the text.
    - I extracted each function body from `b24837b16~1:tests/shipped.rs` and diffed it against the body in the new file. Both are IDENTICAL, byte for byte. This card moved them and changed nothing.

    The written exception releases a finding that asks to change test code which was there before. The finding is dropped.

    I could not reproduce the 11 duplication items the implementer reported from the same scope. My `review file` run over `tests/shipped*.rs` with the `duplication` validator gives 1 finding, not 11. The 1 it gives is the item above.

    **Check 2 — the split keeps every test. PASS.** Measured by counting `#[test]` and `#[tokio::test]`.

    - Before, at `b24837b16~1`: `tests/shipped.rs` 29, `tests/execute.rs` 9. Total 38.
    - After: `tests/shipped.rs` 1, `shipped/complexity.rs` 4, `shipped/magic_numbers.rs` 6, `shipped/missing_docs.rs` 13, `shipped/dead_code.rs` 2, `shipped/unused_dependencies.rs` 2 — 28 in the tree. `tests/execute.rs` 10. Total 38.

    `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators`: **592 tests run, 592 passed, 0 skipped**, in 50.131s.

    **Check 3 — no sentence about a sibling rule is stale. PASS.**

    - Each of the six language rules DOES state the annotation that exempts one item, so the closing line of `missing-docs.md` is checkable today: `#[allow(missing_docs)]` (rust), `// swiftlint:disable:next missing_docs` (swift, at the last line of the file), `// eslint-disable-next-line jsdoc/require-jsdoc` (typescript), `// ignore: public_member_api_docs` (dart), and a "How to exempt one item" section in go and python.
    - The Python claim holds. The selector in the shipped script is `D100,D101,D102,D103,D104,D106,D107`. `D105` is out, so the obvious-implementation carve-out is kept; `D102` is in, so the `@property` getter reports.
    - `missing-docs-go.md:41` quotes "These exemptions yield to stricter language-specific..." and that sentence still stands at `missing-docs.md:33`. Not stale.

    **Check 1 — the new default gives the same answer as the shipped tool. FAIL for Swift.** I extracted the `tool.run` script out of each of the six rule files and ran each one over a probe holding one undocumented public getter and one obvious implementation. All six tools are installed. Every run exited 0.

    | rule | undocumented public getter | obvious implementation |
    |---|---|---|
    | rust (clippy 0.1.97) | REPORTED, row 15 | silent |
    | go (revive 1.15.0) | REPORTED, row 15 | silent |
    | python (ruff 0.14.5) | REPORTED, `D102` row 12 | silent |
    | dart (SDK 3.11.0) | REPORTED, row 8 | silent |
    | swift (swiftlint 0.65.0) | **SILENT** | silent |
    | typescript (eslint 10.8.0) | REPORTED, row 14 | REPORTED, row 18 `toString()` |

    The second bullet of the note is true: five stay silent on the obvious implementation, and TypeScript reports `toString()`.

    The first bullet is not true. `missing-docs.md:57` says "Each of the six reports an undocumented public getter". Swift does not, when the type declares a protocol conformance. I proved the cause with an A/B that differs in one token, using the shipped script: `public struct Conforming: CustomStringConvertible` reports 0 findings and exits 0; the same body as `public struct Plain` reports row 8 and row 10 and exits 0. The `missing_docs` default `excludes_inherited_types: true` makes swiftlint pass over every member of a conforming type.

    - evidence: 1 open finding — `builtin/validators/code-hygiene/rules/missing-docs.md:57`
    - next: correct the getter claim in `missing-docs.md`, or set `excludes_inherited_types: false` in the Swift rule config so the claim becomes true. Then review again.
  timestamp: 2026-08-11T06:02:53.928439+00:00
- actor: claude-code
  id: 01kzqpp5rhva934h891hkstcf7
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 8 files; missing-docs.md states a default rather than a roster; tests/shipped.rs split into one module per rule family, below the prompt cap
    - test: green — cargo nextest run -p swissarmyhammer-validators, 592 passed; the split kept 38 test functions; clippy -D warnings clean
    - commit: b24837b16 — 8 files
    - review: findings — builtin/validators/code-hygiene/rules/missing-docs.md:57 ("each of the six reports an undocumented public getter" is false for Swift, because swiftlint excludes_inherited_types silences every member of a conforming type)
  timestamp: 2026-08-11T06:03:20.721921+00:00
- actor: claude-code
  id: 01kzqq3xhybngnkrph29srrkkr
  text: |
    ### The finding reproduced, then the whole file swept

    **The A/B, run again by me.** I read the `run` script out of `builtin/validators/code-hygiene/rules/missing-docs-swift.md` and ran it over two files that differ in one token. swiftlint 0.65.0.

    - `public struct Conforming: CustomStringConvertible`, with an undocumented `public var name` at row 8 and an undocumented `public var description` at row 10: 0 findings, exit 0.
    - The same body as `public struct Plain`: rows 8 and 10 each report `public declarations should be documented`, exit 0.

    The finding holds. The `missing_docs` default `excludes_inherited_types: true` makes swiftlint pass over EVERY member of a conforming type, not the conformance member alone.

    **Every other counting claim, measured.** I extracted each of the six rules' own `run` script from its rule file and ran each one over a probe holding an undocumented public getter, an undocumented public setter and an obvious implementation. All six tools are installed. Every run exited 0.

    | rule | undocumented public getter | obvious implementation |
    |---|---|---|
    | dart (SDK 3.11.0) | REPORTED, row 8 | silent (`@override toString()`) |
    | go (revive 1.15.0) | REPORTED, row 14 | silent (`String()`) |
    | python (ruff 0.14.5) | REPORTED, `D102` row 12 | silent (`__str__`, `__repr__`) |
    | rust (clippy) | REPORTED, row 15 | silent (`impl Display fn fmt`) |
    | swift (swiftlint 0.65.0) | SILENT in a conforming type; REPORTED row 8 with no conformance | silent in a conforming type |
    | typescript (eslint 10.8.0) | REPORTED, row 10 | REPORTED, row 18 `toString()` and row 22 `valueOf()` |

    Three other claims in the file were checked and each holds:

    - "each of the six shipped language rules — Dart, Go, Python, Rust, Swift and TypeScript". Six `missing-docs-<lang>.md` files ship. Correct.
    - The Python claims. `D105` is out of the selector, so `__str__` and `__repr__` stay silent; `D102` is in, so the `@property` getter reports at row 12. Correct.
    - "Each language rule states that annotation." All six state one: `// ignore: public_member_api_docs`, `//revive:disable-next-line:exported`, `# noqa: D103`, `#[allow(missing_docs)]`, `// swiftlint:disable:next missing_docs`, `// eslint-disable-next-line jsdoc/require-jsdoc`. Correct.

    ### The correction

    Only the two summary bullets changed. The DEFAULT above them stays word for word: a carve-out holds exactly where the tool stays silent, so a language-rule finding is never released by a carve-out. That default is true, it needs no reading of a sibling rule, and it removes the reader's judgment.

    The getter bullet now states the measured count of five, names the five rules, and states the condition that limits the Swift answer with its cause. The obvious-implementation bullet now states the measured count of four, names the four rules, names the two TypeScript items, and states the Swift condition. The old bullet said "Most of the six" and "at least one", which state no measurement.

    I did not edit `missing-docs-swift.md`. That is ^xv57pf8.

    ### Commands

    `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` first, so the compiled-in builtins hold the changed rule.

    - `cargo nextest run -p swissarmyhammer-validators` — 592 run, 592 passed, 0 skipped.
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean.
  timestamp: 2026-08-11T06:10:51.070022+00:00
- actor: claude-code
  id: 01kzqq450df4qm56w59yxwjh4g
  text: |
    ### implement — changed
    - evidence: 1 file — `builtin/validators/code-hygiene/rules/missing-docs.md`, 13 insertions, 5 deletions. The one open finding corrected, and every other counting claim in the file measured. Commands, after `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`: `cargo nextest run -p swissarmyhammer-validators` 592 run, 592 passed, 0 skipped; `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. The Swift A/B reproduced: `Conforming` 0 findings exit 0, `Plain` rows 8 and 10 exit 0.
    - next: /review
  timestamp: 2026-08-11T06:10:58.701227+00:00
- actor: claude-code
  id: 01kzqqgqa7tw9pygt266xfs22n
  text: |
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (db2432125), one file, 13 insertions and 5 deletions.
    - engine: `review sha` and `review file` both answer "Nothing in scope to review". The engine does not read Markdown, so it gives no coverage for this change. The claims were measured by hand against the real tools.
    - prior finding (2026-08-11 05:55): CLOSED. The false sentences are gone. The new text states measured counts, names the rules, and states the Swift condition with its cause.
    - measured all six shipped rules with their own `run` scripts. Getter probe and obvious-implementation probe:
      - Dart (dart 3.11.0): getter row 8 reported; `@override toString()` silent.
      - Go (revive 1.15.0): `Widget.Name` row 7 reported; `Widget.String` silent.
      - Python (ruff 0.14.5): `@property name` row 12 reported as D102; `__str__` silent, because D105 is out of the selector `D100,D101,D102,D103,D104,D106,D107`.
      - Rust (cargo 1.97.1 clippy): `pub fn name` row 9 reported; the `Display::fmt` impl item silent.
      - Swift (swiftlint 0.65.0): `Plain` reports rows 5 and 6; `Conforming: CustomStringConvertible` reports 0.
      - TypeScript (eslint 10.8.0): `get name` row 6, `toString()` row 7, and `valueOf()` row 8 all reported.
    - both bullet counts are correct: "five of the six" for the getter carve-out, "four of the six" for the obvious implementation.
    - 2 new findings. Both are on the reach and the wording of the new Swift text, not on the counts.
    - evidence: builtin/validators/code-hygiene/rules/missing-docs.md:60, builtin/validators/code-hygiene/rules/missing-docs.md:62
    - next: state the measured reach at line 60, and replace the unmeasured frequency at line 62.
  timestamp: 2026-08-11T06:17:50.663401+00:00
- actor: claude-code
  id: 01kzqqhtm3evtk5p1747npbchq
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — 1 file; the false "each of the six" sentence is replaced with measured counts and the Swift condition
    - test: green — cargo nextest run -p swissarmyhammer-validators, 592 passed; clippy -D warnings clean
    - commit: db2432125 — 1 file
    - review: findings — builtin/validators/code-hygiene/rules/missing-docs.md:60 and :62, both on the new Swift paragraph

    Note for the next iteration: every iteration of this card has produced a new finding on the SAME paragraph of the shared missing-docs.md note. The cause is a sentence that states more than the measurement supports. The next iteration must state only what was measured, with no frequency word, or the card is stuck.
  timestamp: 2026-08-11T06:18:26.819497+00:00
- actor: claude-code
  id: 01kzqqthkkggq4whaemrgbqej1
  text: |
    ### Measured the `:60` probe again before writing

    Reproduced the probe of the 06:16 round with swiftlint 0.65.0, with the shipped `missing-docs-swift` run script, read out of `builtin/validators/code-hygiene/rules/missing-docs-swift.md` and run with no change.

    Probe A, `Wide.swift`: `public struct Wide: Equatable` row 1, `public var name` row 2, `public func compute()` row 3, nested `public struct Inner` row 4, `public var v` row 5. No doc comment on any of the five.

    - shipped default: 0 findings, exit 0.
    - `excludes_inherited_types: false`: rows 1, 2, 3, 4 and 5.

    Probe B, `Plain.swift`: the same five rows in `public struct Plain`, which declares no conformance.

    - shipped default: rows 1, 2, 3, 4 and 5.

    Probe B is the control the earlier rounds did not hold at this shape. It shows the conformance is the cause, and it shows the reach: the silence covers the type declaration, each member, the nested type and each member of the nested type.

    ### What changed

    One paragraph of `builtin/validators/code-hygiene/rules/missing-docs.md` — the "Simple getters/setters" bullet. Three defects were removed together, because all three came from one cause: a sentence that stated more than the measurement supports.

    - "pass over EVERY member" replaced with the measured reach: the type declaration, each member, a nested type and each member of the nested type.
    - "so Swift stays silent for most getters" replaced with the measured statement: Swift stays silent for every getter in a type that declares a conformance.
    - "A conformance such as `Equatable`, `Codable`, `Sendable` or `Identifiable` is a usual Swift shape" was removed. It is a frequency claim, and only `Equatable` was measured. It was not named in a finding, but it holds the same defect as the two that were, and this rule set forbids an unmeasured frequency.

    The bullet now names swiftlint 0.65.0, the script it ran, the two probe files, and the counts each run gave.

    `missing-docs-swift.md` was NOT edited — that is card ^xv57pf8.

    ### Verification

    `rg` over the tree for `most getters`, `EVERY member` and `usual Swift shape`: 0 matches.
    `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` before each cargo command, so the compiled-in builtins hold the new rule body.

    ### implement — changed
    - evidence: 1 file — builtin/validators/code-hygiene/rules/missing-docs.md; `cargo nextest run -p swissarmyhammer-validators` 592 tests run, 592 passed, 0 failed, 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all` clean; `git status --porcelain` shows one source file changed
    - next: /review
  timestamp: 2026-08-11T06:23:12.499995+00:00
- actor: claude-code
  id: 01kzqr2qtva5pz7atk848tkswe
  text: |
    ### review — findings
    - evidence: 1 finding — builtin/validators/code-hygiene/rules/missing-docs.md:58 (same cause at :71 and :76). `review sha HEAD~1..HEAD` reports "Nothing in scope to review", because the engine does not read Markdown. The claims were measured by hand with swiftlint 0.65.0 and the shipped `missing-docs-swift` run script.
    - verified true: `public struct Wide: Equatable` with a property, a method, a nested type and a nested property reports 0 findings and exits 0; the same five rows report with `excludes_inherited_types: false`; the `public struct Plain` control with no conformance reports all five rows under the shipped default. No frequency word stands in the file.
    - defect: the note keys the silence on a protocol conformance. Swiftlint keys it on an inherited type. `public class Sub: Base` (superclass) reports 0 findings and exits 0; rows 6, 7 and 8 report with `excludes_inherited_types: false`. `public enum Raw: String` (raw value) reports 0 findings and exits 0; rows 1, 2, 3 and 4 report with `excludes_inherited_types: false`. Neither type declares a protocol conformance.
    - next: correct the three lines together. State that the tool passes over the type declaration, each member and each nested item when the type declares an inherited type, and that an inherited type is a protocol conformance, a superclass or a raw-value type.
  timestamp: 2026-08-11T06:27:41.019971+00:00
- actor: claude-code
  id: 01kzqr3se55z5ey761pj9030mq
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — 1 file; every frequency word is removed from the paragraph, and the reach of the swiftlint silence is stated from a probe with a control
    - test: green — cargo nextest run -p swissarmyhammer-validators, 592 passed; clippy -D warnings clean
    - commit: 61da55169 — 1 file
    - review: findings — builtin/validators/code-hygiene/rules/missing-docs.md:58, with the same cause at :71 and :76

    The finding is correct and it is narrow: the setting is `excludes_inherited_types`, and an inherited type is not only a protocol conformance. A superclass and a raw-value type are inherited types too, each measured silent. The word "conformance" must become "inherited type" in three places. The findings on this paragraph get narrower each round (a paragraph, then two sentences, now one word), so the work converges. This is the last iteration: if the next review finds another claim in this paragraph, the card is stuck and a person decides.
  timestamp: 2026-08-11T06:28:15.429831+00:00
- actor: claude-code
  id: 01kzqrdvy992ae2y2yg3tvqcyg
  text: |
    Reproduced both probes before the edit, plus the two probes of the earlier rounds. swiftlint 0.65.0, jq, with the shipped `missing-docs-swift` run script copied word for word out of `builtin/validators/code-hygiene/rules/missing-docs-swift.md`. Each file was run twice: once with the shipped config (`only_rules: [missing_docs]`), once with `missing_docs.excludes_inherited_types: false` added.

    | probe | inherited type | shipped | `excludes_inherited_types: false` |
    |---|---|---|---|
    | `public class Sub: Base` r6, `public var name` r7, `public func compute()` r8 | superclass | 0 findings, exit 0 | rows 6, 7, 8 |
    | `public enum Raw: String` r1, case r2, `public var name` r3, `public func compute()` r4 | raw-value type | 0 findings, exit 0 | rows 1, 2, 3, 4 |
    | `public struct Wide: Equatable` r1, `public var name` r2, `public func compute()` r3, nested `public struct Inner` r4, `public var v` r5 | protocol conformance | 0 findings, exit 0 | rows 1, 2, 3, 4, 5 |
    | `public struct Plain` r1, same four rows below it | none | rows 1, 2, 3, 4, 5 | rows 1, 2, 3, 4, 5 |

    The three shapes measured are a protocol conformance, a superclass and a raw-value type. No other inherited type was measured, so the rule text does not claim the list is complete.

    One substitution, made in three places in `builtin/validators/code-hygiene/rules/missing-docs.md`: the condition is that the type declares an INHERITED TYPE, not that it declares a protocol conformance. The two new probes and the fourth control probe are written into the bullet, each with the count seen. No frequency word is in the paragraph.

    No test asserts on this text. A grep for `protocol conformance`, `conforming type` and `excludes_inherited_types` over `*.rs` and `*.md` matched only the `acp-conformance` crate and the stale generated `target/` copy.
  timestamp: 2026-08-11T06:33:45.673607+00:00
- actor: claude-code
  id: 01kzqre1exw76s3cg64sm06j82
  text: |
    ### implement — changed
    - evidence: 1 file — `builtin/validators/code-hygiene/rules/missing-docs.md`. `touch crates/swissarmyhammer-validators/src/builtin/mod.rs` then `cargo nextest run -p swissarmyhammer-validators` — 592 tests run, 592 passed, 0 skipped, 0 failed. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, 0 warnings.
    - next: /review
  timestamp: 2026-08-11T06:33:51.325578+00:00
- actor: claude-code
  id: 01kzqrmgc6ecfqb4bprjnn4gwj
  text: |-
    ### review — clean
    - evidence: 0 new findings on d7cf15c89 (1 file changed: builtin/validators/code-hygiene/rules/missing-docs.md). All 16 prior findings are checked. The commit changes Markdown only, so the review is a hand check of the claims in the changed paragraph.
    - probe 1 `public struct Wide: Equatable` with a member, a function, a nested `public struct Inner` and a nested member: the shipped run script reports 0 findings and exits 0. With `excludes_inherited_types: false` it reports rows 1, 2, 3, 4 and 5. This agrees with the text.
    - probe 2 documented `open class Base` with `public class Sub: Base` at row 6, a member at row 7 and a function at row 8: 0 findings, exit 0. With `excludes_inherited_types: false` it reports rows 6, 7 and 8. This agrees with the text.
    - probe 3 `public enum Raw: String` with a case, a member and a function: 0 findings, exit 0. With `excludes_inherited_types: false` it reports rows 1, 2, 3 and 4. This agrees with the text.
    - probe 4 control `public struct Plain` with no inherited type: the shipped default reports rows 1, 2, 3, 4 and 5. This agrees with the text.
    - `swiftlint rules missing_docs` shows the default `excludes_inherited_types: true`, which is the cause the text names. swiftlint 0.65.0.
    - A fifth probe holds `public struct Conf: Equatable` and `public struct NoInherit` in one file. The default reports rows 4 and 5 only. Thus the tool passes over the type that declares the inherited type, its members and its nested items, and no more. The reach in the text is not wider and not narrower than the measurement.
    - The condition "reports it only when the type declares no inherited type" is a necessary condition. No probe reports an item in a type that declares an inherited type, so no measurement makes the condition false.
    - No frequency word stands in the paragraph. Each name resolves: swiftlint 0.65.0, the `missing_docs` rule, the `excludes_inherited_types` key (swiftlint gives no unknown-key warning), and the shipped `missing-docs-swift` run script.
    - next: card moved to done.
  timestamp: 2026-08-11T06:37:23.206099+00:00
- actor: claude-code
  id: 01kzqrna554z775q588jt5e7vn
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — 1 file; the condition reads "inherited type", not "protocol conformance", in three places; four probes with a control, each with its count
    - test: green — cargo nextest run -p swissarmyhammer-validators, 592 passed; clippy -D warnings clean
    - commit: d7cf15c89 — 1 file
    - review: clean — four probes reproduced exactly with swiftlint 0.65.0, plus a fifth that proves the pass-over is not file-wide; 16 of 16 findings verified closed; card moved to done
  timestamp: 2026-08-11T06:37:49.605344+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe280
title: missing-docs-python selects the whole D1 group, which flags tests, __str__ and trivial getters
---
`builtin/validators/code-hygiene/rules/missing-docs-python.md` runs `ruff check --isolated --select D1` and declares `supersedes: [missing-docs]`.

Three carve-outs of `missing-docs.md` are dropped, and all three were measured.

- "Functions explicitly marked as tests by attribute or framework convention". On `--stdin-filename tests/test_foo.py`, ruff reports `D100 Missing docstring in public module` and `D103 Missing docstring in public function` on `def test_foo():`. `--isolated` also discards the `per-file-ignores` entry (`"tests/*" = ["D"]`) that most Python projects hold for exactly this, so the exemption is removed twice.
- "Obvious implementations (Display, Debug, ToString, etc.)". `D105 Missing docstring in magic method` fires on `def __str__(self)`. D105 is the direct Python analogue of the carve-out, and it sits inside the selected `D1` group. Selecting `D1` without `D105` reproduces the carve-out.
- "Simple getters/setters with self-explanatory names". `D102` fires on a two-line `@property def name(self): return self._n`.

"Generated code" is dropped as well; ruff has no generated-file heuristic.

The prompt rule's closing note yields the obvious-implementation and getter carve-outs only to the Swift and Rust rules, so Python does not hold that dispensation. These are contradictions, not deliberate strictness.

The private-item carve-out IS reproduced: `D1` targets public items only.

Decide the selector, and how the test carve-out is stated.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity

## Review Findings (2026-08-10 22:57)

- [x] `builtin/validators/README.md:71` — the sentence "This is the shortest shipped tool rule" is not correct, and it gives no measured count. Measured over each shipped rule that holds a `tool:` block: `complexity-python.md` has 59 lines, `missing-docs-rust.md` has 43 lines, and `missing-docs-swift.md` has 47 lines. Two shipped tool rules are shorter. Measured by frontmatter only, `complexity-python.md` has 22 lines, which is equal to `missing-docs-swift.md`, `function-length-python.md`, `magic-numbers-python.md` and `unused-code-go.md`. The rule is not the shortest by either measure. State the count you measured, or write a claim that is true.
- [x] `crates/swissarmyhammer-validators/src/validators/parser.rs:1655` — the test data holds the old Python selector `ruff check --select D1`, under the name `missing-docs-python`. The assertion at `crates/swissarmyhammer-validators/src/validators/parser.rs:1675` holds the same text. The doc comment at `crates/swissarmyhammer-validators/src/validators/parser.rs:1639` says the block is the shape of "The README's tool-rule example", but this change made the README example `complexity-python`. The test data and the README example do not agree. Make the test data agree with the README example again.

## Review Findings (2026-08-11 04:31)

- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:15` — with ZERO arguments the `for file in "$@"` loop is a no-op, and `ruff` then falls back to its own default target `.`. Measured: it reported 4 findings across `top.py` and `deep/nested/other.py`, files never passed to it, and exited 0. That is the same silent-wrong-answer class this card set out to remove. The run must report nothing and exit 0 when it is given no file.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:23` — `work="$(mktemp -d)"` has no `trap`, so every run leaks a temporary directory.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:198` — the stated mechanism is false. On `--select ZZ999` ruff writes 0 bytes to stdout, so `cat "$work/ruff.json" >&2` emits nothing; the diagnostic reaches stderr because ruff writes it there directly. The outcome the body claims is right, the reason is not.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:194` — only the selector half of the claim is measured, and the configuration half cannot occur at all, because the script passes `--isolated`.
- [x] `builtin/validators/code-hygiene/rules/missing-docs-python.md:126` — the prose says the filter drops `def test...`, but the script also matches `async def test`. Measured: `async def test_async` is dropped. State what the script does.
- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:33` — the note says the "obvious implementation" and "simple getter" carve-outs "do not apply" wholesale, but Python now deliberately KEEPS the obvious-implementation carve-out by leaving `D105` out of the selector. A reader who applies that note to a Python magic method gets the wrong answer.
- [x] `builtin/validators/README.md:108` — the guidance tells a rule author to write "the pipe is the mapping" with no caveat about exit status. That is the exact construct whose exit-code swallowing caused the three silent-zero defects this card fixed. The guidance must state the trap.
- [x] `builtin/validators/README.md:117` — the same. The guidance must state the trap.

## Review Findings (2026-08-10 23:51)

> WARNING: 1 file was not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — 274957 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:35` — the note tells the reader "the language rule states it", and line 45 tells the reader "Read the language rule to learn which carve-outs it keeps". Three of the six language rules state nothing. Measured with a search for `carve`, `obvious`, `getter`, `Display`, `Debug` and `toString` in each rule body: `missing-docs-dart.md` gives 11 matches and `missing-docs-go.md` gives 12, and each states its decision; `missing-docs-swift.md`, `missing-docs-rust.md` and `missing-docs-typescript.md` each give 0 matches. This change also removed the one sentence that gave the answer for Swift and Rust ("that rule wins and the `obvious implementation` / `simple getter` carve-outs above do not apply"). A reader who meets a Swift `missing_docs` finding on an undocumented `description` property now has no written answer, so the reader must judge, and this rule set forbids a reader's judgment as an exemption. State the decision in each of the three rules, or write in this note the default that holds for a language rule which states nothing about the carve-outs.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1` — this file is over the review prompt cap — 274957 rendered bytes against the 262144-byte per-file cap — so the `duplication` validator could not read it. This change added 136 lines to the file. Split the file into smaller modules that each stay below the cap.

## Review Findings (2026-08-11 05:55)

- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:57` — the sentence "Each of the six reports an undocumented public getter" is not true for Swift, and the bullet above it at `builtin/validators/code-hygiene/rules/missing-docs.md:56` that says the getter carve-out "holds for none of them" is not true for the same reason. The shipped `missing-docs-swift` script reports NOTHING for an undocumented `public var` getter when the type declares a protocol conformance. Measured with swiftlint 0.65.0, running the shipped `run` script read out of `builtin/validators/code-hygiene/rules/missing-docs-swift.md`, over two files that differ in one token. `public struct Conforming: CustomStringConvertible`, with a documented initializer and an undocumented `public var name` at row 8 and an undocumented `public var description` at row 10: 0 findings, exit 0. The same body as `public struct Plain`, with no conformance: `{"line":8,"message":"public declarations should be documented"}` and `{"line":10,...}`, exit 0. The cause is the `missing_docs` rule default `excludes_inherited_types: true`, which makes swiftlint pass over EVERY member of a type that declares a conformance, not the conformance member alone. A protocol conformance is a usual shape in Swift (`Equatable`, `Codable`, `Sendable`, `Identifiable`), so the exception is not rare. The implementer's own comment on this card records the same measurement ("with `excludes_inherited_types: false` the same probe reports rows 19, 21 and 23; with the shipped default it reports none of them"), so the text and the measurement behind it do not agree. This is the objectivity defect this card exists to remove: a reader who trusts line 57 raises a Swift finding the tool never raises. State what you measured, with the condition that limits it, or set `excludes_inherited_types: false` in the Swift rule's config so the claim becomes true.

## Review Findings (2026-08-11 06:16)

- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:60` — the words "pass over EVERY member of a type that declares a conformance" give a smaller reach than the tool has. Swiftlint also passes over the type declaration itself, over a nested type, and over the members of that nested type. Measured with swiftlint 0.65.0. The shipped `run` script was read out of `builtin/validators/code-hygiene/rules/missing-docs-swift.md`. The probe file holds an undocumented `public struct Wide: Equatable` at row 1, an undocumented `public var name` at row 2, an undocumented `public func compute()` at row 3, an undocumented nested `public struct Inner` at row 4, and an undocumented `public var v` at row 5. With the shipped default the tool reports 0 findings and exits 0. With `excludes_inherited_types: false` the same tool reports rows 1, 2, 3, 4 and 5. A reader who has an undocumented `public struct Wide: Equatable` reads this line, sees that the tool passes over members only, and raises a Swift finding on the type declaration. The tool never raises that finding. This is the same defect that the round of 2026-08-11 05:55 closed. State the reach you measured: the tool passes over the type declaration, every member, and every nested item below it.
- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:62` — the words "so Swift stays silent for most getters" give a frequency for Swift code. No measurement supports that frequency. The two bullets that hold this line give measured counts ("five of the six", "four of the six"), so this line does not agree with the method of the file. `swiftlint rules missing_docs` gives the default, and the probe gives the condition. Neither gives a proportion of the getters in Swift code. The round of 2026-08-10 22:57 removed the words "This is the shortest shipped tool rule" for the same cause: a count that was not measured. Write the measured statement instead: Swift stays silent for every getter in a type that declares a conformance.

## Review Findings (2026-08-11 01:26)

- [x] `builtin/validators/code-hygiene/rules/missing-docs.md:58` — the words "Swift reports it only when the type declares no protocol conformance" are not true. Swiftlint stays silent when the type declares an INHERITED TYPE. A protocol conformance is one inherited type. A superclass is an inherited type. A raw-value type is an inherited type. The note names only the protocol conformance, so it states a smaller condition than the tool applies, and it gives the wrong answer for the other two. Measured with swiftlint 0.65.0, with the shipped `missing-docs-swift` run script. The first probe file holds a documented `open class Base`, an undocumented `public class Sub: Base` at row 6, an undocumented `public var name` at row 7 and an undocumented `public func compute()` at row 8. `Sub` declares no protocol conformance. The run reports 0 findings and exits 0. With `excludes_inherited_types: false` the same run reports rows 6, 7 and 8. The second probe file holds an undocumented `public enum Raw: String` at row 1, a case at row 2, an undocumented `public var name` at row 3 and an undocumented `public func compute()` at row 4. `Raw` declares no protocol conformance. The run reports 0 findings and exits 0. With `excludes_inherited_types: false` the same run reports rows 1, 2, 3 and 4. A reader who has an undocumented getter in `public class Sub: Base` reads this line, sees no protocol conformance, and raises a Swift finding. The tool never raises that finding. This is the same defect that the rounds of 2026-08-11 05:55 and 2026-08-11 06:16 closed. The same cause stands at two more lines of the same file, and all three must change together: `builtin/validators/code-hygiene/rules/missing-docs.md:71` says "Swift stays silent for every getter in a type that declares a conformance", and `builtin/validators/code-hygiene/rules/missing-docs.md:76` says "Swift stays silent inside a conforming type, for the reason above". State the condition you measured: the tool passes over the type declaration, each member and each nested item when the type declares an inherited type, and an inherited type is a protocol conformance, a superclass or a raw-value type.
