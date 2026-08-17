---
name: magic-numbers-go
description: Unnamed Go literals need constants — checked by mnd, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: magic-numbers
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
    cat > "$config" <<'MND_CONFIG'
    version: "2"
    run:
      allow-serial-runners: true
    linters:
      default: none
      enable:
        - mnd
      settings:
        mnd:
          ignored-numbers: ["0", "1", "-1", "100"]
    issues:
      max-issues-per-linter: 0
      max-same-issues: 0
    MND_CONFIG
    status=0
    GOLANGCI_LINT_CACHE="$cache" \
    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... > "$work/report.json" 2> "$work/lint.err" || status=$?
    if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
      cat "$work/lint.err" "$work/report.json" >&2
      printf 'magic-numbers-go: golangci-lint exited %s and measured no literal\n' "$status" >&2
      exit 1
    fi
    filtered=0
    jq -r '(.Issues // [])[] | select(.FromLinter != "mnd")
           | "\(.Pos.Filename):\(.Pos.Line): \(.FromLinter) \(.Text)"' "$work/report.json" \
      > "$work/unmeasured.txt" || filtered=$?
    jq -c '(.Issues // [])[] | select(.FromLinter == "mnd")
           | {file: .Pos.Filename, line: .Pos.Line, message: .Text}' "$work/report.json" \
      > "$work/reported.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'magic-numbers-go: jq could not read the golangci-lint report\n' >&2
      exit 1
    fi
    if [ -s "$work/unmeasured.txt" ]; then
      cat "$work/lint.err" "$work/unmeasured.txt" >&2
      printf 'magic-numbers-go: golangci-lint reported a row of another linter, which drops every mnd row of the same run\n' >&2
      exit 1
    fi
    while IFS= read -r line || [ -n "$line" ]; do
      printf 'sah-diagnostic: golangci-lint declined an item and said: %s\n' "$line" >&2
    done < "$work/lint.err"
    cat "$work/reported.json"
  doctor:
    check_command: "which golangci-lint go jq cat mktemp shasum mkdir touch find rm"
    check_version_command: "golangci-lint --version"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@v2.12.2'
---

# Magic Numbers — Go

`mnd` reports every unnamed numeric literal. It checks six positions —
`argument`, `case`, `condition`, `operation`, `return`, and `assign` — and those
six are its default set, which this rule keeps. Measured against a probe package
holding one literal of each kind, it reported the comparison, the condition, the
switch case, the operation, the call argument, and the bare return, and left the
`const` declaration and the `:=` binding alone. A declaration names its value, so
those two are the `magic-numbers` prompt carve-out and the tool already honors it.

`ignored-numbers` is the one threshold the rule sets. `mnd` ignores nothing by
default, so the config states `["0", "1", "-1", "100"]` — the value list the
prompt rule carves out. The key takes strings, not bare numbers. `0`, `1` and
`-1` are the first half of the list, and `100` is the percent half of
"conventional values". Measured: with `["0", "1", "-1"]` each of
`scaled := part * 100`, `return part * 100`, `schedule(part * 100)` and
`usage == 100` reported `Magic number: 100`; with `100` in the list all four are
silent, and `size * 4096` still reports.

## The shift carve-out cannot be expressed

The prompt rule names two conventional values, and this rule restores one of
them. `100` for percent is a VALUE, so `ignored-numbers` states it. A `<< 8` is
a POSITION — the operand of a shift — and `ignored-numbers` selects a value and
never a position.

Measured on the same probe: `word << 8`, `word >> 8` and `status == 8` each
reported `Magic number: 8`, and `8` added to `ignored-numbers` silenced all
three. A list that carried `8` would therefore drop a genuine `status == 8` to
keep the shift silent, which trades a real finding for a carve-out.

`checks` does not answer it either. `mnd` attributes a literal to the check of
the statement that holds it, so a shift inside a `return` is a `<return>`
finding and a shift inside a call is an `<argument>` finding. Measured:
`checks` without `operation` silenced `packed := word << 8`, and
`return word << 8` and `schedule(word << 8)` both still reported. The drop
therefore does not buy the carve-out, and it loses `n * 3600`, which is a real
finding.

No other setting names a shift. `golangci-lint config verify` accepts exactly
four keys under `linters.settings.mnd` — `checks`, `ignored-numbers`,
`ignored-files` and `ignored-functions` — and answers an added `ignored-shifts`
with `additional properties 'ignored-shifts' not allowed`.

So a shift operand REPORTS, and the recourse is the inline suppression at the
end of this file: write `//nolint:mnd // <reason>` on the shift. The fail
fixture carries `word << 8` for that reason, and the acceptance test
`the_shipped_go_magic_numbers_tool_rule_reports_every_fail_fixture_value` holds
`mnd` to reporting it, so the gap stays measured.

## Why golangci-lint runs the lint

`mnd` ships a standalone binary, and that binary cannot report on a current Go
toolchain. `go install github.com/tommy-muehle/go-mnd/v2/cmd/mnd@v2.5.1` builds,
then fails on every input with `panic: runtime error: invalid memory address or
nil pointer dereference` inside `go/types.(*StdSizes).Sizeof`. It requires
`golang.org/x/tools` at `v0.0.0-20200329025819`, and that copy hands `go/types` a
nil `Sizes` value the current standard library no longer defaults. Rebuilding the
same source against a current `x/tools` makes it work, which names the cause — but
`v2.5.1` is its newest tag and its default branch resolves to the same commit, so
no pinned install command can produce a working binary.

`golangci-lint` carries the same `mnd` analyzer, is maintained, and runs on the
current toolchain. `default: none` with `enable: [mnd]` turns every other linter
off, so the rule owns its whole invocation.

## Why the run is shaped this way

The script writes its own configuration to a temporary path and passes it with
`--config`, so the rule never reads the project's own `.golangci.yml`.

`max-issues-per-linter` and `max-same-issues` are both `0`, which means no limit.
The golangci-lint defaults are 50 and 3, and the second one is the dangerous one:
it keeps three findings that share a message and silently drops the rest. On a
probe file holding eight identical comparisons the default reported three.

`--path-mode abs` makes every reported path absolute. Without it golangci-lint
reports a path relative to the configuration file's own directory, which is a
temporary directory, so every path would point outside the workspace.

`allow-serial-runners` makes a second instance WAIT for the lock. `golangci-lint`
takes one file lock for each run, and by default a second instance stops with
`Error: parallel golangci-lint is running` on stderr, writes nothing to stdout,
and exits 3. The earlier shape of this run dropped stderr and ended in `jq`, so
that run read as a clean file rather than as a failure; the status gate below
now breaks it, and the key is what keeps the run measuring at all.

Measured with golangci-lint 2.12.2 over one workspace holding one unnamed
literal, eight runs of the shipped script started together, three rounds:

| the run | rounds | runs that reported the row | runs that reported nothing |
|---|---|---|---|
| without the key | 3 | 5 of 8, in each round | 3 of 8, in each round |
| with the key | 3 | 8 of 8, in each round | 0 of 8, in each round |

Each of the three that reported nothing carried
`Error: parallel golangci-lint is running` on stderr, wrote 0 bytes of report,
and made the script say `golangci-lint exited 3 and measured no literal`.

`GOLANGCI_LINT_CACHE` gives each workspace its own cache directory, named for the
workspace path. The shared cache stores a finding with the ABSOLUTE path the run
that first cached it read, and it answers by package content, so a second
workspace holding the same bytes under the same module name gets the FIRST
workspace's paths back. Measured on two directories holding the same fixture:
the second run reported `/private/var/.../T/.tmpuFxLbQ/src/magic_numbers_go_fail.go`,
a path outside itself, and the engine drops a finding it cannot place in the
workspace — so the rule reports nothing and names no reason. With a cache of its
own the same run reported its own path. Two checkouts of one repository are the
everyday form of this, and a review runs in a worktree.

### The name separates every workspace, which a checksum did not

Two workspaces reach one cache directory when they reach one NAME, and the
paragraph above is what happens then. So the name is a sha-256 digest of `$PWD`,
written whole. The name was `printf '%s' "$PWD" | cksum | tr -dc '0-9'` before,
which is a 32-bit checksum with the byte COUNT glued on after it; every
temporary workspace of one test run holds the same number of bytes, so the whole
keyspace was the checksum. `function-length-go` records the measurement and the
acceptance test that holds it.

The digest is taken of `$PWD` and of nothing else, so it is not a nonce: this
rule and `function-length-go` reach the SAME directory for one workspace, which
is what makes them share one lock.

The scope is `workspace` because `mnd` needs a loaded package to read, and
`./...` loads the whole module. The engine keeps only the findings in the changed
files.

Selection in the filter is attribution, not exemption. The filter keeps the
`mnd` rows as findings, and it reads a row of any other linter as a broken run
rather than dropping it — the section "A run that measured no literal" below
states why. Every exemption this rule makes stands in the configuration, where
golangci-lint decides it. To exempt one literal in the code, write
`//nolint:mnd // <reason>` on it.

## A run that measured no literal

The script reads golangci-lint's own status and its own report, because each one
carries a shape a pipe ending in `jq` reads as a clean tree. Measured with
golangci-lint 2.12.2 against the shipped command line, over a probe module
holding one `return status == 404`:

| the run | exit | the report | stderr |
|---|---|---|---|
| every literal in the ignored list | 0 | `Issues: []` | 0 bytes |
| one literal outside it | 1 | one `mnd` row | 0 bytes |
| a `.go` file that does not parse | 1 | one `typecheck` row ALONE | 0 bytes |
| a workspace whose one package nobody may read | 7 | `Issues: []` | one `level=error` line |
| the same file beside a package the run DID measure | 1 | the `mnd` row | one `level=error` line |
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

### A file that does not parse, which costs the WHOLE run

golangci-lint reports a file it cannot parse as a `typecheck` row. Its
`invalid_issue` processor then answers with the typecheck rows ALONE —
`if len(tcIssues) > 0 { return tcIssues, nil }` — so one Go file that does not
parse drops every `mnd` row of the same run.

Measured with golangci-lint 2.12.2 over a probe module holding the literal in
one package and a file whose call never closes in another: the report carried
the `typecheck` row and no `mnd` row, at exit 1 and 0 bytes of stderr. The same
probe with the second package removed reported the `mnd` row.

A row of another linter is therefore a run that measured no literal, and the
script exits 1 naming that row. `sah-diagnostic:` is the answer for a declined
ITEM of a sound run, and this run is not sound: nothing it measured reached the
report. The acceptance test
`the_shipped_go_magic_numbers_tool_rule_breaks_on_a_file_it_cannot_parse` stages
the literal beside a file that does not parse, and holds the run to breaking, to
naming the file, and to placing no finding.

The configuration enables one linter, and two can write a row. Measured on the
same report: `Report.Linters` carries 121 entries, and `Enabled: true` stands on
`mnd` and on `typecheck` and on no other. So the filter reads any row that is
not `mnd` as this same broken run, rather than naming `typecheck` and staying
silent for a row this rule never met.

### A file it cannot read, which costs the run the package it never read

golangci-lint refuses a `.go` file it cannot open another way. It writes
`level=error msg="[linters_context] typechecking error: open <path>: permission
denied"` to stderr, and it writes no row for that file. A workspace whose ONE
package is that package therefore measures nothing at all: `Issues: []` at exit
7, which the status gate above breaks.

A run whose cache holds the answer for the OTHER packages reports those findings
beside the same stderr line, at exit 1. Measured over one workspace linted with
the file readable and then unreadable, over the cache the first run filled: the
`mnd` row at exit 1, with the `[linters_context]` line on stderr. That run judged
the code and could not judge ONE item, so the script states the item and exits 0.
Which packages a COLD run measures is a race inside golangci-lint, and
`function-length-go` records that measurement whole over the same command line.

The script writes each line golangci-lint put on stderr under the marker
`builtin/validators/README.md` states, at exit 0:

    sah-diagnostic: golangci-lint declined an item and said: level=error msg="[linters_context] typechecking error: open <repo>/noread/unreadable.go: permission denied"

The whole line is forwarded, and no head is read or stripped. golangci-lint
writes a LOG on that channel rather than a decline channel of its own, so a head
written into this rule would answer for the one shape it was written for and
stay silent for every other. That is the lesson `missing-docs-python` records
for ruff's own stderr.

A sound run writes 0 bytes there. Measured against the shipped command line: a
module whose every literal stands in the ignored list, a module with one finding,
and a module holding a file that does not parse each wrote 0 bytes to stderr.

Two acceptance tests hold the two shapes, and each one stages the workspace that
makes its shape deterministic.
`the_shipped_go_magic_numbers_tool_rule_breaks_on_a_file_it_may_not_read` stages
a workspace whose ONE package is the package nobody may read, and holds the run
to breaking and to placing no finding.
`the_shipped_go_magic_numbers_tool_rule_declines_a_file_it_may_not_read` holds
the other shape: it hands the script the bytes the warm run answered with, and
holds the run to reporting the finding AND to stating one diagnostic that names
the file.

## The two directories under `TMPDIR`

The script names two directories under `TMPDIR`, and each has an owner. The
configuration directory `mktemp -d` makes is the run's own, and
`trap 'rm -rf "$work"' EXIT` removes it. The cache stands under
`$TMPDIR/sah-golangci-lint`, which is one entry however many workspaces the
machine holds. The scope is `workspace`, so this script takes no file argument.

The golangci-lint cache is named after the working directory and stands between
runs on purpose. The LOCK stands INSIDE that directory, so a directory of its
own for each run would give each run a lock of its own and no run would wait for
another — the `allow-serial-runners` measurement above is what that costs. The
reason still holds, so the cache stays and the run that made it never removes
it.

Measured over a Go module of one file: the first run raised the count of
entries under `TMPDIR` by 2 before the trap, one for the cache and one for
the configuration, and each run after it raised the count by 1. After the
trap the configuration is gone and the cache stays.

### The sweep, which is what a cache nobody removes needs

A run over a temporary workspace leaves one directory behind, and that workspace
never runs again. Measured on one machine on 2026-08-16: 6609 such directories,
422804 KiB. golangci-lint states the lifetime itself —
`internal/go/cache/cache.go` sets `trimLimit = 5 * 24 * time.Hour` — and it
applies that limit INSIDE a cache directory it is given, never to a directory
nobody names again.

So the script sweeps the directories past that limit, and no others:

    stale_days=5
    find "$caches" -mindepth 1 -maxdepth 1 -type d -mtime "+$stale_days" -exec rm -rf {} + 2>/dev/null || true

`touch "$cache"` runs first, so the age the sweep reads is the last RUN of that
workspace, and `-mindepth 1` keeps `$caches` itself out of the sweep. The caches
stand under one directory of their own so that the sweep reads their entries and
not every entry of `TMPDIR`: measured, 2.58 s over a `TMPDIR` of 324623 entries
against 0.01 s over 7000 caches under their own parent.

`function-length-go` records the whole measurement, and the two rules sweep the
same directories because they name them the same way.
