---
name: unreachable-code-python
description: Python statements behind a jump that always runs — checked by vulture, not by prompt.
match:
  files:
    - "**/*.py"
    - "**/*.pyw"
  project_types:
    - python
tool:
  scope: files
  run: |
    vulture --min-confidence 100 "$@" |
      sed -n 's/^\(.*: unreachable code after .*\) (100% confidence)$/\1/p'
  doctor:
    check_command: "which vulture sed"
    check_version_command: "vulture --version"
  install:
    commands:
      - "uv tool install vulture==2.14"
      - "pipx install vulture==2.14"
---

# Unreachable Code — Python

`vulture` reports every statement that sits behind a jump its branch always
takes — after `return`, after `raise`, after `continue`, and after `break`. The
function can never run it.

`--min-confidence 100` is what makes this rule narrow, and the number is chosen,
not inherited. Vulture grades each finding: an unused function, method, or
attribute scores 60, an unused import scores 90, and unreachable code scores
100. Only the last is a fact about the code alone. The 60 tier reads a name and
guesses, so it reports the framework-invoked override and the attribute a
library consumes by name; the 90 tier reports an import a string annotation
uses. Unreachable code is a control-flow fact, so the floor of 100 admits that
one kind and nothing else.

This rule supersedes nothing. The `dead-code` prompt rule keeps running, with
its carve-outs for entry points, exported public API, and work-in-process
scaffolding. This rule decides only the one question those carve-outs never
reach: code behind a jump has no future consumer, so no staging argument can
explain it.

The scope is `files` because a jump and the statement behind it live in one
function. Reading the file alone is the whole analysis, and reading more files
would not change an answer.

Selection in the pipe is attribution, not exemption. The `sed` keeps the
unreachable-code lines and drops any other kind a later vulture might score at
100, and it strips the confidence suffix the tool appends. Ending the pipe in
`sed` also normalizes the exit status, because vulture exits 3 when it has
findings. To exempt one statement, write `# noqa` on it in the code, or list its
name in a whitelist the code owns.
