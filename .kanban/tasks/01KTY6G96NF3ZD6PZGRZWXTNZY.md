---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa580
project: local-review
title: 'fix(review): idle-liveness counts llama queue-wait as stall — start the clock at first decode [BLOCKER: blocks qwen review completion]'
---
## What

**PRIORITY RAISED 2026-06-13 — now BLOCKING qwen review completion, not a minor task-loss.** In the 2026-06-13 prime+fork qwen run (../calcutron/.sah/mcp.33532.log), after 88 minutes and 4 review calls, **ZERO reviews completed (verify complete=0)** and **11 fleet tasks were abandoned, all "made no streaming progress for 300s"** (climbing 2→8→11). Pin was clean (failed-to-pin=0), fork reuse healthy (44 reuse events, ~32k tokens each), Queue 0/0 — the KV mechanism works, but the run can't finish a review because tasks die in the queue.

**Why prime+fork AMPLIFIED this:** the old monolithic path was one turn per validator-batch. Prime+fork is one prime per validator (~15) PLUS one fork per batch — far more total turns, all serialized on the single GPU (one worker + GpuLock). Deeper queue → longer queue-waits → the buggy idle clock (which starts at SUBMISSION, counting innocent queue-wait as "no progress") trips its 300s timeout on many more turns. The prefill optimization multiplied the turn count, and this bug converts that into mass abandonment.

**Mechanism (verify in code):** `run_turn_with_liveness` in crates/swissarmyhammer-validators/src/validators/pool.rs starts the idle deadline at turn submission. A fork queued behind other decodes on the one GPU streams nothing until it actually runs; if it waits >300s (PROMPT_IDLE_TIMEOUT) in the queue it is abandoned + session/cancel'd before it ever decodes.

Fix (pick cleanest; verify against the notification flow):
- [ ] Start the idle window only when the turn shows FIRST progress (first session/update / first token) — before that apply only the absolute ceiling (PROMPT_TURN_CEILING). A QUEUED turn is not idle. This is the core fix and directly unblocks the qwen run.
- [ ] And/or emit a queue-position/heartbeat notification from the llama side while a request is queued, so liveness sees genuine signal (a turn making forward queue progress is alive).
- [ ] Consider: with prime+fork the turn count per review is much higher — confirm the absolute ceiling (45min) is still appropriate for a fork that waits a long time then decodes, and that cancel-on-abandon doesn't cancel a parent prefix session a queued fork still needs.

## Acceptance Criteria

- [ ] A turn that waits longer than the idle window in the queue, then decodes normally, completes instead of being abandoned.
- [ ] A turn that genuinely stalls AFTER starting (no progress post-first-token for the idle window) is still abandoned.
- [ ] The absolute ceiling still bounds total turn time including queue wait.
- [ ] Re-run the calcutron/harness qwen review: reviews reach verify-complete (>0), idle-abandons drop toward 0.

## Tests

- [ ] pool.rs scripted-agent tests (<10s, no model): (a) delayed-start turn — no progress for > idle window, then streams and completes → must NOT be abandoned; (b) started-then-stalled turn → abandoned; (c) never-starts turn → ceiling abandons it; (d) NEW: a queue of N turns where only one decodes at a time (gated executor) — none of the waiting turns abandon while their turn-to-run is pending under the ceiling.
- [ ] `cargo test -p swissarmyhammer-validators` green.
- [ ] Real-model: `python3 scripts/review-verify/drive.py` reaches a completed review with idle-abandons=0 on qwen.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Evidence

2026-06-12 mcp.50099.log: 25 abandons, all "no streaming progress", reviews still completed (91/194 min). 2026-06-13 prime+fork mcp.33532.log: 11+ abandons in 88 min, verify-complete=0 — prime+fork's higher turn count pushed queue-waits past the 300s submission-anchored idle clock, blocking completion. failed-to-pin=0, fork reuse 44 events ~32k tokens, Queue 0/0 — isolating this as the liveness bug, not pin (9qmpz3t) or queue (resolved).