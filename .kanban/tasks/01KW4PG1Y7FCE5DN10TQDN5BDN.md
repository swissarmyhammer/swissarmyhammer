---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4rs4tacnvgdfy5sf5dj14g
  text: |-
    Implemented: raised review batching default batch_size 32 KB (32768) → 128 KB (131072). The oversize-error path is unchanged; only the default threshold moved.

    TDD: added `default_batch_size_is_128_kib` test in fleet.rs first, watched it fail RED (left: 32768, right: 131072), then changed the const → GREEN.

    Files changed:
    - crates/swissarmyhammer-validators/src/review/fleet.rs — DEFAULT_BATCH_SIZE = 128 * 1024 (was 32 * 1024); updated doc comment to 128 KiB + rationale (clears ~95 KB largest single file; 32 KiB was smaller than many real source files). Added config test asserting the default == 131072 == FleetConfig::default().batch_size.
    - crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs — BATCH_SIZE_PARAM description "default 32768 = 32 KiB" → "default 131072 = 128 KiB".
    - crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs — two doc/comment refs "(32 KiB)" → "(128 KiB)".
    - builtin/skills/check-sah/SKILL.md — "content-batched by batch_size (default 32 KB)" → "default 128 KB".

    review/SKILL.md and review/description.md do not cite the default (no change needed).

    Grep verification:
    - `rg 32768 crates` (excl .kanban): only op_tool_helpers.rs usize_arg parsing test (value "n":32768, unrelated to batch size). No remaining batch-size-default refs.
    - `rg -i '32 ?KB' builtin/skills builtin/validators crates/.../review`: no matches.
    - scope.rs tests pass `32 * 1024` as an explicit batch_work_list arg (a chosen budget, not the default) — left as-is.

    Tests: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools` → 1700 passed, 0 failed, 0 skipped. `cargo fmt` clean. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` → exit 0, no warnings.

    Left in doing for /review. Nothing blocked. Not committed (working tree may carry unrelated parallel changes).
  timestamp: 2026-06-27T14:47:15.530178+00:00
- actor: claude-code
  id: 01kw4s54y3q0nxgv1ymg28wjav
  text: |-
    BLOCKED on stale running engine — not a code problem. Implemented + committed (9e83139c4): DEFAULT_BATCH_SIZE 32768→131072, param/doc/skill updates; cargo nextest -p swissarmyhammer-validators -p swissarmyhammer-tools = 1700 passed; fmt+clippy clean.

    The /review gate cannot run against the current binary: reviewing this commit (touches fleet.rs, 96271 bytes) requires batch_size > 96K, but the running engine IGNORES the batch_size override. Proven: called `review sha HEAD~1..HEAD batch_size=200000` directly → still errored "over the 32768-byte review batch_size". Verified the source threads the override correctly end-to-end (mod.rs:329 usize_arg → review_op.rs:251 FleetConfig{batch_size: request.batch_size...} → synthesize.rs:284 batch_work_list(&work, fleet_config.batch_size)), and b4bac5136 shipped that same wiring. So the running binary is stale: no binary newer than 13:52 UTC exists on disk, and `/mcp reconnect` re-attaches to the existing server process rather than restarting it with a fresh build.

    UNBLOCK: rebuild from current HEAD (cargo build --release / install) AND fully restart the sah MCP server process (not just /mcp reconnect). Then default batch_size is 128K, every file in the eccd288dc^..HEAD range is <128K, and the review batches+completes with no override needed — including a clean re-review of this very commit. Left in `doing` pending that.
  timestamp: 2026-06-27T14:53:48.867946+00:00
- actor: claude-code
  id: 01kw4w4g7dmkc7657h24ajeany
  text: 'UNBLOCKED + acceptance verified live. After a real rebuild (binary refreshed to 09:56 local) + server restart, the headline check passed: `review sha eccd288dc^..HEAD` (the 114-file range that previously failed 15/15) now COMPLETES with the default 128 KB batch_size — attempted: 112, failed: 0, 26 findings. Batching split the range into multiple batches and every fan-out task succeeded; no oversize error on any file (all < 128 KB). Default batch_size override is honored now that the binary is current. Marking done — code committed (9e83139c4), tests green (1700), acceptance empirically confirmed.'
  timestamp: 2026-06-27T15:45:53.390012+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffeb80
project: local-review
title: 'Review batching: raise default batch_size 32 KB → 128 KB'
---
## Why
The batching default of 32 KB is smaller than a large fraction of real source files, so a default review of an ordinary commit hits the oversize-file hard error instead of reviewing. Demonstrated live (rebuilt engine):

`review sha eccd288dc^..HEAD` with default batch_size →
`file crates/swissarmyhammer-kanban/src/board/init.rs inlines 33931 bytes, over the 32768-byte review batch_size`

Many changed files in a normal engine diff exceed 32 KB: fleet.rs 95 KB, scope.rs 66 KB, drive.rs 53 KB, types.rs 45 KB, parser.rs 42 KB, verify.rs 42 KB, test_support.rs 42 KB, loader.rs 38 KB, synthesize.rs 34 KB, board/init.rs 34 KB, tests.rs 32.6 KB. With a 32 KB default, `/finish`'s per-commit `HEAD~1..HEAD` review would ERROR on any commit touching one of these — too aggressive.

The oversize-error path itself is correct and validated (it names file/size/limit/fix); the issue is purely the DEFAULT value.

## Change
Raise the default `batch_size` from 32768 (32 KB) to **131072 (128 KB)**. 128 KB clears the largest single file in a typical change (~95 KB fleet.rs) so normal commits review without erroring, while a genuinely large multi-file diff (this range totals ~1 MB across ~30 files) still splits into multiple batches.

## Blast radius — update every place the default 32 KB appears
- `DEFAULT_BATCH_SIZE` const (crates/swissarmyhammer-validators/src/review/fleet.rs) → 131072.
- The `review` tool `batch_size` param default / schema doc (swissarmyhammer-tools/src/mcp/tools/review/{mod.rs,review_op.rs}) — wherever 32768 / "32 KB" is stated as the default.
- Any test asserting the default equals 32768 (grep `32768` and `DEFAULT_BATCH_SIZE` across crates) → update expectations.
- `builtin/skills/check-sah/SKILL.md` — just edited; it says "content-batched by `batch_size` (default 32 KB)" → change to 128 KB.
- `builtin/skills/review/SKILL.md` and the review tool `description.md` if they cite the default.
- The `s5g44e2` card text says "default 32 KB" — leave the card (history) but don't propagate the stale number into any live doc.

## Acceptance
- Default `review sha eccd288dc^..HEAD` (every changed file < 128 KB) batches and COMPLETES (findings or clean) instead of oversize-erroring.
- A file > 128 KB still hard-errors with the clear message (the path is unchanged, only the threshold moved).
- `cargo nextest -p swissarmyhammer-validators -p swissarmyhammer-tools` green; `grep -rn 32768` shows no remaining default-batch_size references; docs say 128 KB.