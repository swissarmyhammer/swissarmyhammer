---
name: function-length-python
description: Python functions stay under the length gate — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: function-length
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    cat > "$work/definition.py" <<'REPORTED_DEFINITION'
    """States each file `ruff` declined, then writes each `PLR0915` finding
    whose definition is not a test.

    `ruff` writes no function name into its message. It anchors each `PLR0915`
    diagnostic on the NAME of the function it measured, so `location` and
    `end_location` select that identifier out of the source line, and the
    definition then states whether the function is a test.

    The read fails OPEN. A file this script cannot read states no name, so the
    finding stands.
    """

    import json
    import sys

    # What pytest and unittest each collect a test by, at the definition.
    # pytest 9.1.1 states `python_functions = ["test"]`, and the standard
    # library states `unittest.TestLoader.testMethodPrefix` is `test`.
    TEST_NAME_PREFIX = "test"

    # The one rule this run measures with. `ruff` writes a file it cannot parse
    # onto the same report under a code of its own.
    MEASURED_CODE = "PLR0915"

    # What the script calls itself on stderr.
    RULE_NAME = "function-length-python"

    # What opens the line `ruff` writes for a file it could not read, and what
    # stands after it: the path, then `: `, then the reason.
    DECLINED_HEAD = "warning: Failed to lint "

    # The marker `builtin/validators/README.md` states a declined item by.
    DIAGNOSTIC_MARKER = "sah-diagnostic:"


    def source_lines(path, cache):
        """The lines of the file at `path`, or none of them if it will not read."""
        if path not in cache:
            try:
                with open(path, encoding="utf-8") as handle:
                    cache[path] = handle.read().splitlines()
            except (OSError, UnicodeDecodeError):
                cache[path] = []
        return cache[path]


    def reported_name(finding, cache):
        """The identifier `ruff` anchored `finding` on, or an empty string.

        A Python name stands on one line, so a range across two rows names no
        definition. Every character before a name on its own line is ASCII —
        the indentation, `def `, `async def ` — so the columns, which count
        characters, select the name exactly.
        """
        start = finding["location"]
        end = finding["end_location"]
        if start["row"] != end["row"]:
            return ""
        lines = source_lines(finding["filename"], cache)
        if start["row"] > len(lines):
            return ""
        return lines[start["row"] - 1][start["column"] - 1:end["column"] - 1]


    def declined(path):
        """Each file `ruff` said it could not read, as `file: reason`.

        `ruff` states such a file on stderr and exits as it would without it,
        so this is the one place the run learns the file was never judged. The
        read replaces a byte it cannot decode rather than failing, because a
        path itself can hold one.
        """
        with open(path, encoding="utf-8", errors="replace") as handle:
            lines = handle.read().splitlines()
        return [line[len(DECLINED_HEAD):] for line in lines if line.startswith(DECLINED_HEAD)]


    def main():
        """States each declined file, then writes one JSON object for each
        finding the carve-out keeps."""
        for file in declined(sys.argv[2]):
            sys.stderr.write(
                "{} ruff could not read {}\n".format(DIAGNOSTIC_MARKER, file)
            )
        with open(sys.argv[1], encoding="utf-8") as handle:
            report = handle.read().strip()
        findings = json.loads(report) if report else []
        unmeasured = [row for row in findings if row.get("code") != MEASURED_CODE]
        for row in unmeasured:
            sys.stderr.write(
                "{}: ruff could not measure {}: {} {}\n".format(
                    RULE_NAME, row["filename"], row.get("code"), row.get("message")
                )
            )
        if unmeasured:
            sys.exit(1)
        cache = {}
        for finding in findings:
            if reported_name(finding, cache).startswith(TEST_NAME_PREFIX):
                continue
            sys.stdout.write(
                json.dumps(
                    {
                        "file": finding["filename"],
                        "line": finding["location"]["row"],
                        "message": "{} {}".format(finding["code"], finding["message"]),
                    }
                )
                + "\n"
            )


    main()
    REPORTED_DEFINITION
    ruff check --isolated --no-cache --config "lint.pylint.max-statements=180" \
      --select PLR0915 --output-format json "$@" > "$work/report.json" 2> "$work/ruff.err"
    status=$?
    if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
      cat "$work/ruff.err" "$work/report.json" >&2
      printf 'function-length-python: ruff exited %s and judged no code\n' "$status" >&2
      exit 1
    fi
    python3 "$work/definition.py" "$work/report.json" "$work/ruff.err"
  doctor:
    check_command: "which ruff python3 mktemp"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Function Length — Python

`ruff` reports every function that runs too long. `PLR0915` is the one rule
that names that check, and `lint.pylint.max-statements` is the one threshold it
reads.

## Why the threshold is 180

`PLR0915` counts statements, and the `function-length` prompt rule counts code
lines — 250 of them, blank lines and comment-only lines excluded. The two
counts are not the same number, so the threshold is derived rather than copied.

The ratio was measured, not guessed. Running ruff with
`lint.pylint.max-statements=0` makes it report every function and print that
function's exact statement count in the message. Run that way over the CPython
3.12 standard library, and compared against the code lines of each reported
function:

- 60 functions of 80 code lines or more hold a median of 0.732 statements for
  each code line.
- 22 functions of 120 code lines or more hold a median of 0.728.
- 8 functions of 150 code lines or more hold a median of 0.722.

The ratio is stable across the range, and it is below 1 for the reason Python
makes it so: a call spread over several lines is one statement, and an `else:`,
`elif:`, `except:`, or `finally:` header occupies a line without being a
statement of its own.

250 code lines times 0.72 is 180, so the rule sets
`lint.pylint.max-statements=180`.

Measured on a probe module: a function of 202 statements reports, and the same
function at 142 statements does not. Those two shapes are the fixture pair.

## The corpus every number below was measured over

Seven Python repositories, cloned at the commits below on 2026-08-14:

| repository | commit | `.py` files |
|---|---|---|
| ansible/ansible | `9cf16a4aca7898481c257f1e17ad28d0b67b1f85` | 1825 |
| django/django | `3436cf9bce84bb1f6877ad96819637366b27b719` | 2928 |
| fastapi/fastapi | `a1fa70d4237d50aae6586a0d9b229df583463d21` | 1136 |
| pallets/flask | `2a8a38b051fc248865730bf3511bf2e2ea325e81` | 83 |
| pandas-dev/pandas | `518f2a3cb9504555b40c1d5aaab4690245a7d265` | 1519 |
| psf/requests | `8068356288978c4f54661ae6f95afe0e0831885e` | 37 |
| python/cpython `Lib`, v3.12.12 | `4a5632fbf9bf59477c540e3f53fa7cdbeea3e3f5` | 1807 |

9335 files. `cpython`, `django`, `ansible` and `pandas` each carry a large test
suite, which is what the test carve-out below is measured against. The CPython
release is 3.12 because the ratio above was measured there, and because ruff
0.14.5 cannot parse the `lazy import` syntax CPython 3.15 is adding — 30 files
of a `main` checkout state a parse failure rather than a statement count.

At the gate of 180 the corpus reports **14** functions:

| repository | findings | a name that starts `test` | a test path | a generated head |
|---|---|---|---|---|
| ansible | 6 | 0 | 2 | 0 |
| django | 0 | 0 | 0 | 0 |
| fastapi | 0 | 0 | 0 | 0 |
| flask | 0 | 0 | 0 | 0 |
| pandas | 0 | 0 | 0 | 0 |
| requests | 0 | 0 | 0 | 0 |
| cpython | 8 | 6 | 6 | 0 |
| TOTAL | 14 | 6 | 8 | 0 |

## How the run is shaped

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration. The
threshold then has to come from the command line, which is what `--config`
does. `--no-cache` keeps ruff from writing a cache directory into the
workspace.

The scope is `files` because ruff reads the files it is given.

`mktemp -d` makes the directory the filter program and the report stand in, and
`trap 'rm -rf "$work"' EXIT` removes it.

### What `--isolated` costs, and what it buys

`--isolated` is the reason a project's `per-file-ignores` entry for its tests
does not reach this gate. That cost is paid deliberately, and the section below
states what the rule does instead.

The rest of the command line already wins over the project. Measured with ruff
0.14.5 over one probe of `src.py` and `tests/test_thing.py`, three functions of
200 statements, against the shipped flags:

| the project's own `pyproject.toml` | `--isolated` | the project's configuration |
|---|---|---|
| none | 3 | 3 |
| `[tool.ruff.lint] ignore = ["PLR0915"]` | 3 | 3 |
| `[tool.ruff.lint] select = []` | 3 | 3 |
| `[tool.ruff.lint.pylint] max-statements = 500` | 3 | 3 |
| `[tool.ruff] exclude = ["src.py", "tests"]` | 3 | 3 |
| `[tool.ruff.lint.per-file-ignores] "tests/*" = ["PLR0915"]` | 3 | 1 |
| `[tool.ruff.lint.per-file-ignores] "*" = ["PLR0915"]` | 3 | 0, at exit 0 |
| `[tool.ruff.lint.extend-per-file-ignores] "*" = ["PLR0915"]` | 3 | 0, at exit 0 |
| `[project` — not TOML at all | 3 | nothing, at exit 2 |

`--select` on the command line beats the project's `ignore` and its `select`,
and `--config` beats the project's `max-statements`, so `per-file-ignores` is
the ONE project setting that reaches this gate. It is also the setting that
turns the WHOLE gate off — rows 7 and 8 answer 0 findings at exit 0 while doing
it — and row 9 is a manifest a project is editing, which without `--isolated`
stops the run with an empty report. That is the hazard the sibling
`dead-code-python` measured for vulture, in ruff's own form, so `--isolated`
stays.

### Selection in the pipe is attribution

The filter program keeps the rows whose code is `PLR0915`. ruff writes a file
it cannot parse onto the same report under `"code": "invalid-syntax"`, and
those rows belong to the parser rather than to this rule. The section "A file
ruff could not measure" below states what the run does with them.

To exempt one function, write `# noqa: PLR0915` on its `def` line in the code.

## The tool space, and what ruff cannot do

Every configuration option of ruff 0.14.5 was read with `ruff config`, each
group walked to its leaves: 166 options. Against the three carve-outs this rule
has to answer:

| the carve-out | what ruff offers |
|---|---|
| a test | nothing that reads a NAME for `PLR0915` |
| a class a runner loads | nothing that reads a BASE class at all |
| generated code | no generated-file heuristic, and no option that holds the word |

The options that do read a name each belong to their own linter and reach no
other rule: `lint.pep8-naming.ignore-names` covers the `N` rules,
`lint.flake8-self.ignore-names` the `SLF` rules, `lint.pydocstyle.ignore-decorators`
the `D` rules, `lint.flake8-type-checking.runtime-evaluated-decorators` the `TC`
rules, and `lint.pylint.allow-dunder-method-names` `PLW3201` alone.
`lint.pylint` holds 11 settings and `max-statements` is the only one `PLR0915`
reads. Every exemption ruff offers for `PLR0915` is a PATH —
`per-file-ignores`, `extend-per-file-ignores`, `exclude` — or the in-code
`# noqa`.

What ruff DOES state is the definition. It writes no name into its message —
`Too many statements (200 > 180)` — and it anchors each `PLR0915` diagnostic on
the function's own NAME. Measured with ruff 0.14.5:

| the declaration | the range ruff reports | the text it selects |
|---|---|---|
| `    def test_dense(self):` | row 2, column 9 to 19 | `test_dense` |
| `async def test_async_thing():` under two decorators | row 3, column 11 to 27 | `test_async_thing` |
| `    def __init__(self):` | row 2, column 9 to 17 | `__init__` |
| `def café_été(é):` | row 2, column 5 to 13 | `café_été` |

The last row states that a column counts CHARACTERS rather than bytes. Every
character standing before a name on its own line is ASCII — the indentation,
`def `, `async def ` — so the two columns select the name exactly. No output
format carries the name instead: measured, ruff's SARIF writes no
`logicalLocations`, and its `rdjson` writes the same range and no name.

## The four carve-outs the prompt rule states

`function-length` exempts four shapes: a test, generated code, a function that
is mostly configuration or data, and an initialization function that sets many
fields. The metric drops the third for itself, the run drops the first, and
nothing drops the other two.

### Configuration and data, which the metric drops

`function-length` exempts "Functions that are mostly configuration/data (e.g.,
builder patterns with many options)". `PLR0915` counts statements, and a
literal is one expression however many rows it holds. Measured with ruff 0.14.5
on a probe module, each shape's own statement count read by bisecting the gate:

| the shape | lines | statements | the run at 180 |
|---|---|---|---|
| a procedure of 200 assignments | 201 | 200 | reports |
| a mapping literal of 400 rows, returned | 403 | 0 | silent |
| a mapping literal of 400 rows, named then returned | 404 | 1 | silent |
| a sequence literal of 400 rows, named then returned | 404 | 1 | silent |
| a builder chain of 400 options, named then returned | 405 | 1 | silent |

A trailing `return` adds nothing of its own: measured, `def one(): return 1`
holds 0 statements and stays silent at a gate of 0.

### A test, which the run drops by the DEFINITION

`function-length` exempts "Functions explicitly marked as tests", and
this set names the mark: identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. A complex helper
named `build_request` in a file called `foo_test.rs` is still a long function
and is still listed.

Python states that convention twice, and the sibling `missing-docs-python`
reads the same two sources. pytest collects a function or
method whose name starts with `test` — read from pytest 9.1.1,
`python_functions = ["test"]`. unittest collects a method whose name starts
with `test` — read from the standard library,
`unittest.TestLoader.testMethodPrefix` is `test`.

The filter program therefore drops a finding whose name starts with `test`, and
that name is the one ruff anchored the finding on. Over the corpus, at the gate
of 180:

| the run | findings | in a test path |
|---|---|---|
| no test carve-out | 14 | 8 |
| the shipped name filter | 8 | 2 |
| a path exclusion instead | 6 | 0 |

The path exclusion is the row that is wrong. It drops 8 rather than 6, and the
two it drops beyond the carve-out are helpers the prompt rule still lists:

- `ansible` `test/lib/ansible_test/.../validate-modules/validate_modules/main.py`
  `_validate_argument_spec`, 371 statements.
- `ansible` `test/lib/ansible_test/_util/target/sanity/import/importer.py`
  `main`, 246 statements.

That is the trade `function-length-go` refuses for `_test.go`. The acceptance test
`the_shipped_python_function_length_tool_rule_reads_a_test_from_its_definition`
holds one file carrying `test_dense` over the gate, `build_request` over the
gate, and a 300-row data table, and holds the run to reporting `build_request`
alone.

Go's own carve-out reads BOTH halves of `go test` — the name and the `_test.go`
file name — because `go test` requires both. Python requires neither: pytest's
`python_files` and unittest's discovery pattern are configuration, not a
language rule, and the prompt rule refuses the file name outright. So this rule
reads the name and nothing else, which is what `missing-docs-python` already
does for the same language.

### An initializer that sets many fields, which nothing drops

`function-length` exempts "Initialization functions that set many fields". Each
`self.x = ...` is one statement, so `PLR0915` cannot tell such an initializer
from a procedure, and ruff holds no option that would.

The measurement says no mechanism is needed. Every function of the 9335 corpus
files was parsed and its `self.x = ...` statements counted:

| the widest field setters of the corpus | `self.x = ...` | statements |
|---|---|---|
| `cpython Lib/test/libregrtest/main.py` `__init__` | 55 | 67 |
| `pandas pandas/plotting/_matplotlib/core.py` `__init__` | 48 | 74 |
| `ansible lib/ansible/modules/user.py` `__init__` | 44 | 57 |
| `django django/db/models/options.py` `__init__` | 41 | 42 |
| `cpython Lib/test/libregrtest/cmdline.py` `__init__` | 40 | 42 |

The widest sets 55 fields, and the gate is 180 — 3.3 times above it. One
initializer of the corpus does stand over the gate,
`cpython Lib/idlelib/editor.py:74` `__init__` at 206 statements, and 23 of
those 206 set a field. It is a long procedure, not the shape the prompt rule
exempts.

So a field-setting initializer over the gate REPORTS, and the author answers
it. The first answer is to move the fields into a default for each one, or into
a table the initializer reads. The second answer is the annotation below. This
is the verdict `function-length-go` and `function-length-swift` each record for
the same carve-out. The acceptance test
`the_shipped_python_function_length_tool_rule_reports_a_field_setting_initializer`
holds one bare initializer of 190 field-setting statements beside the same one
annotated, and holds the run to reporting the bare one alone.

### Generated code, which nothing drops

`function-length` exempts generated code. ruff holds no generated-file
heuristic — no option of the 166 names one — and Python states no header
convention for one either. Go states one, which is why `function-length-go`
drops a generated file; a Python header test would name the first lines of one
generator and never the convention. That is the verdict `function-length-rust`
records for Rust, for the same reason.

Measured over the corpus: 0 of the 14 findings stand in a file whose head
carries a generated mark, so the gap costs the corpus nothing. Measured on a
probe: one function of 200 statements reports whether or not the head carries
`# Generated by the protocol buffer compiler.  DO NOT EDIT!`.

The author cannot answer this carve-out with the annotation either, because the
generator writes the file again and the annotation goes away each time. A
project that generates Python keeps the generated tree out of the review with
its own ignore list, which is where the README puts a file list the project
owns. The acceptance test
`the_shipped_python_function_length_tool_rule_reports_a_generated_file` holds
the rule to that, so the gap stays measured rather than discovered.

## The annotation an author writes

To exempt one function, write `# noqa: PLR0915` on its `def` line, with the
reason beside it:

    def __init__(self):  # noqa: PLR0915  one field for each column of the form
        ...

Measured with ruff 0.14.5 over one function of 200 statements against the gate
of 180: `# noqa: PLR0915` on the `def` line gives no finding, and a bare
`# noqa` on the same line gives none either. ruff names the `def` line itself in
`noqa_row`, so that is where the annotation goes for a decorated function as
well — measured over the same function under two decorators, the annotation on
the `def` line gives no finding and the same annotation on the first decorator
line gives one.

The first fix a finding asks for is still to split the function. The annotation
is the second fix, and the reason beside it states why.

## The run answers for its own arguments

`ruff` takes a default target of `.`, so a run with no path walks the whole
tree for the statement gate. The script counts its arguments first, and a
count of zero exits 0 with no finding.

Measured over two Python files, each holding one function of 190
statements, with no argument: 2 findings before the guard, 0 after it. The
same script over the two files reports 2. The acceptance test
`the_shipped_python_function_length_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

## A file ruff could not measure

The run reads two statements ruff makes about a file it could not measure: a
row of another code on the report, and a `Failed to lint` line on stderr. The
run answers each one differently, and the two sections below state each answer
against what was measured for it.

### A file ruff cannot parse

`ruff` writes a file it cannot parse onto the SAME report as a finding, under
`"code": "invalid-syntax"`, and it exits 1 either way. Measured with ruff
0.14.5 over a file whose `if` body never opens: one row of that code, and no
statement count for the file at all.

An earlier shape of this run piped every row into a finding, so a file that
does not parse was reported as a function-length finding — 7 files of the
corpus do that, and every one of them is a deliberate fixture of a test suite:
`cpython Lib/test/tokenizedata/badsyntax_3131.py`, five Python-2 grammar files
under `cpython Lib/test/test_lib2to3/data`, and
`django tests/test_runner_apps/tagged/tests_syntax_error.py`. A run that
selected `PLR0915` and dropped the rest instead would read each of those files
as clean, which is worse: ruff measured no statement count there.

So the filter program writes each row of another code to stderr, naming the
file and the parser's own message, and exits 1. The engine then reads a broken
run rather than a clean file. The acceptance test
`the_shipped_python_function_length_tool_rule_breaks_on_a_file_it_cannot_parse`
holds that.

### A path ruff cannot read

`ruff` states a path it could not read on stderr, and it exits as it would
without that path. Measured with ruff 0.14.5, one path for each run, against
the shipped command line:

| the path | the report | stderr | exit |
|---|---|---|---|
| a path that holds no file | `[]` | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | 0 |
| a file whose bytes are not UTF-8 | `[]` | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | 0 |
| a file with no read permission | `[]` | `warning: Failed to lint noread.py: Permission denied (os error 13)` | 0 |

Each row is a path the tool declined. ruff judges every other file the run was
handed, so neither the report nor the status carries the decline. An earlier
shape of this run read the report alone, and the engine then read a path ruff
never opened as a clean file.

The filter program therefore reads ruff's stderr for a line opening
`warning: Failed to lint `, and writes what stands after that head under the
marker `builtin/validators/README.md` states, at exit 0:

    sah-diagnostic: ruff could not read absent.py: No such file or directory (os error 2)

The engine renders each marked line in the report, and no file filter drops it,
because a diagnostic is about the RUN and has no path to be kept by.

`exit 1` is the answer this rule does NOT give here. A nonzero exit fails the
WHOLE run, so one path the tool could not open throws away every finding the
run did make, and the engine then reads no diagnostic either — it reads a
broken run. Measured with the shipped script over one file of 190 statements
handed to the run beside all three paths of the table: one finding on stdout,
three marked lines on stderr, exit 0. That finding is what an `exit 1` here
would cost.

A test of the PATH cannot answer all three rows. Measured against the three
staged paths: `[ ! -r "$file" ]` is true for the path that holds no file and
for the file with no read permission, and FALSE for the file whose bytes are
not UTF-8 — the mode lets a reader open that one. A run gated on that test
would read the third file as clean. The answer has to come from what ruff
itself said, which is the shape the three swiftlint rules already take: each
reads swiftlint's own stderr message rather than a file name.

Three acceptance tests hold the three rows, one for each —
`the_shipped_python_function_length_tool_rule_declines_a_path_that_holds_no_file`,
`..._declines_a_file_it_cannot_decode` and `..._declines_a_file_it_may_not_read`.
Each stages one function of 190 statements beside the path, and holds the run
to reporting that finding AND to stating one diagnostic that names the path. A
run that lost either half fails them.

### Both answers in one run

Measured with the shipped script over a file that does not parse beside a path
that holds no file: the marked line and the parse failure both stand on stderr,
and the run exits 1. The engine reads a broken run there, and the whole of
stderr is the error detail it states, so the marked line reaches the reader
inside that error rather than as a diagnostic of its own.

The script tests ruff's own exit status beside all of it. Measured with ruff
0.14.5: 0 for a report with no row, 1 for a report with one row or more. Any
other status writes ruff's stderr and its report to stderr and exits 1, so a
ruff that refuses its command line never reads as a clean tree.
