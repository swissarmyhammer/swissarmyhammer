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
    cache="${TMPDIR:-/tmp}/sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    config="$work/golangci.yml"
    cat > "$config" <<'FUNLEN_CONFIG'
    version: "2"
    run:
      allow-serial-runners: true
    linters:
      default: none
      enable:
        - funlen
      settings:
        funlen:
          lines: 10000
          statements: 160
          ignore-comments: true
      exclusions:
        rules:
          - linters:
              - funlen
            path: _test\.go$
            text: "^Function '(Test|Benchmark|Fuzz|Example)([^\\p{Ll}].*)?' "
    issues:
      max-issues-per-linter: 0
      max-same-issues: 0
    FUNLEN_CONFIG
    GOLANGCI_LINT_CACHE="$cache" \
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

`funlen` reports every function that runs too long. It holds two dimensions,
`lines` and `statements`, and this rule gates on the second one.

## The corpus every number below was measured over

Six well-known Go repositories, cloned at HEAD on 2026-08-14:

| repository | commit |
|---|---|
| kubernetes/client-go | `3fcdd4c72588c077802ae4c6a3fec8375665080b` |
| spf13/cobra | `adbc8813901bba65827259daa8e22ff94ec1f30e` |
| etcd-io/etcd | `0836b69e9cf47d00b535f2bc331b4c47bb23cb80` |
| gin-gonic/gin | `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd` |
| grpc/grpc-go | `bf9e7cd3430df40d0732ba42eb88bd5f2cc63407` |
| prometheus/prometheus | `05f9eb8b3b8e10b48c8f4153b0714dbe9bc9a630` |

5470 `.go` files, 1290 of them `_test.go`, in 32 Go modules. Each module was run
two times through golangci-lint: one run at `lines: 1, statements: 10000`, which
makes funlen print every function's own LINE count in its message, and one at
`lines: 10000, statements: 1`, which makes it print every function's own
STATEMENT count. 23216 functions came back with both numbers, so every sweep
below is arithmetic on the tool's own counts rather than on a model of them.

## Why the gate is a statement count

`funlen` ORs its two dimensions. `funlen.go` runs the statement check first and
`continue`s past the line check when it fires, and that `continue` is there so
one function reports one time — not so one dimension can excuse the other. A
statement limit standing BESIDE a line gate can therefore only add findings.
Measured at `lines: 250`:

| `statements` | findings | in `_test.go` |
|---|---|---|
| 40 | 982 | 625 |
| 80 | 253 | 185 |
| 120 | 180 | 149 |
| 180 | 162 | 142 |
| 250 | 161 | 142 |
| 10000 | 161 | 142 |

The line gate at 250 is therefore the floor of that column, and its own finding
set is nearly all shapes the `function-length` prompt rule exempts. Of the 161
functions over 250 funlen lines, 136 hold 40 statements or fewer:

| statements | findings at `lines: 250` | in `_test.go` |
|---|---|---|
| 0..20 | 114 | 111 |
| 21..40 | 22 | 19 |
| 41..80 | 6 | 6 |
| 81..120 | 3 | 1 |
| 121..180 | 6 | 1 |
| over 180 | 10 | 4 |

The two dimensions select two different populations, and the ratio states it:

| population | statements for each funlen line |
|---|---|
| functions of 250+ lines | p10 0.004, median 0.017, p90 0.290 |
| functions of 100+ statements | p10 0.437, median 0.633, p90 0.780 |

A median of 0.017 is four statements over 250 lines, which is a data literal.
`cobra/bash_completions.go` `writePreamble` runs 365 lines and holds 2
statements — one call writing one shell script. `prometheus` `HandleDropletsList`
runs 589 lines and holds 1. Lines select data; statements select code.

So the statement count is the gate, and the line limit stands out of reach.

## Why the earlier line gate measured the wrong thing

This rule gated on `lines: 250` before, and the measurement behind that number
was sound for the question it asked. A Go program replicating funlen's
`parseStmts` and `getLines`, over the Go 1.26.5 standard library plus 132 MB of
the module cache — 94774 functions — put funlen's `lines` with
`ignore-comments: true` at a median ratio of 1.002 to the true code lines a
`go/scanner` walk counts, against 1.132 for revive's `function-length`. funlen's
line count IS the prompt rule's own count of code lines, and that is still true.

The ground truth was the wrong one. It was "over 250 code lines", and the prompt
rule does not state that a function over 250 code lines is a finding: it states
that one is a finding UNLESS it is mostly configuration or data, an
initialization function that sets many fields, generated code, or a test. Take
the exempt shapes out of the ground truth and the line count stops being the
metric, because the shapes it selects are almost entirely the exempt ones.

The same measurement recorded `funlen statements` as "not usable", on a ratio to
code lines that spans p10 0.012 to p90 0.997. That spread IS the carve-out doing
its work: a function holds far fewer statements than lines exactly when its
lines are data.

## Why the gate is 160

The prompt rule counts 250 code lines. The procedural population — the 58
functions of 100 statements or more — holds a median 0.633 statements for each
funlen line, and funlen's line count is the code-line count. 250 times 0.633 is
158, so the gate is 160. This is the derivation `function-length-python` makes
for its own `PLR0915` threshold of 180; Go's ratio is lower than Python's 0.72,
so Go's number is lower.

Measured, 160 is where the sweep turns. A statement gate reports a function
whatever its line count, so a gate under the ratio reports functions the prompt
rule does not list at all:

| `statements`, `lines: 10000` | findings | over 250 lines | under 250 lines |
|---|---|---|---|
| 120 | 35 | 16 | 19 |
| 140 | 17 | 12 | 5 |
| 160 | 12 | 11 | 1 |
| 180 | 11 | 10 | 1 |
| 200 | 7 | 7 | 0 |

At 160 the corpus reports 12 functions and 11 of them stand over the prompt
rule's own 250 lines as well. The one that does not is `grpc-go`
`interop/client/client.go` `main`, 235 lines of 187 statements.

The trade is stated rather than hidden: a function whose length comes from long
expressions rather than from statements passes. `prometheus/tsdb/head_wal.go`
`loadWAL` runs 396 lines on 122 statements and this gate is silent on it. A
missed finding leaves the review where it was, and a wrong finding is a
requirement to change correct code, which is the same trade `complexity-swift`
records for `ignores_case_statements`.

## Why the line limit is 10000

The line dimension cannot be turned off: `NewAnalyzer` reads `lineLimit == 0` as
"use the default of 60". It has to be put out of reach instead. The largest
funlen line count in the corpus is 3294, so 10000 clears every real function.

## Why golangci-lint runs the lint

`funlen` ships a standalone binary and that binary has no threshold flags.
`funlen -flags` lists `V`, `all`, `c`, `flags`, `json`, `source`, `tags`, `test`
and `v`, and nothing else; `lines` and `statements` are hardwired to 60 and 40
in `NewAnalyzer`. A standalone run therefore cannot gate at 160, whatever the
command line says.

`golangci-lint` carries the same funlen analyzer, configures it, and carries the
exclusion machinery the test carve-out below needs. That is the same verdict,
reached the same way, that `magic-numbers-go` records for `mnd`, and it is the
same pinned tool.

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

`allow-serial-runners` makes a second instance WAIT for the lock. `golangci-lint`
takes one file lock for each run, and by default a second instance stops with
`Error: parallel golangci-lint is running` on stderr and writes nothing to
stdout. The script drops stderr, so that run would read as a clean file rather
than as a failure. The lock stands in the cache directory, which the line above
names for the workspace, so the runs of ONE workspace share it — and this set
ships two rules that drive golangci-lint over one workspace. Measured over one
workspace holding a probe module of one function above the gate, each row three
rounds from a cold cache and three rounds from a warm one:

| runs started together | runs that reported nothing |
|---|---|
| four, without the key | 0 of 4, in each of the six rounds |
| eight, without the key | 3 of 8, in each of the six rounds |
| eight, with the key | 0 of 8, in each of the six rounds |

Four runs never clash, so the probe needs eight. The same eight runs with stderr
kept named the reason on each run that reported nothing:
`Error: parallel golangci-lint is running`.

`GOLANGCI_LINT_CACHE` gives each workspace its own cache directory, named for the
workspace path. The shared cache stores a finding with the ABSOLUTE path the run
that first cached it read, and it answers by package content, so a second
workspace holding the same bytes under the same module name gets the FIRST
workspace's paths back. Two checkouts of one repository are the everyday form of
this, and a review runs in a worktree. Measured over two directories holding the
same probe module, one shared cache:

| the run | what it reported |
|---|---|
| the first directory | its own plain file |
| the second directory | the FIRST directory's plain file |
| the second directory, the first one removed | the FIRST directory's plain file AND its generated file |
| each directory, a cache of its own | its own plain file |

Row 2 is the silence: the engine drops a finding it cannot place in the
workspace, so the rule reports nothing and names no reason. Row 3 is worse than
silence — it is a WRONG finding on generated code. `linters.exclusions.generated`
reads the head of the file at the reported path, and a stale path names a file
that is no longer there, so the filter lets the finding through. The carve-out
fails OPEN. The acceptance test
`the_shipped_go_function_length_tool_rule_reads_the_workspace_it_ran_in` drives
the generated-code probe over two workspaces for that reason, and
`magic-numbers-go` records the same measurement for `mnd`.

The scope is `workspace` because golangci-lint loads packages, not loose files,
and `./...` loads the whole module. The engine keeps only the findings in the
changed files.

Selection in the pipe is attribution, not exemption. `golangci-lint` also emits
`typecheck` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. Every exemption this rule makes stands in
the configuration, where golangci-lint decides it.

## The temporary directory the configuration stands in

The script names two directories under `TMPDIR`, and each has an owner. The
golangci-lint cache is named after the working directory and stands between runs
on purpose. The configuration directory `mktemp -d` makes is the run's own, and
`trap 'rm -rf "$work"' EXIT` removes it. The scope is `workspace`, so this
script takes no file argument.

Measured over a Go module of one file: the first run raised the count of entries
under `TMPDIR` by 2 before the trap, one for the cache and one for the
configuration, and each run after it raised the count by 1. After the trap the
configuration is gone and the cache stays.

## The carve-outs the prompt rule states

`function-length` exempts four shapes: a test, generated code, a function that
is mostly configuration or data, and an initialization function that sets many
fields. The run reproduces all four.

### Configuration, data and an initializer, which the gate drops

`function-length` exempts "Functions that are mostly configuration/data (e.g.,
builder patterns with many options)" and "Initialization functions that set many
fields". A composite literal is ONE statement however many rows it holds, and a
builder chain is one statement however many options it sets, so the statement
gate drops both without reading a name or a path.

Measured with golangci-lint 2.12.2 on a probe module, at the gate of 160:

| the shape | funlen lines | funlen statements | the run |
|---|---|---|---|
| a procedure of 170 assignments | 170 | 170 | reports |
| a composite literal of 300 rows | 302 | 1 | silent |
| a builder chain of 300 options | 301 | 1 | silent |
| a table-driven test of 300 rows | 307 | 4 | silent |

The acceptance test
`the_shipped_go_function_length_tool_rule_measures_statements_and_not_lines`
holds the first three rows. An initializer that sets many fields is the same
shape: each assignment is a statement, so an initializer of more than 160 fields
reports and the author answers it with the annotation below.

### A test, which the run drops by the DEFINITION

`function-length` exempts "Functions explicitly marked as tests", and
`cognitive-complexity` names the mark for the whole set: "Identify a test from
its attribute or framework naming convention at the **definition**, never from
the file name. A complex helper named `build_request` in a file called
`foo_test.rs` is still a complex function and is still listed."

Go states that convention in `go test`: a test is a function named `TestXxx`,
`BenchmarkXxx`, `FuzzXxx` or `ExampleXxx`, where the rune after the prefix is
not a lower-case letter, in a file whose name ends `_test.go`. Both halves are
the definition, and `go test` requires both.

funlen writes the function's own NAME into every message it reports — `Function
'TestFoo' has too many statements (170 > 160)` — so a `linters.exclusions.rules`
entry that reads `text` reads the definition. The entry names both halves of the
convention and names `funlen` alone, so it can silence no other linter.

The statement gate already drops the table-driven test, which is the shape that
would otherwise make a suppression mandatory on idiomatic code: measured, 111 of
the 142 `_test.go` findings the old line gate raised hold 20 statements or
fewer. The exclusion covers the remainder. Over the corpus, at the gate of 160:

| the run | findings | in `_test.go` |
|---|---|---|
| no test carve-out | 12 | 4 |
| the shipped exclusion | 8 | 0 |

The four it drops are named test functions of 191 to 258 statements. A path
exclusion alone would have dropped 142 of the old gate's 161 findings, 11 of
them helpers in test files that the prompt rule still lists — the trade
`complexity-go` refuses.

Measured with golangci-lint 2.12.2 on a probe module, at the gate of 160, over
one function of 170 statements in each shape:

| the declaration | the run |
|---|---|
| `TestDense` in a `_test.go` file | silent |
| `buildRequest` in the same `_test.go` file | reports |
| `Testify` in a `_test.go` file | reports |
| `TestLooking` outside a `_test.go` file | reports |

Row 2 is the helper the prompt rule keeps. Row 3 is the lower-case rune after
the prefix, which `go test` refuses as a test and the `[^\p{Ll}]` class refuses
here. Row 4 is the file-name half. The acceptance test
`the_shipped_go_function_length_tool_rule_reads_a_test_from_its_definition`
holds rows 1 and 2 beside a table-driven test.

The expression stands in a double-quoted YAML scalar, so the class is written
`[^\\p{Ll}]`: YAML reads `\\` as one backslash and hands `[^\p{Ll}]` to Go's
regexp engine. `\p` is not a YAML escape, so the single-backslash spelling is a
parse failure rather than a different match.

### Generated code, which golangci-lint drops for itself

`linters.exclusions.generated` defaults to `lax`, which drops every finding in a
file whose head carries the line `go generate` states —
`^// Code generated .* DO NOT EDIT\.$` above the first text that is neither a
comment nor blank. The rule states no `generated` key, so it takes that default.
A `rules` list does not replace it: measured over two files that hold the same
function of 170 statements, one under the header and one without it, the run
reports the plain file alone. The acceptance test
`the_shipped_go_function_length_tool_rule_skips_a_generated_file` holds both
positions, and `complexity-go` records the same measurement for the three Go
rules of this set.

An author cannot answer this carve-out with the annotation below. The generator
writes the file again, and the annotation goes away each time. That is why the
run makes the test and the author does not.

## The annotation an author writes

To exempt one function, write `//nolint:funlen // <reason>` on it in the code.
Measured with golangci-lint 2.12.2 over one function of 170 statements against
the gate of 160: the annotation directly above the `func` line gives no finding.

The first fix a finding asks for is still to split the function. The annotation
is the second fix, and the reason beside it states why.
