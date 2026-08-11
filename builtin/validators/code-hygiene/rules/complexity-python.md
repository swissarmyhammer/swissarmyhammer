---
name: complexity-python
description: Python functions stay under the complexity gate — checked by complexipy, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: cognitive-complexity
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    root="$PWD"
    : > "$work/findings.txt"
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'complexity-python: complexipy could not read %s\n' "$file" >&2
        exit 1
      fi
      rm -f "$work/report.sarif"
      status=0
      (cd "$work" && complexipy --max-complexity-allowed 15 --color no \
        --output-format sarif --output "$work/report.sarif" "$root/$file") \
        > "$work/console.txt" 2>&1 || status=$?
      measured=0
      if [ "$status" -eq 0 ] && [ -s "$work/report.sarif" ]; then
        measured=1
      elif [ "$status" -eq 1 ] &&
        jq -e '.runs[0].results | length > 0' "$work/report.sarif" >/dev/null 2>&1
      then
        measured=1
      fi
      if [ "$measured" -eq 0 ]; then
        cat "$work/console.txt" >&2
        printf 'complexity-python: complexipy could not read %s\n' "$file" >&2
        exit 1
      fi
      jq -r --arg file "$file" '.runs[0].results[]
             | select((.locations[0].logicalLocations[0].name // "")
                      | split("::") | last | startswith("test") | not)
             | "\($file):\(.locations[0].physicalLocation.region.startLine): \(.message.text)"' \
        "$work/report.sarif" >> "$work/findings.txt"
    done
    cat "$work/findings.txt"
  doctor:
    check_command: "which complexipy jq mktemp"
    check_version_command: "complexipy --version"
  install:
    commands:
      - "uv tool install complexipy==7.0.0"
      - "pipx install complexipy==7.0.0"
---

# Complexity — Python

`complexipy` reports every function whose cognitive complexity runs over the
gate. `--max-complexity-allowed` is the one flag that names that gate.

## This is the probe's own metric

`complexipy` implements the published Sonar cognitive complexity algorithm,
which is the same metric the `complexity` probe computes. Measured with
complexipy 7.0.0 on a probe module: a `for` holding an `if` holding an
`if`/`elif`/`else` scores 8 — 1 for the loop, 2 for the `if` one level in, 3
for the `if` two levels in, then 1 each for the `elif` and the `else`. That is
the algorithm, term by term.

Nesting is the thing it charges for. Measured over one function of six nested
`if` blocks, which is the shape the Rust, Go and TypeScript rules of this
roster each probe with: 21.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
no complexipy flag, so superseding drops it for Python. That is the trade the
tool rule makes.

Measured on the fixture pair: `classify_reading`, which folds a four-arm chain
and a `while` inside a loop and then reads eight flags and a mode, scores 21;
its refactored form — the sample bands moved to `_sample_step` and three flag
steps moved to a table — scores 12, and the helper scores 3.

## Why this rule left ruff

This rule ran `ruff check --select C901` at `lint.mccabe.max-complexity=15`
before. `C901` is McCabe cyclomatic complexity, which counts decision points
and reads no nesting at all. Measured over one function of each shape, with
ruff 0.14.5 and complexipy 7.0.0:

| the shape | `C901` | complexipy |
|---|---|---|
| 21-arm flat `if`/`elif` dispatch chain | 22 | 21 |
| the same 21 arms as a `match` statement | 22 | 1 |
| the same 21 arms as a dict beside `TABLE.get(mode, -1)` | 1 | 0 |
| six nested `if` blocks | 7 | 21 |

The last row is why the rule moved. `C901` scored the deeply nested function 7
against a gate of 15 and reported nothing, so the gate stayed silent on the one
shape the prompt rule exists for, and on the one shape every sibling rule of
this roster reports. The middle two rows are the other half: the Sonar metric
counts a `match` once for the whole construct and a table not at all, so it
rewards the two refactors that make a long dispatch readable, and `C901`
charges the `match` the full 22.

ruff ships no cognitive-complexity rule. Measured with ruff 0.14.5, the whole
list of `ruff rule --all` holds `C901 complex-structure`, `PLR0912
too-many-branches`, `PLR0911 too-many-return-statements`, `PLR0915
too-many-statements` and `PLR1702 too-many-nested-blocks`, and no other rule
that reads the shape of a function. `PLR1702` reads nesting and is preview-only
— measured, `--select PLR1702` writes `warning: Selection PLR1702 has no effect
because preview is not enabled` and reports nothing. The sibling
`function-length-python` still drives ruff, for `PLR0915`.

## How the run is shaped

The scope is `files` because `complexipy` reads the paths it is given, one
function at a time, and needs no package.

The run stands in the temporary directory rather than in the workspace, and it
names each file by an absolute path. Two measured behaviours ask for that.

- `complexipy` reads `[tool.complexipy]` out of the `pyproject.toml` of its
  WORKING DIRECTORY. Measured over one file that scores 21, beside a project
  `pyproject.toml` stating `max-complexity-allowed = 100`, with no gate on the
  command line: the run from the project directory reported 0 findings, and the
  run from a directory of its own reported 1. The gate on the command line wins
  over that project value as well — measured, the same project file beside
  `--max-complexity-allowed 15` reported 1 finding from either directory — and
  the run still stands outside the project, because the project's snapshot file
  and its `exclude` list read from the working directory too.
- `complexipy` writes a `.complexipy_cache` directory into its working
  directory on every run. Measured: the run from the workspace root left
  `.complexipy_cache` there; the run from the temporary directory left it under
  `mktemp -d`, and the trap removed it.

`--output` takes the path of the report, and `--output-format sarif` is the one
format that carries the row and the name of each function. Measured over one
file holding a class: the SARIF result names
`logicalLocations[0].name` as `TestThing::helper` and
`region.startLine` as the row of the definition. `--color no` keeps ANSI escapes
out of the console text the script forwards to stderr on a broken run.

The script hands `complexipy` one file at a time, and it holds each finding in
`"$work/findings.txt"` and writes them all at the end, so a run that breaks
writes NO finding. The section "A file the tool cannot read" below states what
the one call lost.

## The annotation an author writes

`complexipy` reads two directives, and each of them stands on the `def` line or
on the line directly above it:

    def load_settings(raw):  # complexipy: ignore  the option list is flat
        ...

Measured with complexipy 7.0.0, over one function that scores 21 against the
gate of 15. Each of these spellings gives no finding: `# complexipy: ignore` on
the `def` line; the same comment on the line directly above the `def`;
`# noqa: complexipy` on the `def` line; `#complexipy:ignore` with no space in
it; `# Complexipy: Ignore` with capital letters; and
`# complexipy: ignore  keep, config table` with text after the directive. Each
of these spellings gives one finding: the directive with a blank line between
it and the `def`; the directive on the first line of the body; the directive
under the docstring; a bare `# noqa`; and `# noqa: C901`, which named the ruff
rule this gate no longer runs.

The first fix a finding asks for is still to split the function. The directive
is the second fix, and the text beside it states why.

## The three carve-outs the superseded prompt rule states

`cognitive-complexity` exempts a test, it exempts a long flat list of simple
cases, and it exempts generated code. The run reproduces the first. The author
answers the second. Nothing answers the third.

### A test, which the run drops

The prompt rule states the mark: "Identify a test from its attribute or
framework naming convention at the **definition**, never from the file name. A
complex helper named `build_request` in a file called `foo_test.rs` is still a
complex function and is still listed."

Python states that convention twice, and the sibling `missing-docs-python`
reads the same two sources. pytest collects a function or method whose name
starts with `test` — read from pytest 9.1.1, `python_functions = ["test"]`.
unittest collects a method whose name starts with `test` — read from the
standard library, `unittest.TestLoader.testMethodPrefix` is `test`.

`complexipy` holds no flag that reads the name of a function. Its one file
filter is `--exclude <glob>`, which reads the PATH, and the prompt rule refuses
the path. So the filter in the script reads the NAME the SARIF report carries.
`logicalLocations[0].name` is the bare name of a function and `Class::method`
for a method, so the filter drops a finding whose name after the last `::`
starts with `test`.

Measured over one `tests/staged_test.py` holding a `TestThing.test_method` and
a module-level `build_request`, each scoring 21: the script without the filter
reported both, and the shipped script reports `build_request` alone. That is
the prompt rule's own sentence, and it is why the filter reads the name rather
than the path.

`--exclude '*_test.py'` would silence the file, and it would silence the helper
in it as well. The rule states no such expression, for the reason
`magic-numbers-go` refuses one for a shift operand: an expression that silences
the helper trades a true finding for the carve-out.

The acceptance test
`the_shipped_python_complexity_tool_rule_drops_a_test_function_and_keeps_its_helper`
holds both halves.

### Configuration parsing, which the author answers

The prompt rule exempts "Configuration parsing with many options, where the
score comes from a long flat list of simple cases rather than from nesting".

The metric does not make that carve-out for itself. The prompt rule states why
in its own words: "An `if` / `else if` / `else` chain is flat. Each branch adds
1, and no branch nests inside the one before it." Measured over a 21-arm flat
`if`/`elif` dispatch chain at nesting depth 1: complexipy scores it 21, so the
`complexity` probe scores it 21 as well and the prompt rule's carve-out is an
exemption ON TOP of that score.

`complexipy` holds no flag for a flat list of simple cases. So the author
answers this one, and there are two answers. The first is to write the dispatch
as the language writes a dispatch: measured over the same 21 arms, a `match`
statement scores 1 and a dict beside `TABLE.get(mode, -1)` scores 0, so both
refactors carry the function under the gate by measurement rather than by
argument. The second is `# complexipy: ignore` on the `def` line, with the
reason beside it.

### Generated code, which nothing answers

The prompt rule exempts "Generated code and macro expansions". This rule cannot
reproduce that carve-out, and the author cannot annotate it either.

- `complexipy` reads no file header. Measured over three files, each holding
  one function that scores 21: the file whose head carries `# Generated by the
  protocol buffer compiler.  DO NOT EDIT!`, the file whose head carries
  `# @generated`, and the plain file each reported their function.
- Its one file filter reads the PATH and never the content, and it reaches no
  file named on the command line at all. Measured over the same three files
  named as arguments: `--exclude 'DO NOT EDIT'`, `--exclude '*_pb2.py'` and
  `--exclude 'marked_pb2.py'` each dropped none of the three. Measured over the
  directory that holds them, walked rather than named: the walk reported 3, and
  `--exclude '*_pb2.py'` on the same walk reported 2. So the expression works,
  and this rule names each file, so no expression reaches it.
- Python states no generated-file header convention. Go states one, which is
  why the sibling `complexity-go` makes the test itself: a line matching
  `^// Code generated .* DO NOT EDIT\.$` above the first text that is neither a
  comment nor blank. A Python header test would name the first lines of one
  generator and never the convention, so this rule states none.
- The author cannot answer it with the directive, because the generator writes
  the file again and the directive goes away each time.

So a generated Python file REPORTS. The acceptance test
`the_shipped_python_complexity_tool_rule_reports_a_generated_file` holds the
rule to that, so the gap stays measured rather than discovered. A project that
generates Python keeps the generated tree out of the review with its own
ignore list, which is where the README puts a file list the project owns.

## The run answers for its own arguments

`complexipy` holds no default target. Measured with complexipy 7.0.0, given no
path: `You need to define paths in the CLI call arguments or in complexipy.toml
file`, and exit 1. So a run that reached the tool with no path would answer that
refusal and judge no file.

The script therefore counts its arguments one time, at its head, and a count of
zero exits 0 with no finding. That count is the guard the `run` key of
`builtin/validators/README.md` states for each `files`-scope rule, and it stands
above every line that runs. The coverage guard
`each_shipped_files_scope_script_answers_a_run_that_gives_it_no_file` holds each
such rule to the text and to the place.

The loop gives `complexipy` one file at a time, so the tool takes no empty
argument list even without the count. Measured over a probe tree of `top.py` and
`deep/nested/other.py`, each holding one function that scores 21, with the count
removed: the run with no argument reported nothing and exited 0, the same as the
shipped script. The count stands because the contract states it, and because it
answers before `mktemp -d` runs.

The acceptance test
`the_shipped_python_complexity_tool_rule_reads_only_the_files_it_is_given`
holds two halves: the run with no argument reports nothing, and the run over the
two staged files reports 2.

## A file the tool cannot read

`complexipy` keeps ONE exit status for a finding and for a failure. Measured
with complexipy 7.0.0, one file at each run:

| the file | the report | exit |
|---|---|---|
| one function over the gate | a SARIF run holding one result | 1 |
| no function over the gate | a SARIF run holding no result | 0 |
| an empty `.py` file | a SARIF run holding no result | 0 |
| a path that holds no file | a SARIF run holding no result | 1 |
| a syntax error | a SARIF run holding no result | 1 |
| a file with no read permission | a SARIF run holding no result | 1 |

The status alone therefore cannot tell a finding from a failure. The `run` key
of `builtin/validators/README.md` states the answer for a shared status: test
the REPORT beside the status, and accept the shared status only for the report
shape a measured run writes. So the script calls a run measured under two
shapes alone — status 0 with a report on disk, and status 1 with a report
holding one result or more. Every other answer forwards the tool's own console
text, writes `complexity-python: complexipy could not read <path>` to stderr,
and exits 1, so the engine reads a broken run rather than a clean file.

A path the script cannot read never reaches `complexipy`. The `[ ! -r "$file" ]`
test names that path and exits 1. The report gate above would break the run as
well — measured, an absent path and a file with no read permission each exit 1
with no result — so the test stands to name the path before the tool starts,
which is what the `run` key asks each script to do.

`complexipy` writes its own diagnosis to STDOUT and writes nothing to stderr.
Measured over a file whose function body never closes: `error: Failed to process
<path> - Please check file/folder exists or check syntax` on stdout, 0 bytes on
stderr, exit 1. The script captures that console text and forwards it to stderr
on a broken run, so the agent reads the tool's own words.

One call over the whole run loses that. Measured over one ordinary Python file
that scores 21 beside one file whose function body never closes, in each order:
`complexipy` reported the ordinary function and exited 1, so a script that made
one call reported 1 finding and exited 0, and the engine read the unparsable
file as a clean file. The loop reports no finding and exits 1.

Two acceptance tests hold the two causes apart:
`the_shipped_python_complexity_tool_rule_breaks_on_a_file_it_cannot_read`
stages no file at the named path, and
`the_shipped_python_complexity_tool_rule_breaks_on_a_file_it_cannot_parse`
stages a file whose function body never closes.
