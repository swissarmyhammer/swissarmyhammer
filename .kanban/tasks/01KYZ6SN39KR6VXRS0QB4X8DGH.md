---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz4kmzvmntb0qza6tmfmfxky
  text: |-
    ### implement — changed
    - files: crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs (DEFAULT_REMOTE_WORKERS 4 -> 16), crates/swissarmyhammer-validators/src/validators/pool.rs (MAX_REMOTE_WORKERS 8 -> 16; test_pool_config_remote_clamps_workers updated, RED confirmed before the constant change then GREEN after)
    - pool_config_for (review_op.rs) and PoolConfig::local() left untouched, as specified
    - cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools: all unit/integration suites pass (validators 1284 passed; tools 7+1+1+8+17+1+1+1+3+2+162+1+362 passed); one PRE-EXISTING doctest failure at crates/swissarmyhammer-tools/src/lib.rs - mcp (line 93) (start_mcp_server arg-count mismatch in a doc comment) confirmed unrelated by stashing my two edits and re-running --doc, same failure reproduces without my changes
    - cargo fmt --all: no changes
    - cargo clippy --workspace --all-targets -- -D warnings: clean
    - cargo nextest run -E 'rdeps(swissarmyhammer-validators) or rdeps(swissarmyhammer-tools)': 3067 tests run: 3067 passed (3 slow, 1 leaky), 0 skipped
    next: /review
  timestamp: 2026-08-03T20:04:41.972247+00:00
- actor: claude-code
  id: 01kz4ksq785yqc4mg5wywftzfk
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — DEFAULT_REMOTE_WORKERS (review_op.rs) 4->16, MAX_REMOTE_WORKERS (pool.rs) 8->16, test_pool_config_remote_clamps_workers updated to assert the new 16 cap. review.concurrency override and PoolConfig::local() (1 worker) confirmed untouched. TDD: RED confirmed before the constant edit, GREEN after.
    - test: green — cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools all green except one pre-existing unrelated doctest failure (confirmed via git stash + rerun reproducing identically without this change). fmt clean, clippy clean. Full rdeps(swissarmyhammer-validators) or rdeps(swissarmyhammer-tools) run for real: 3067 passed, 0 skipped, 0 failed.
    - commit: 07ca81895
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-03T20:07:16.968481+00:00
- actor: claude-code
  id: 01kz4mgqnd3rrbzp4gj49w5yah
  text: '### review — clean\n- evidence: review sha 07ca81895~1..07ca81895 — engine ran (counts: findings 13, confirmed 13, refuted 29, failed 0). All 13 confirmed findings blame-checked against 07ca81895: none fall inside the commit''s changed lines (review_op.rs:300, pool.rs:44, pool.rs:~1600-1610) — all reference pre-existing code (review_op.rs:1277,1450; pool.rs:343,576,1698,1728,1818,1819,1824,1860,1938,2012,2024) untouched by this diff. Dropped per pre-existing-code rule. Zero new findings.\n- next: done'
  timestamp: 2026-08-03T20:19:51.085299+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9680
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

- [x] Raise `DEFAULT_REMOTE_WORKERS` to 16 in `review_op.rs`
- [x] Raise `MAX_REMOTE_WORKERS` to 16 in `pool.rs`
- [x] Update the pool and review-op tests that assert the old clamp and default values

## Acceptance Criteria

- [x] `PoolConfig::remote(16).workers == 16` and `PoolConfig::remote(100).workers == 16` (the new clamp)
- [x] A review session with no `review.concurrency` override builds a pool with 16 workers
- [x] `review.concurrency` still pins the worker count and disables AIMD
- [x] `PoolConfig::local()` still yields 1 worker

## Tests

- [x] Update the unit tests in `crates/swissarmyhammer-validators/src/validators/pool.rs` (around lines 1557–1573) that assert the remote clamp and AIMD flags; add an assert for the 16 cap
- [x] Check the test at `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1859` that asserts `session.workers == DEFAULT_REMOTE_WORKERS`; it compiles against the constant and must still pass
- [x] Run `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
#review