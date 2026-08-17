---
assignees:
- claude-code
position_column: todo
position_ordinal: fff180
title: 'magic-numbers-go is a bare pipe: one Go file that does not parse reads as a clean tree'
---
`builtin/validators/code-hygiene/rules/magic-numbers-go.md` drives golangci-lint
through the same shape `function-length-go` carried before `^rfjsx87`:

    golangci-lint run --config "$config" --path-mode abs --show-stats=false \
      --output.json.path stdout ./... 2>/dev/null |
      jq -c '(.Issues // [])[] | select(.FromLinter == "mnd") | ...'

The pipe ends in `jq`, so every golangci-lint status reads as 0, and
`2>/dev/null` drops the one channel golangci-lint states a declined item on.

Measured with golangci-lint 2.12.2 while implementing `^rfjsx87`, over the
shipped command line:

- One Go file that does not parse gives a `typecheck` row, and the
  `invalid_issue` processor then answers with the typecheck rows ALONE:
  `if len(tcIssues) > 0 { return tcIssues, nil }`. So the run reports no `mnd`
  row for any file, at exit 1, with 0 bytes on stderr.
- A workspace holding no `go.mod` exits 7 with a report of `Issues: []`.
- A module holding no `.go` file exits 5 with 0 bytes of stdout.
- A `--config` golangci-lint cannot read exits 3 with 0 bytes of stdout.
- Eight runs started together without `allow-serial-runners` stop three of the
  eight at exit 3 with `Error: parallel golangci-lint is running`.
- A `.go` file nobody may read writes
  `level=error msg="[linters_context] typechecking error: open <path>: permission denied"`
  to stderr. A run whose cache holds no answer for the other packages then
  reports nothing at exit 7; a run whose cache does hold one reports those
  findings at exit 1.

The work: give `magic-numbers-go` the shape `function-length-go` now carries —
run the tool into a file, gate the status against 0 and 1, break the run for a
report row of another linter, and state each stderr line under
`sah-diagnostic:` at exit 0. Add the acceptance tests beside the ones
`function_length_go.rs` holds.

Found while implementing `^rfjsx87`. #tool-validators #objectivity