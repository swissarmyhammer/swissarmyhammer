---
name: dead-code-go
description: Unexported Go items nothing in the package uses — checked by staticcheck, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: dead-code
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
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install honnef.co/go/tools/cmd/staticcheck@2025.1.1'
---

# Dead Code — Go

`staticcheck` reports every unexported type, struct field, method, constant,
variable, and function that no file of its package names. `U1000` is the one
check that names that lint, and `-checks U1000` turns every other staticcheck
check off, so the rule owns its whole invocation and never reads the project's
own `staticcheck.conf`.

Between the Go compiler and `U1000`, nothing is left for a reader to judge, and
that is why this rule supersedes the `dead-code` prompt rule for Go files.

- An unused local variable and an unused import are **compile errors**. The
  build is the gate, and a review never sees them; `staticcheck` reports them on
  the same stream as `compile` diagnostics, which the `jq` drops because they
  belong to the build rather than to this rule.
- An exported identifier is the package's surface for callers outside the
  module, so `U1000` never reports it — the carve-out is the language's own
  capitalization rule, not a judgment.
- A `func TestFoo(t *testing.T)` is invoked by the test harness, and `U1000`
  counts the harness as its caller.
- A `func main` and a `//export`ed cgo symbol are entry points for the same
  reason.

What is left is the narrow set a tool decides alone: an unexported item is the
package's own business, so the compiler already sees every caller it could ever
have, and no caller outside the module can explain the silence.

## The staging contract

Write `//lint:ignore U1000 <reason>` on the line above an item a later change
will use. Nothing else counts. A staged item with no marker is dead.

Measured against staticcheck 2025.1.1 on a probe package: an unannotated
unexported function reports, the same function under
`//lint:ignore U1000 the caller lands in the next change` does not, and
`//nolint:staticcheck` — the golangci-lint spelling — does **not** suppress it.
The marker works on every kind the check reports, the struct field included; the
passing fixture carries one of each.

The reason is part of the directive, not decoration. It names the change that
lands the consumer, so the next reader can tell staged work from a leftover.

## The measurement

Over `gohugoio/hugo` at HEAD: **13** findings, in 2 m 12 s from a cold build
cache. Every one is real — `stateOld`, `isTrueOld`, `evalFunctionOld`,
`evalFieldOld` and `evalCallOld`, five superseded copies left in a vendored
template evaluator; `_validateType`, a method renamed out of use; `_TestLinkerGC`,
a test disabled by an underscore; and four unused test constants. No false
positive.

## How the run is shaped

The scope is `workspace` because `U1000` reads a whole package. Passing only the
changed files makes the tool report every helper the unchanged files call: a
package where `a.go` defines `sharedHelper` and `b.go` calls it reports
`sharedHelper` unused when `a.go` is linted alone. `./...` loads the whole
module, and the engine keeps only the findings in the changed files.

Selection in the pipe is attribution, not exemption. `staticcheck` also emits
`compile` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. To exempt one item, write
`//lint:ignore U1000 <reason>` above it in the code.
