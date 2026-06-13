---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
project: local-review
title: 'fix(review): idle-liveness counts llama queue-wait as stall — start the clock at first decode'
---
## What

In the 2026-06-12 calcutron run (../calcutron/.sah/mcp.50099.log), the local review pipeline worked end-to-end for the first time (two completed reviews: 91min/8.4KB report with 147→30 confirmed findings, 194min/7.4KB with 161→26 confirmed; AgentMessage=530, GPU lock 548/548, zero queue-full/queue-shut). But **25 fleet turns were abandoned with "made no streaming progress"** (~4% task loss).

Likely mechanism (verify in code/log before fixing): two reviews ran concurrently; each pool (1 worker for local) submits turns into the SHARED llama RequestQueue, which serializes decodes on the one GPU. A turn from review B can wait in the llama queue behind review A's multi-minute decode, streaming nothing — `run_turn_with_liveness` in crates/swissarmyhammer-validators/src/validators/pool.rs starts the idle clock at submission, so ≥300s of innocent queue-wait reads as a stall and the turn is abandoned (and session/cancel'd) even though it never got a chance to run.

Fix options to evaluate (pick the cleanest, verify against the actual notification flow):
- [ ] Start the idle window only when the turn shows FIRST progress (first session/update or first token) — before that, apply only the absolute ceiling. A turn that is queued is not idle.
- [ ] And/or: emit a queue-position/heartbeat notification from the llama side while a request is queued, so liveness sees genuine signal.
- [ ] Confirm in mcp.50099.log that the 25 abandons cluster during the window where the two reviews overlapped (timestamps of "no streaming progress" vs the two review-call windows) and note findings on the task.

## Acceptance Criteria

- [ ] A turn that waits longer than the idle window in the queue, then decodes normally, completes instead of being abandoned.
- [ ] A turn that genuinely stalls after starting (no progress post-first-token for the idle window) is still abandoned.
- [ ] The absolute ceiling still bounds total turn time including queue wait.

## Tests

- [ ] pool.rs scripted-agent tests (<10s, no model): (a) delayed-start turn — no progress for > idle window, then streams and completes → must NOT be abandoned; (b) started-then-stalled turn → abandoned; (c) never-starts turn → ceiling abandons it.
- [ ] `cargo test -p swissarmyhammer-validators` green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Evidence

mcp.50099.log (2026-06-12): fleet task failed=25, all 25 "made no streaming progress for"; incomplete review=0 (under the 50% guard); both completed reviews returned real synthesized findings. Loss is availability-only — the guard kept reports trustworthy.