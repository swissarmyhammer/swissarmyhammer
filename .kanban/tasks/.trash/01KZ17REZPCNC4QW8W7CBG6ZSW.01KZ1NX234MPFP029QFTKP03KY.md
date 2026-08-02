---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
title: Review engine fails 128/128 review tasks on large commit ranges (71-file diff)
---
`review sha 0c8b969b8~1..0c8b969b8` (71 files changed, 570(+)/414(-)) fails every review task, three separate times, across two sessions and after a fresh `sah` rebuild that ruled out the stale-binary theory:

- Attempt 1 (pre-rebuild): 128/128 failed, `{findings:0, confirmed:0, refuted:0, attempted:128, failed:128, skipped:0}`, tried 3x on the `session` backend and 1x on `local` — identical every time.
- Attempt 2 (post-rebuild, fresh `sah`): 128/128 failed again, and a narrowed retry with `validators: ["rust"]` still failed at exactly 128 — meaning the failure happens before per-validator dispatch, not inside a specific validator.

## Ruled out

- Stale binary — `sah` was freshly rebuilt today; `check validators` reports all 21 validators healthy.
- General engine outage — `review sha HEAD~1..HEAD` on the same repo state succeeded cleanly (16/16, real findings), and `review file crates/swissarmyhammer-tools/src/mcp/server.rs` with a validator filter also succeeded (16/16, real findings).
- Git/commit integrity — the commit resolves fine (`git show`, `git diff` both clean).

## Likely cause

Something in the engine's fan-out scales badly (or hard-fails) specifically for a large file-count diff — 71 files here, producing 128 review tasks (files × some validator/chunk multiplier). Smaller ranges on the same repo, same commit history, work fine.

## Reproduce

```
sah tool review review sha --sha 0c8b969b8~1..0c8b969b8
```

Expect: 128/128 failed, 0 findings, `results are INCOMPLETE`.

## Impact

Blocks `/finish` on any task whose checkpoint commit touches a large number of files — task `^p4mp9n6` (Lowercase the remaining capitalized MCP error Display messages outside the kanban tool) is stuck in `review` because of this. #bug