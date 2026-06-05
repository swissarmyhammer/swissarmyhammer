---
assignees:
- claude-code
depends_on:
- 01KTBNFB7NPXNWKDK86T9A0M5C
position_column: todo
position_ordinal: '8680'
project: local-review
title: 'Teardown: retire AVP hook-execution machinery (keep loader + ACP executor)'
---
## What
Remove the hook-triggered execution path from the engine crate (still named `avp-common` at this point; rename happens next). This is the slowness the user never uses — an LLM agent spawned per validator, per tool call, via PreToolUse/PostToolUse/Stop hooks. KEEP the two genuinely reusable pieces for the new review fleet.

Remove:
- The chain/hook links that dispatch validators on hook events — `crates/avp-common/src/chain/links/validator_executor.rs` and the surrounding chain wiring that maps `HookType` (PreToolUse/PostToolUse/Stop) to validator runs.
- The hook-context plumbing: `MatchContext` hook-type fields, the turn-diff sidecar reading/writing (`.avp/turn_diffs/...`), and any `HookType`-driven branching that only exists to serve hooks.
- Stdin/stdout hook-protocol handling (exit-code-2 blocking, JSON decision blocks) that lived behind the deleted `avp` binary.

KEEP (these move into the engine, reused by the review fleet):
- The rules-as-data loader (`loader.rs`, `types.rs`) — file/glob matching, precedence stacking, `@`-include expansion.
- A hook-free **shared bounded agent pool** carved out of `runner.rs`'s concurrency engine — NOT a one-shot batch executor. Shape: `AgentPool` with `submit(prompt) -> Future<Result>` (or a channel), drained by a fixed set of workers over a single internal queue. This is the ONE place parallelism is controlled for the whole review pipeline; fan-out and verify both submit to the same pool, so verify tasks pipeline alongside still-running fan-out tasks.
  - **Worker count is the only concurrency control, and it is legitimate (physical/discovered, not arbitrary):**
    - local Llama backend → **1 worker** (one in-process model/GPU).
    - remote/Claude-API backend → N workers, default from config, optionally AIMD-adjusted (reduce active workers on rate-limit/timeout, recover after K successes) to discover the API ceiling.
    - a `review.concurrency` config value pins N when set.
  - Submission is unbounded and non-blocking (the queue absorbs it); only N run at once. Keep the per-call token cap.

## Acceptance Criteria
- [ ] No code path in the engine crate references `HookType`, PreToolUse/PostToolUse/Stop, turn-diff sidecars, or hook stdin/stdout protocol.
- [ ] A hook-free `AgentPool` exists: callers submit tasks at any time; a fixed worker count drains one shared queue; worker count is backend-aware (local→1, remote→N/AIMD, `review.concurrency` override); per-call token cap retained.
- [ ] All consumers of avp-common that used the hook path (e.g. `apps/swissarmyhammer-cli`) are updated or have the now-dead surface removed; `cargo build --workspace` green.
- [ ] No real-time validation fires on tool calls anywhere (the hook system is gone, as intended).

## Tests
- [ ] Retain/adapt the loader tests (precedence, glob match) — still green.
- [ ] `AgentPool` test (mock-agent harness `PlaybackAgent`/`SessionRecordingAgent`/`SlowAgent`): submit M tasks to a pool of N workers → all M results returned, never more than N agents in flight at once; tasks submitted mid-drain (pipelining) are picked up; a local-backend policy runs strictly 1 at a time; one slow/erroring task doesn't deadlock the pool.
- [ ] `cargo test -p avp-common` and `cargo build --workspace` green.

## Workflow
- Use `/tdd` — write the pool/concurrency/pipelining tests against the mock agents first, then carve the pool out of `runner.rs`. Delete dead hook code; do not leave commented-out husks. Depends on avp-cli removal (the binary entry point is already gone).