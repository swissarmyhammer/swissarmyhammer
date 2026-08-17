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
    set -e
    caches="${TMPDIR:-/tmp}/sah-golangci-lint"
    digest="$(printf '%s' "$PWD" | shasum -a 256)"
    cache="$caches/${digest%% *}"
    mkdir -p "$cache"
    touch "$cache"
    stale_days=5
    find "$caches" -mindepth 1 -maxdepth 1 -type d -mtime "+$stale_days" -exec rm -rf {} + 2>/dev/null || true
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
    status=0
    GOLANGCI_LINT_CACHE="$cache" \
    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... > "$work/report.json" 2> "$work/lint.err" || status=$?
    if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
      cat "$work/lint.err" "$work/report.json" >&2
      printf 'function-length-go: golangci-lint exited %s and measured no function\n' "$status" >&2
      exit 1
    fi
    filtered=0
    jq -r '(.Issues // [])[] | select(.FromLinter != "funlen")
           | "\(.Pos.Filename):\(.Pos.Line): \(.FromLinter) \(.Text)"' "$work/report.json" \
      > "$work/unmeasured.txt" || filtered=$?
    jq -c '(.Issues // [])[] | select(.FromLinter == "funlen")
           | {file: .Pos.Filename, line: .Pos.Line, message: .Text}' "$work/report.json" \
      > "$work/reported.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'function-length-go: jq could not read the golangci-lint report\n' >&2
      exit 1
    fi
    if [ -s "$work/unmeasured.txt" ]; then
      cat "$work/lint.err" "$work/unmeasured.txt" >&2
      printf 'function-length-go: golangci-lint reported a row of another linter, which drops every funlen row of the same run\n' >&2
      exit 1
    fi
    while IFS= read -r line || [ -n "$line" ]; do
      printf 'sah-diagnostic: golangci-lint declined an item and said: %s\n' "$line" >&2
    done < "$work/lint.err"
    cat "$work/reported.json"
  doctor:
    check_command: "which golangci-lint go jq cat mktemp shasum mkdir touch find"
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
requirement to change correct code, which is the trade this set takes wherever
a gate would otherwise make a suppression mandatory on correct code.

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
`Error: parallel golangci-lint is running` on stderr, writes nothing to stdout,
and exits 3. The earlier shape of this run dropped stderr and ended in `jq`, so
that run read as a clean file rather than as a failure; the status gate below
now breaks it, and the key is what keeps it measuring at all. The lock stands in
the cache directory, which the line above names for the workspace, so the runs
of ONE workspace share it — and this set ships two rules that drive
golangci-lint over one workspace. Measured over one
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

### The name separates every workspace, which a checksum did not

Two workspaces reach one cache directory when they reach one NAME, and the table
above is what happens then. So the name is a sha-256 digest of `$PWD`, written
whole:

    caches="${TMPDIR:-/tmp}/sah-golangci-lint"
    digest="$(printf '%s' "$PWD" | shasum -a 256)"
    cache="$caches/${digest%% *}"

The name was `printf '%s' "$PWD" | cksum | tr -dc '0-9'` before. That is a
32-bit checksum with the byte COUNT glued on after it. Every temporary workspace
of one test run holds the same number of bytes, so the count states nothing
across them and the whole keyspace is the checksum. Measured over 200000 paths
of the shape `tempfile` writes: the checksum name gave 199996 distinct
directories — 4 collisions, which is the birthday count over 2^32 — and the
digest name gave 200000.

Measured with golangci-lint 2.12.2 over two directories whose paths carry the
same checksum over the same number of bytes, run one after the other:

| the name | cache directories | what the second run reported |
|---|---|---|
| the checksum | 1 | the FIRST directory's `plain/staged.go` |
| the digest | 2 | its own `plain/staged.go` |

The digest is taken of `$PWD` and of nothing else, so it is not a nonce: the two
rules of this set that drive golangci-lint reach the SAME directory for one
workspace, which is what makes them share one lock. The acceptance test
`the_shipped_go_function_length_tool_rule_reads_the_workspace_one_checksum_merged`
stages that pair and holds each run to reporting its own position. It SEARCHES
for the pair, because the working directory is a temporary path no constant can
name.

The scope is `workspace` because golangci-lint loads packages, not loose files,
and `./...` loads the whole module. The engine keeps only the findings in the
changed files.

Selection in the filter is attribution, not exemption. The filter keeps the
`funlen` rows as findings, and it reads a row of any other linter as a broken
run rather than dropping it — the section "A run that measured no function"
below states why. Every exemption this rule makes stands in the configuration,
where golangci-lint decides it.

## The two directories under `TMPDIR`

The script names two directories under `TMPDIR`, and each has an owner. The
configuration directory `mktemp -d` makes is the run's own, and
`trap 'rm -rf "$work"' EXIT` removes it. The cache stands under
`$TMPDIR/sah-golangci-lint`, which is one entry however many workspaces the
machine holds. The scope is `workspace`, so this script takes no file argument.

The golangci-lint cache is named after the working directory and stands between
runs on purpose, and the purpose is two measured things.

- The LOCK stands INSIDE that directory. A directory of its own for each run
  would give each run a lock of its own, so no run would wait for another, and
  the `allow-serial-runners` table above is what that costs.
- A WARM cache is what makes row 3 of the unreadable-file table below
  DETERMINISTIC. A run whose cache holds the answer reports its findings and
  declines one item, every time. A run whose cache is cold answers that way only
  when a race inside golangci-lint falls its way, which the same section
  measures.

Both reasons still hold, so the cache stays and the run that made it never
removes it.

Measured over a Go module of one file: the first run raised the count of entries
under `TMPDIR` by 2 before the trap, one for the cache and one for the
configuration, and each run after it raised the count by 1. After the trap the
configuration is gone and the cache stays.

### The sweep, which is what a cache nobody removes needs

A run over a temporary workspace leaves one directory behind, and that workspace
never runs again. Measured on one machine on 2026-08-16: 6609 such directories,
422804 KiB, mean 64 KiB. None of them stood over three days old — not because
this rule removed one, but because the platform swept them. A machine that
sweeps no temporary directory grows without bound.

golangci-lint states the lifetime itself. `internal/go/cache/cache.go` sets
`trimLimit = 5 * 24 * time.Hour`, and `Trim` drops every entry whose last use
stands before `now - trimLimit - mtimeInterval`. It does that INSIDE a cache
directory it is given, and it never removes a directory nobody names again.

So the script sweeps the directories past that limit, and no others:

    stale_days=5
    find "$caches" -mindepth 1 -maxdepth 1 -type d -mtime "+$stale_days" -exec rm -rf {} + 2>/dev/null || true

`touch "$cache"` runs first, so the age the sweep reads is the last RUN of that
workspace and not whatever golangci-lint last wrote inside it. Every entry of a
directory nothing has touched for that long was itself last used at least that
long ago, so such a directory holds nothing golangci-lint would have kept. A
shorter cut would throw away entries the tool still serves, which is why the
number is the tool's own.

Measured with `find -mtime +5` over directories staged at 0, 1, 4, 5, 5.5, 6, 7
and 30 days: it removed the last three and kept the first five, so the cut is at
six days. At the measured rate of about 3300 runs a day the sweep therefore
holds the count near six days of runs, which is the price of keeping every entry
the tool would still serve.

`-mindepth 1` keeps `$caches` itself out of the sweep. Without it the parent
directory matches `-type d` as well, and a parent nothing has touched for six
days would take every cache under it.

### Why the caches stand under one directory of their own

The sweep reads a directory, so where the caches STAND decides what it costs.
The earlier shape named each cache at the top of `TMPDIR`, and `find` then had
to read every entry of `TMPDIR` to find them. Measured on one machine holding
324623 entries there:

| the sweep | time |
|---|---|
| `TMPDIR`, the caches named at the top of it | 2.58 s |
| the caches under one directory of their own, 7000 of them | 0.01 s |

A bare `ls` of that same `TMPDIR` takes 1.83 s, so the flat shape cannot be
made cheap: the cost IS reading the directory. 2.58 s stands in front of every
run of both Go rules, and it is the run's own latency rather than the tool's.

The parent directory answers the count as well. `TMPDIR` holds ONE entry for
this set however many workspaces the machine has linted, where the flat shape
held 6609.

Measured with 40 stale directories, while another process removed ten of them
under the sweep: `find` exited 0 and left none of the 40. The `2>/dev/null` and
the `|| true` answer that race. A directory that goes away under `find` is
already swept, and a sweep is housekeeping rather than measurement, so it must
never break a run that judged the code.

The acceptance test
`the_shipped_go_function_length_tool_rule_sweeps_a_stale_cache_directory` stages
one directory nothing has touched for thirty days beside one touched now, and
holds the run to removing the first and keeping the second.

## A run that measured no function

The script reads golangci-lint's own status and its own report, because each one
carries a shape a pipe ending in `jq` reads as a clean tree. Measured with
golangci-lint 2.12.2 against the shipped command line, over a probe module
holding one function of 170 statements:

| the run | exit | the report | stderr |
|---|---|---|---|
| every function under the gate | 0 | `Issues: []` | 0 bytes |
| one function over the gate | 1 | one `funlen` row | 0 bytes |
| a workspace whose one package nobody may read | 7 | `Issues: []` | one `level=error` line |
| the same file beside a package the run DID measure | 1 | the `funlen` row | one `level=error` line |
| a `.go` file that does not parse | 1 | one `typecheck` row ALONE | 0 bytes |
| a workspace holding no `go.mod` | 7 | `Issues: []` | one `level=error` line |
| a module holding no `.go` file | 5 | 0 bytes | one `level=error` line |
| a `--config` golangci-lint cannot read | 3 | 0 bytes | `Error: can't load config: ...` |
| eight runs together, no `allow-serial-runners` | 3, for three of the eight | 0 bytes | `Error: parallel golangci-lint is running` |

golangci-lint names its own statuses in `pkg/exitcodes`: 0 `Success`,
1 `IssuesFound`, 2 `WarningInTest`, 3 `Failure`, 4 `Timeout`, 5 `NoGoFiles`,
6 `NoConfigFileDetected`, 7 `ErrorWasLogged`. So 0 and 1 are the two statuses a
measured run answers with, and the script exits 1 for every other one. It
forwards golangci-lint's own stderr and its own report first, then names the
status, so a golangci-lint that refused its command line never reads as a clean
tree.

The gate costs no finding at status 7. `setupExitCode` sets that status only
while the status is still `Success`, so a run that exits 7 reported no finding
at all.

### A file that does not parse, which costs the WHOLE run

golangci-lint reports a file it cannot parse as a `typecheck` row. Its
`invalid_issue` processor then answers with the typecheck rows ALONE —
`if len(tcIssues) > 0 { return tcIssues, nil }` — so one Go file that does not
parse drops every `funlen` row of the same run.

Measured with golangci-lint 2.12.2 over a probe module holding the function of
170 statements in one package and a file whose call never closes in another: the
report carried the `typecheck` row and no `funlen` row, at exit 1 and 0 bytes of
stderr. The same probe with the second package removed reported the `funlen`
row. Four more shapes answered the same way as the first: a second sound package
beside the broken one, the broken file inside the SAME package, the broken file
as a `_test.go`, and a file whose bytes are not UTF-8, which golangci-lint
reports as `typecheck illegal UTF-8 encoding`.

A row of another linter is therefore a run that measured no function, and the
script exits 1 naming that row. `sah-diagnostic:` is the answer for a declined
ITEM of a sound run, and this run is not sound: nothing it measured reached the
report. The acceptance test
`the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_cannot_parse`
stages the function of 170 statements beside a file that does not parse, and
holds the run to breaking, to naming the file, and to placing no finding.

The configuration enables one linter, and two can write a row. Measured on the
same report: `Report.Linters` carries 121 entries, and `Enabled: true` stands on
`funlen` and on `typecheck` and on no other. So the filter reads any row that is
not `funlen` as this same broken run, rather than naming `typecheck` and staying
silent for a row this rule never met.

### A file it cannot read, which costs the run the package it never read

golangci-lint refuses a `.go` file it cannot open another way. It writes
`level=error msg="[linters_context] typechecking error: open <path>: permission
denied"` to stderr, and it writes no row for that file. A workspace whose ONE
package is that package therefore measures nothing at all: `Issues: []` at exit
7, which the status gate above breaks. Measured over such a workspace with the
cache directory made empty for every round, 20 rounds at `GOMAXPROCS=1` and 20
at `GOMAXPROCS=18`: 40 of 40 answered that way.

What the run reports for the OTHER packages of the workspace is a RACE, and the
cache is not what decides it. Measured with golangci-lint 2.12.2 over ONE
workspace holding the function of 170 statements beside a package whose one file
carries mode 000, the cache directory made empty for every round and removed
after it:

| the run | rounds | reported the `funlen` row | reported nothing |
|---|---|---|---|
| `GOMAXPROCS=1` | 40 | 25 | 15 |
| `GOMAXPROCS=2` | 40 | 0 | 40 |
| `GOMAXPROCS=18` | 40 | 0 | 40 |

A cache that is empty by construction cannot make that split.
`pkg/goanalysis/runner.go` gives every package of one run ONE context and a
`loadSem` of `runtime.GOMAXPROCS(-1)` places. In
`pkg/goanalysis/runner_checker.go` the method `(act *action) analyze` answers
`analysis skipped: IllTypedError` for the action of the package nobody may read.
In `pkg/goanalysis/runner_loadingpackage.go` the `errgroup` hands that error up,
and the method `(lp *loadingPackage) analyze` calls `cancel()` on the shared
context. A package that has not yet passed
`select { case <-ctx.Done(): return; case loadSem <- struct{}{}: }` is dropped
without a word. So which packages a run measures is a race between the failing
package and the sound ones, and heavy load on the machine moves it.

A CACHE the run can read decides it, and that is the one shape that is
deterministic. Measured over one workspace linted with the file readable and
then with the file unreadable, over the cache the first run filled, 20 rounds at
`GOMAXPROCS=1` and 20 at `GOMAXPROCS=18`: 40 of 40 reported the `funlen` row at
exit 1. `saveIssuesToCache` runs only for a run that met no error, so the
readable run is what fills that cache.

Both answers are sound for this rule, and the shipped script was driven over 24
fresh workspaces at `GOMAXPROCS=1` to hold them: 6 broke with
`golangci-lint exited 7 and measured no function`, and 18 reported the one
`funlen` row at exit 0 with one `sah-diagnostic:` line naming the file. No run
answered a third shape, and 40 of 40 runs at the default concurrency broke.

The script writes each line golangci-lint put on stderr under the marker
`builtin/validators/README.md` states, at exit 0:

    sah-diagnostic: golangci-lint declined an item and said: level=error msg="[linters_context] typechecking error: open <repo>/noread/x.go: permission denied"

The whole line is forwarded, and no head is read or stripped. golangci-lint
writes a LOG on that channel rather than a decline channel of its own, so a head
written into this rule would answer for the one shape it was written for and
stay silent for every other. That is the lesson `missing-docs-python` records
for ruff's own stderr.

A sound run writes 0 bytes there. Measured against the shipped command line: a
clean module, a module with one finding, a module holding a file that does not
parse, and eight runs started together over one workspace each wrote 0 bytes to
stderr. The one measured shape that writes at a findings status is the declined
read above. A configuration that names a deprecated linter writes `level=warning`
lines there as well — measured with `wsl` added to the shipped `enable` list,
three warning lines at exit 1 — and the shipped configuration names no
deprecated linter.

Two acceptance tests hold the two shapes, and each one stages the workspace that
makes its shape deterministic.
`the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read`
stages a workspace whose ONE package is the package nobody may read, and holds
the run to breaking and to placing no finding. It stages no second package,
because that is the shape the race above reaches: measured, a probe holding a
readable function beside the unreadable package failed 5 of 6 rounds under
`GOMAXPROCS=1`.
`the_shipped_go_function_length_tool_rule_declines_a_file_it_may_not_read` holds
the other shape: it hands the script the bytes the warm run answered with, and
holds the run to reporting the finding AND to stating one diagnostic that names
the file. It stages those bytes rather than the workspace, because a workspace
whose cache is cold reaches that answer only when the race falls its way.

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
this set names the mark: identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. A complex helper
named `build_request` in a file called `foo_test.rs` is still a long function
and is still listed.

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
`function-length-python` refuses for the same shape.

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
positions.

An author cannot answer this carve-out with the annotation below. The generator
writes the file again, and the annotation goes away each time. That is why the
run makes the test and the author does not.

## The annotation an author writes

To exempt one function, write `//nolint:funlen // <reason>` on it in the code.
Measured with golangci-lint 2.12.2 over one function of 170 statements against
the gate of 160: the annotation directly above the `func` line gives no finding.

The first fix a finding asks for is still to split the function. The annotation
is the second fix, and the reason beside it states why.
