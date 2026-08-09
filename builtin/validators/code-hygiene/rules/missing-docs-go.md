---
name: missing-docs-go
description: Exported Go items need doc comments — checked by revive, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: missing-docs
tool:
  scope: files
  run: |
    config="$(mktemp -d)/revive.toml"
    printf '%s\n' '[rule.exported]' > "$config"
    revive -config "$config" -formatter json "$@" |
      jq -c '(. // [])[] | select(.RuleName == "exported")
             | {file: .Position.Start.Filename, line: .Position.Start.Line, message: .Failure}'
  doctor:
    check_command: "which revive jq"
    check_version_command: "revive -version"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/mgechev/revive@v1.15.0'
---

# Missing Documentation — Go

`revive` reports every exported function, type, constant, and variable without
a doc comment. The `exported` rule names that check.

The script writes its own `revive.toml` to a temporary path and passes it with
`-config`. The config holds one line, `[rule.exported]`, which both turns the
rule on and turns every other revive rule off, so the rule owns its whole
invocation and never reads the project's own revive configuration.

The scope is `files` because revive reads the files it is given. It needs no
`go.mod` to lint a loose file.

`-formatter json` prints `null`, not an empty array, for a file with no
findings, so the pipe starts with `(. // [])[]`.

Selection in the pipe is attribution, not exemption: to exempt one item, write
`//revive:disable-next-line:exported` above it in the code.
