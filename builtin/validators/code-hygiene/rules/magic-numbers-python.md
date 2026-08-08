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

That scope is why this rule needs no threshold of its own. The rule already
matches the `magic-numbers` prompt carve-outs: it ignores `0`, `1`, `""`, and
`"__main__"`, and it never reads a literal a declaration names, a default
parameter, an index, or a call argument. Measured against a probe module holding
one literal of each of those kinds, it reported the comparison alone.

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration.
`--no-cache` keeps ruff from writing a cache directory into the workspace.

The scope is `files` because ruff reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one comparison,
write `# noqa: PLR2004` on it in the code.
