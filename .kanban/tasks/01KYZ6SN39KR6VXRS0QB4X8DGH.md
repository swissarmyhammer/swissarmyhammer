---
assignees:
- claude-code
position_column: todo
position_ordinal: e280
title: Raise the review agent pool default worker count from 4 to 16
---
## What

The review fleet queues all its agent turns — one per validator, plus follow-up sweeps, plus one per verify finding — behind a pool of 4 workers. The MCP log for 2026-08-01 shows a 13.8-minute run with 45 agent turns through 4 workers; the validator fan-out used 11 of the 14 minutes. The turns are independent remote API calls, so the queue is pure wait time. A larger pool also puts all forks inside the 5-minute prompt-cache window of the primed prefix, which reduces cold re-uploads.

Make these changes:

- In `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:300`, raise `DEFAULT_REMOTE_WORKERS` from `4` to `16`.
- In `crates/swissarmyhammer-validators/src/validators/pool.rs:44`, raise `MAX_REMOTE_WORKERS` from `8` to `16` (`PoolConfig::remote` clamps to this cap at `pool.rs:127`).
- Keep the `review.concurrency` override behavior: when set, `pool_config_for` (`review_op.rs:287`) pins the count through `PoolConfig::with_concurrency` and disables AIMD. No change there.

Scope note: the `local` backend stays at 1 worker (`PoolConfig::local`, one model/GPU). Only the remote/Claude-API default changes.

## Subtasks

- [ ] Raise `DEFAULT_REMOTE_WORKERS` to 16 in `review_op.rs`
- [ ] Raise `MAX_REMOTE_WORKERS` to 16 in `pool.rs`
- [ ] Update the pool and review-op tests that assert the old clamp and default values

## Acceptance Criteria

- [ ] `PoolConfig::remote(16).workers == 16` and `PoolConfig::remote(100).workers == 16` (the new clamp)
- [ ] A review session with no `review.concurrency` override builds a pool with 16 workers
- [ ] `review.concurrency` still pins the worker count and disables AIMD
- [ ] `PoolConfig::local()` still yields 1 worker

## Tests

- [ ] Update the unit tests in `crates/swissarmyhammer-validators/src/validators/pool.rs` (around lines 1557–1573) that assert the remote clamp and AIMD flags; add an assert for the 16 cap
- [ ] Check the test at `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1859` that asserts `session.workers == DEFAULT_REMOTE_WORKERS`; it compiles against the constant and must still pass
- [ ] Run `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review