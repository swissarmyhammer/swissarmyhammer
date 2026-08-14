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

## Why the script names no cache and no lock

`magic-numbers-go` and `function-length-go` each name a cache directory of their
own and each ask golangci-lint to serialize on its lock, because golangci-lint
answers by package CONTENT and stores each finding with the ABSOLUTE path of the
run that first cached it — so a second workspace holding the same bytes gets the
FIRST workspace's paths — and because a second instance sharing a cache stops
rather than waits. `staticcheck` needs neither, and the difference is its cache
key: the WORKSPACE PATH is part of it, so a copy of the same bytes at another
path is a cache MISS and no cached answer can carry a foreign path.

Measured with staticcheck 2025.1.1 over a module of 400 packages, each holding
one unused unexported function, and a copy of that module at a second path:

| the run | time | what it reported |
|---|---|---|
| the first directory, cold | 0.82 s | its own 400 paths |
| the first directory again | 0.21 s | its own 400 paths |
| the second directory, first run | 0.84 s | its own 400 paths |
| the second directory again | 0.24 s | its own 400 paths |

Row 2 is the cache hit, four times faster than row 1. Row 3 is the whole answer:
the same bytes at another path took the COLD time, so the second workspace shares
no cached answer with the first one. The first directory was then removed and the
second one still reported its own paths.

`staticcheck` takes no lock either. Eight runs of this script started together in
one workspace each reported all 400 findings, over two rounds.
