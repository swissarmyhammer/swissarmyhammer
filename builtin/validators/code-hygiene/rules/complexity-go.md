---
name: complexity-go
description: Go functions stay under the complexity gate — checked by gocognit, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: cognitive-complexity
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    gocognit -over 15 -json "$@" |
      jq -c '(. // [])[]
             | {file: .Pos.Filename, line: .Pos.Line,
                message: "cognitive complexity \(.Complexity) of func \(.FuncName) is over the gate of 15"}'
  doctor:
    check_command: "which gocognit go jq"
    check_version_command: "go version -m \"$(command -v gocognit)\" | awk '$1 == \"mod\" { print $2, $3 }'"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/uudashr/gocognit/cmd/gocognit@v1.2.1'
---

# Complexity — Go

`gocognit` reports every function whose cognitive complexity runs over the gate.
`-over N` is the one flag that names that check.

## This is the probe's own metric

`gocognit` implements the published Sonar cognitive complexity algorithm, which
is the same metric the `complexity` probe computes. Hand-checked on a probe
package: a `for` holding an `if` holding an `if`/`else if`/`else` scores 8 —
1 for the loop, 2 for the `if` one level in, 3 for the `if` two levels in, then
1 each for the `else if` and the `else`. That is the algorithm, term by term.

Length never leaks into the score. A flat 260-line function of `total += n`
scores 0.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
no gocognit flag, so superseding drops it for Go. That is the trade the tool
rule makes.

Measured over the Go 1.26.5 standard library — 4350 files, `cmd/` and every
`testdata` directory left out — `-over 15` reports 2731 functions, against the
29580 that `-over 0` reports, which is every function that branches at all. The
distribution across the gate is smooth — 356 functions at 13, 282 at 14, 256 at
15, 232 at 16 — with no mass piled just over it, which is what a contaminated
metric looks like.

## How the run is shaped

The scope is `files` because `gocognit` reads the paths it is given, one
function at a time, and needs neither a `go.mod` nor a loaded package.

`-json` prints `null`, not an empty array, when nothing is over the gate, so the
pipe starts with `(. // [])[]`.

`-over 15` also exits 1 whenever it printed something. The pipe ends in `jq`,
which exits 0, so a run with findings is not read as a broken script.

`gocognit` carries no version flag. `go version -m` reads the module path and
version out of the installed binary instead, and the `awk` keeps the one `mod`
line, so `sah doctor` shows `github.com/uudashr/gocognit v1.2.1`. `go` is
therefore named in `check_command` beside `gocognit` and `jq`.

`gocognit` has no suppression comment of its own. To exempt one function, split
it — that is the fix the finding asks for — or set the whole rule aside in this
project by overriding it in `./.validators/`.

## The run answers for its own arguments

`gocognit` holds no default target. Given no path it writes 39 lines of
usage text to stderr and exits nonzero. The pipe ends in `jq`, which exits
0, so that refusal reached the engine as a clean tree. The script counts
its arguments first, and a count of zero exits 0 with no finding.

Measured over two Go files, each holding one function of cognitive
complexity 21, with no argument: 0 findings and exit 0 before the guard,
and the same after it. The same script over the two files reports 2. So
the guard makes the 0 an answer of the script's own, and it keeps the
usage text off stderr. The acceptance test
`the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given`
holds that behaviour.
