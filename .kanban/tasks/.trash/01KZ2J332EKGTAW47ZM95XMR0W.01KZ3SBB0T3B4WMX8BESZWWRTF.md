---
assignees:
- claude-code
position_column: todo
position_ordinal: e980
title: review sha hard-errors with unknown probe 'complexity' on any .rs diff; validators filter doesn't exclude it
---
# Symptom

`review sha` hard-errors on any diff scope touching `.rs` files:

```
MCP error -32603: review pipeline failed: Validator 'complexity' error: unknown probe 'complexity'; the catalog defines: callers, duplicates, similar
```

This is a hard error, not a `counts.failed` tally — the pipeline aborts during
validator/probe resolution, before any fleet task is attempted. `attempted`,
`failed`, `skipped` never populate.

Found while trying to review ^6jsxjbc and ^k5wsxh0 (both were moved to `done`
without review, by explicit user decision, because this bug made verification
impossible — do not treat that as evidence this bug is unimportant).

# What was ruled out

This looked at first like the stale-server-process bug from ^3rnvage — reviewing
`0ecaff64a~1..0ecaff64a` failed against a `sah serve` process started before
`^k5wsxh0`'s commit (`8d7d8f57dd`, which adds the `complexity` probe) existed.
`/mcp reconnect sah` was run, `sah --version` confirmed the fresh binary matches
current HEAD exactly, and the SAME error reproduced. So this is a real defect,
not stale-binary noise.

# Evidence

- `crates/swissarmyhammer-validators/src/review/probes.rs` at HEAD DOES define a
  `complexity` entry in `CATALOG` (added in `8d7d8f57dd`). The source is not
  missing it.
- The live engine's resolved catalog reports only 3 entries: `callers,
  duplicates, similar` — disagrees with the source tree at the same commit.
- Reproducible 3/3 times on `review sha 0ecaff64a~1..0ecaff64a` and on `review
  sha 8d7d8f57dd~1..8d7d8f57dd` (the commit that adds the probe) — fails
  identically on both.
- `review sha HEAD~1..HEAD` (kanban-metadata-only, no `.rs` files) succeeds with
  an empty scope. So the trigger is specifically: a diff scope containing `.rs`
  files matches the `complexity` validator by glob, and probe resolution for it
  fails.
- `review file <path>` (whole-file mode, not diff mode) succeeds normally with
  findings. The failure is specific to `review sha` (diff-scoped) resolution.

# Second defect found alongside it

Passing an explicit `validators: ["rust"]` or an explicit list that excludes
`"complexity"` by name does NOT avoid the error — `complexity` still gets
matched and probe-resolved regardless of the `validators` passthrough filter.
Investigate whether `review sha`'s validator selection ever applies the
`validators:` parameter before matching, or whether it is ignored entirely for
this code path.

# Investigate

- Where `review sha`'s validator/probe resolution differs from `review file`'s —
  they must share a probe catalog and disagree.
- Whether there are two separate `CATALOG` definitions (one used by diff-mode
  resolution, one by whole-file mode) that drifted apart, similar to the
  duplicate-frontmatter-splitter pattern found earlier today.
- Whether the `validators:` filter parameter is actually threaded into `review
  sha`'s matching logic at all.

# Acceptance

- `review sha` succeeds on a diff scope containing `.rs` files, with the
  `complexity` validator running normally (not erroring, not silently skipped).
- A test pinning that `review sha` and `review file` resolve probes from the
  SAME catalog, so they cannot drift apart again.
- Passing an explicit `validators: [...]` list to `review sha` actually
  restricts which validators run — add a test that a validator NOT in the list
  never gets matched or probe-resolved.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.

This blocks verifying any future review-engine change through `review sha`.
#review #bug