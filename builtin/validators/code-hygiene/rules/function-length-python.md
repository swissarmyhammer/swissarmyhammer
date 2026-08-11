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
    ruff check --isolated --no-cache --config "lint.pylint.max-statements=180" --select PLR0915 --output-format json "$@" |
      jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'
  doctor:
    check_command: "which ruff jq"
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

## How the run is shaped

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration. The
threshold then has to come from the command line, which is what `--config`
does. `--no-cache` keeps ruff from writing a cache directory into the
workspace.

The scope is `files` because ruff reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one function,
write `# noqa: PLR0915` on its `def` line in the code.

## The run answers for its own arguments

This rule and `complexity-python` drive one tool, so they share its
default target of `.`. A run with no path walks the tree for the statement
gate as it does for the branch gate. The script counts its arguments
first, and a count of zero exits 0 with no finding.

Measured over two Python files, each holding one function of 190
statements, with no argument: 2 findings before the guard, 0 after it. The
same script over the two files reports 2. The acceptance test
`the_shipped_python_function_length_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.
