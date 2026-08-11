---
name: complexity-python
description: Python functions stay under the complexity gate — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: cognitive-complexity
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    ruff check --isolated --no-cache --config "lint.mccabe.max-complexity=15" --select C901 --output-format json "$@" |
      jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'
  doctor:
    check_command: "which ruff jq"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Complexity — Python

`ruff` reports every function that branches too much. `C901` is the one rule
that names that check, and `lint.mccabe.max-complexity` is the one threshold it
reads.

## What C901 measures, and what goes with it

`C901` is McCabe cyclomatic complexity: it counts the decision points in a
function. That is not the published Sonar cognitive complexity the `complexity`
probe computes, so the two numbers need not agree on the same function. The
tool gate replaces the prompt gate for Python — one number every reviewer gets
the same, in place of a number an agent reads off a probe.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
no ruff rule, so superseding drops it for Python. That is the trade the tool
rule makes.

Measured on a probe module: a function that nests a four-arm chain and a
`while` inside a loop, then reads eight flags and a mode, scores 17; its
refactored form — the sample bands moved to a helper and three flag steps moved
to a table — scores 10. Those two shapes are the fixture pair.

## How the run is shaped

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration. The
threshold then has to come from the command line, which is what `--config`
does. `--no-cache` keeps ruff from writing a cache directory into the
workspace.

The scope is `files` because ruff reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one function,
write `# noqa: C901` on its `def` line in the code.

## The run answers for its own arguments

`ruff check` reads a default target of `.` when it takes no path, and it
walks that whole tree. The script therefore counts its arguments first,
and a count of zero exits 0 with no finding.

Measured over two Python files, each holding one function of 17 branches,
with no argument: 2 findings before the guard, 0 after it. The same script
over the two files reports 2. The acceptance test
`the_shipped_python_complexity_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.
