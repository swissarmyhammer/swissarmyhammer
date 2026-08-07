---
name: unused-code-go
description: Unexported Go items nothing in the package uses — checked by staticcheck, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
tool:
  scope: workspace
  run: |
    staticcheck -checks U1000 -f json ./... |
      jq -c 'select(.code == "U1000")
             | {file: .location.file, line: .location.line, message: .message}'
  doctor:
    check_command: "which staticcheck go jq"
    check_version_command: "staticcheck --version"
  install:
    commands:
      - "go install honnef.co/go/tools/cmd/staticcheck@2025.1.1"
---

# Unused Code — Go

`staticcheck` reports every unexported type, struct field, method, constant,
variable, and function that no file of its package names. `U1000` is the one
check that names that lint, and `-checks U1000` turns every other staticcheck
check off, so the rule owns its whole invocation and never reads the project's
own `staticcheck.conf`.

This rule supersedes nothing. The `dead-code` prompt rule keeps running, with
its carve-outs for entry points, exported public API, and work-in-process
scaffolding. Those carve-outs need judgment and the `callers` probe, which a
tool cannot supply. `U1000` is the narrow half a tool can decide alone: an
unexported item is the package's own business, so the compiler already sees
every caller it could ever have, and no caller outside the repository can
explain the silence.

The scope is `workspace` because `U1000` reads a whole package. Passing only the
changed files makes the tool report every helper the unchanged files call: a
package where `a.go` defines `sharedHelper` and `b.go` calls it reports
`sharedHelper` unused when `a.go` is linted alone. `./...` loads the whole
module, and the engine keeps only the findings in the changed files.

Selection in the pipe is attribution, not exemption. `staticcheck` also emits
`compile` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. To exempt one item, write
`//lint:ignore U1000 <reason>` above it in the code.
