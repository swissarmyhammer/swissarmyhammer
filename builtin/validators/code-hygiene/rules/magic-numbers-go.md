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
    cache="${TMPDIR:-/tmp}/sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')"
    config="$(mktemp -d)/golangci.yml"
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
    GOLANGCI_LINT_CACHE="$cache" \
    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... 2>/dev/null |
      jq -c '(.Issues // [])[] | select(.FromLinter == "mnd")
             | {file: .Pos.Filename, line: .Pos.Line, message: .Text}'
  doctor:
    check_command: "which golangci-lint go jq"
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
`Error: parallel golangci-lint is running` on stderr and writes nothing to
stdout. The script drops stderr, so that run would read as a clean file rather
than as a failure. Measured: eight runs of this script started together reported
`6, 6, 6, 0, 6, 0, 6, 0`, and the three that reported nothing carried the lock
error; with the key, all eight reported `6`.

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

The scope is `workspace` because `mnd` needs a loaded package to read, and
`./...` loads the whole module. The engine keeps only the findings in the changed
files.

Selection in the pipe is attribution, not exemption. `golangci-lint` also emits
`typecheck` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. To exempt one literal, write
`//nolint:mnd // <reason>` on it in the code.
