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
    config="$(mktemp -d)/golangci.yml"
    cat > "$config" <<'MND_CONFIG'
    version: "2"
    linters:
      default: none
      enable:
        - mnd
      settings:
        mnd:
          ignored-numbers: ["0", "1", "-1"]
    issues:
      max-issues-per-linter: 0
      max-same-issues: 0
    MND_CONFIG
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
default, so the config states `["0", "1", "-1"]` — the prompt carve-out list.

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

The scope is `workspace` because `mnd` needs a loaded package to read, and
`./...` loads the whole module. The engine keeps only the findings in the changed
files.

Selection in the pipe is attribution, not exemption. `golangci-lint` also emits
`typecheck` diagnostics on the same stream, and the `jq` filter drops them; they
belong to the build, not to this rule. To exempt one literal, write
`//nolint:mnd // <reason>` on it in the code.
