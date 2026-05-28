---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: todo
position_ordinal: 8b80
project: llama-coverage
title: Cover queue state machine + worker lifecycle (queue.rs) via scripted model
---
## What

`crates/llama-agent/src/queue.rs` (2k lines) is the request queue + single-worker executor. The 0-token bug's second symptom — "Queue is full" on retry — was the worker never releasing after a runaway turn. Cover the state machine so worker-release invariants hold for every turn outcome.

## Cover

- **Worker release on every outcome** — after a turn that: completes normally, hits EOS, hits max_tokens, hits context-full, errors, or is cancelled — the worker must free up and a subsequent enqueue must succeed (no `QueueError::Full`). This is the regression guard for the shipped bug.
- **Queue full** — enqueue past `max_queue_size` returns the typed `Full` error (and only then).
- **Ordering / FIFO** — requests are processed in order by the single worker.
- **Cancellation** — a cancelled request releases the worker and does not corrupt the queue.
- **Backpressure** — the `worker_threads: 1` config: concurrent submits serialize correctly.

Use `ScriptedModel` to make turns deterministic (including the immediate-EOS 0-token turn and a long runaway-bounded turn).

## Acceptance Criteria

- [ ] Every turn-completion outcome is followed by an asserted successful re-enqueue (worker released).
- [ ] `Full` is returned only at capacity; never spuriously.
- [ ] Cancellation path covered.
- [ ] `queue.rs` region coverage reaches the epic threshold (target >95%).

## Tests

- [ ] Extend `queue.rs` `#[cfg(test)]` (the bug fix already added `test_streaming_worker_released_after_turn`; build the full matrix around it).
- [ ] Run: `cargo test -p llama-agent queue` and confirm the coverage delta.

## Workflow

- Use `/tdd`. Depends on the scripted-model keystone for deterministic turn outcomes.