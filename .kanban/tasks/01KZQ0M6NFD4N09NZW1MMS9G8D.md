---
assignees:
- claude-code
position_column: todo
position_ordinal: ffcd80
title: function-length-go silently reports nothing on a lock clash or a shared golangci-lint cache
---
`builtin/validators/code-hygiene/rules/function-length-go.md` runs `funlen` through `golangci-lint` with the same script shape `magic-numbers-go` had before ^s2ftjys. Two defects were measured on that shape, and both make the rule report ZERO findings and name no reason:

1. **The lock.** `golangci-lint` takes one file lock for each run. A second instance stops with `Error: parallel golangci-lint is running` on stderr and writes nothing to stdout. The script sends stderr to `/dev/null`, so the review reads a clean file. Measured: eight runs of the `magic-numbers-go` script started together reported `6, 6, 6, 0, 6, 0, 6, 0`.
   Fix: `run: allow-serial-runners: true` in the written config. With the key, all eight reported `6`.

2. **The shared cache.** `golangci-lint` answers by package content and stores each finding with the ABSOLUTE path of the run that first cached it. A second workspace that holds the same bytes under the same module name gets the FIRST workspace's paths back, and the engine drops a finding it cannot place in the workspace. Measured: a run in directory B reported `/private/var/.../T/.tmpuFxLbQ/src/magic_numbers_go_fail.go`, a path outside itself. Two checkouts of one repository are the everyday form of this, and a review runs in a worktree.
   Fix: name a cache directory for the workspace —
   `cache="${TMPDIR:-/tmp}/sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')"` and run with `GOLANGCI_LINT_CACHE="$cache"`. With a cache of its own the same run reported its own path.

`^s2ftjys` made both changes in `magic-numbers-go.md` and states each with its measurement. Copy the two lines and the two paragraphs, and measure both again for `funlen`.

Also measure `unused-code-go`, which runs `staticcheck` at workspace scope. `staticcheck` keeps its own cache; find out whether it stores absolute paths the same way, and state the answer in that rule.

Found while ^s2ftjys added an end-to-end acceptance test for `magic-numbers-go`. The test failed for both reasons, and it passes only after both fixes. #tool-validators #objectivity