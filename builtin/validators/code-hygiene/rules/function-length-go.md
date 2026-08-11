---
name: function-length-go
description: Go functions stay under the length gate — checked by funlen, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: function-length
tool:
  scope: workspace
  run: |
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    config="$work/golangci.yml"
    cat > "$config" <<'FUNLEN_CONFIG'
    version: "2"
    linters:
      default: none
      enable:
        - funlen
      settings:
        funlen:
          lines: 250
          statements: 10000
          ignore-comments: true
    issues:
      max-issues-per-linter: 0
      max-same-issues: 0
    FUNLEN_CONFIG
    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... 2>/dev/null |
      jq -c '(.Issues // [])[] | select(.FromLinter == "funlen")
             | {file: .Pos.Filename, line: .Pos.Line, message: .Text}'
  doctor:
    check_command: "which golangci-lint go jq mktemp"
    check_version_command: "golangci-lint --version"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@v2.12.2'
---

# Function Length — Go

`funlen` reports every function that runs too long. Its `lines` limit with
`ignore-comments` on is the closest metric Go has to the code-line count the
`function-length` prompt rule states.

## Why funlen's line count, and not a statement count

The prompt rule counts 250 lines of code, blank lines and comment-only lines
excluded. Three Go metrics could stand for that number, so all three were
measured rather than picked.

A Go program replicating funlen's `parseStmts` and `getLines` from its source,
and computing each function's true code lines from `go/scanner` tokens, ran over
the Go 1.26.5 standard library plus the `github.com` tree of the module cache,
132 MB of third-party source — 94774 functions in all. Of those, 379 are
genuinely over 250 code lines.

| metric | median ratio to code lines, functions of 250+ code lines | at a gate of 250: findings / false positives / missed |
|---|---|---|
| funlen `lines`, `ignore-comments: true` | 1.002 | 412 / 39 / 6 |
| revive `function-length` lines | 1.132 | 517 / 138 / 0 |
| funlen `statements` | 0.763, but p10 0.012 and p90 0.997 | not usable |

funlen with comments ignored counts code lines plus blank lines, and a long Go
function carries almost none of the latter, so the ratio sits on 1.00 and the
gate can be the prompt rule's own 250 with no correction.

revive's `function-length` rule counts raw physical lines — its `countLines` is
`End().Line - Pos().Line - 1` and subtracts nothing — so it charges a function
for the comments the prompt rule excludes, and reports 138 functions that are
under the real limit.

A statement count cannot stand for a line count in Go at all. The ratio spans
80x across the range, because a 400-line composite literal is one statement.

## Why the statement limit is 10000

`funlen` runs its statement check first and `continue`s past the line check when
it fires, so the statement dimension has to be out of reach for the line gate to
be the gate. It cannot be turned off: `NewAnalyzer` reads `stmtLimit == 0` as
"use the default of 40". The largest statement count in the 94774-function
corpus is 6400 — `rewriteValueAMD64` in the Go compiler — so 10000 clears every
real function.

## Why golangci-lint runs the lint

`funlen` ships a standalone binary and that binary has no threshold flags.
`funlen -flags` lists `V`, `all`, `c`, `flags`, `json`, `source`, `tags`, `test`
and `v`, and nothing else; `lines` and `statements` are hardwired to 60 and 40
in `NewAnalyzer`. A standalone run therefore cannot gate at 250, whatever the
command line says.

`golangci-lint` carries the same funlen analyzer and configures it. That is the
same verdict, reached the same way, that `magic-numbers-go` records for `mnd`,
and it is the same pinned tool.

## Why the run is shaped this way

The script writes its own configuration to a temporary path and passes it with
`--config`, so the rule never reads the project's own `.golangci.yml`.
`default: none` with `enable: [funlen]` turns every other linter off.

`max-issues-per-linter` and `max-same-issues` are both `0`, which means no
limit. The golangci-lint defaults are 50 and 3, and the second one is the
dangerous one: it keeps three findings that share a message and silently drops
the rest.

`--path-mode abs` makes every reported path absolute. Without it golangci-lint
reports a path relative to the configuration file's own directory, which is a
temporary directory, so every path would point outside the workspace.

The scope is `workspace` because golangci-lint loads packages, not loose files,
and `./...` loads the whole module. The engine keeps only the findings in the
changed files.

Selection in the pipe is attribution, not exemption. `golangci-lint` also emits
`typecheck` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. To exempt one function, write
`//nolint:funlen // <reason>` on it in the code.

## The temporary directory the configuration stands in

`mktemp -d` makes the directory the golangci-lint configuration is written
into, and `trap 'rm -rf "$work"' EXIT` removes it. The scope is
`workspace`, so this script takes no file argument and the trap is the one
change of the pair this rule needed. Measured over a Go module of one
file: one run raised the count of entries under `TMPDIR` by 1 before the
trap, and leaves that count unchanged after it.
