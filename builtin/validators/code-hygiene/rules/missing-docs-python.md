---
name: missing-docs-python
description: Public Python items need docstrings — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: missing-docs
tool:
  scope: files
  run: |
    ruff check --isolated --no-cache --select D1 --output-format json "$@" |
      jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'
  doctor:
    check_command: "which ruff jq"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Missing Documentation — Python

`ruff` reports every public module, class, method, and function without a
docstring. The `D1` selector names that rule group.

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration.
`--no-cache` keeps ruff from writing a cache directory into the workspace.

The scope is `files` because ruff reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one item, write
`# noqa: D103` on it in the code.
