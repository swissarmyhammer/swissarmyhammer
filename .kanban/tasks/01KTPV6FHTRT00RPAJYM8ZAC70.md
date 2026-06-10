---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8e80
project: local-review
title: Review local backend silently drops fan-out tasks when the shared llama queue is full
---
## Symptom (observed on a real run)

A `review … backend=local` sweep of `../calcutron` (production Qwen3 35B MoE: `unsloth/Qwen3.6-35B-A3B-MTP-GGUF`) completed without error but produced **zero findings on every run**. From `../calcutron/.sah/mcp.log` (17:43–18:34):

- `Queue is full` — **420 occurrences**
- `fleet task failed; yielding zero findings for this batch … error=Internal error: Failed to execute prompt: Request processing error: Queue is full` — **60** (≈ every fan-out task, across all 15 validators)
- All **14** `review verify complete` lines: `candidates=0 confirmed=0 refuted=0` (fan-out produced nothing to even verify)
- `did not parse` = 0 (so it's rejection, not bad JSON)

The review didn't find zero issues — it failed to review almost anything, and reported success anyway.

## Root cause

- `llama-agent/src/queue.rs:1004-1016` `enqueue_request` uses `sender.try_send(...)`; a full bounded channel returns `QueueError::Full` **immediately** — no backpressure, no retry.
- `swissarmyhammer-agent/src/lib.rs:1163-1166`: `QueueConfig { max_queue_size: 100, worker_threads: 1 }`, and the `AgentServer` is cached + **shared across all connections** ("reusing cached llama AgentServer (shared across connections)").
- `swissarmyhammer-validators/src/validators/pool.rs`: `PoolConfig::local()` is 1 worker and serializes (proved by `test_pool_local_runs_one_at_a_time`), so a single review pipeline cannot overflow a 100-deep queue on its own. The overflow is **concurrent contention on the one shared local model** — overlapping `review` tool calls and/or the `/finish` loop's other agent traffic hitting the same server — combined with the review's drop-on-full submit path.
- `REVIEW_PIPELINE_GATE` (`review_op.rs:146`, `Semaphore::const_new(1)`) serializes review *pipelines* within a process, but does **not** bound contention on the shared `AgentServer` from non-review agent traffic.

## Fix direction (decide during implementation)

The review path must apply **backpressure** to a saturated local model instead of dropping work:
1. Non-streaming submit waits for capacity (bounded `send().await`) rather than `try_send` — at least on the path the review drives; or
2. The review pool retries on `QueueError::Full` with bounded backoff; or
3. Guarantee the shared local `AgentServer` is only ever driven by one consumer at a time (extend the gate to cover all llama contention, or give review its own server instance).

Avoid simply bumping `max_queue_size` — that defers the wedge, it doesn't fix the silent drop.

## Acceptance criteria

- A `review working backend=local` run against a real repo produces **zero** `fleet task failed … Queue is full` warnings.
- Where findings exist, `review verify complete` shows non-zero `candidates`.
- A unit/integration test proves a slow/saturated local agent causes the review worker to **wait**, not drop the task (extend `pool.rs` tests with a queue-full/slow agent seam).

Related: [[project_review_oom_real_cause]] — same class (nothing caps concurrent load against one shared model; there it OOMed, here it queue-fulls).

## Review Findings (2026-06-09 14:02)

### Blockers
- [x] `crates/llama-agent/src/queue.rs:3644` — The ~30-line 'park the worker + fill the channel to capacity' preamble in batch_submit_backpressure_honors_cancellation (3644-3680) is a near-verbatim copy of the preamble in batch_submit_applies_backpressure_when_saturated (3540-3585), differing only in binding names (_first/_filler vs first/filler) and where max_queue_size is bound. Two copies of this setup drift out of sync as the queue/GatedExecutor API evolves. Extract a single helper, e.g. `async fn saturated_gated_queue() -> (Arc<RequestQueue>, Arc<Notify>, Arc<AtomicUsize>, Session, JoinHandle<...>, JoinHandle<...>)`, that builds the gated single-slot queue, spawns the worker-occupying submit, waits for it to park, spawns the filler, and returns the handles. Both tests then call it and proceed to their distinct assertions.

### Warnings
- [x] `crates/llama-agent/src/queue.rs:967` — The doc comment on `submit_request` states it "Returns [`QueueError::Full`] if the queue is at capacity", but `enqueue_request` was changed to apply backpressure — it now awaits `send` and WAITS for a slot instead of returning `Full`. The doc contradicts the new behavior, so callers writing `Err(Full)` handling for the batch path will find it never triggers. Update the doc to: returns `QueueError::WorkerError` if the worker fails or the request is cancelled while waiting for capacity, and note that a saturated queue applies backpressure (awaits a free slot) rather than returning `Full`.
- [x] `crates/llama-agent/src/queue.rs:3540` — The GatedExecutor fixture setup (build `gate` + `entered`, construct `QueueConfig { worker_threads: 1 }`, wrap a `GatedExecutor` in `RequestQueue::with_executor`) is duplicated across four tests. The module already provides a parallel constructor `scripted_queue(outcome) -> (RequestQueue, Arc<AtomicUsize>)` for the scripted path; the gated path is a near-match that was copied rather than given the same factory. Add `fn gated_queue(max_queue_size: usize) -> (RequestQueue, Arc<Notify>, Arc<AtomicUsize>)` mirroring `scripted_queue`, and have the four gated tests call it.
- [x] `crates/llama-agent/src/queue.rs:3564` — The 'spin until the worker parks' loop — `for _ in 0..40 { if entered.load(SeqCst) == 1 { break } sleep(5ms) }` — is duplicated byte-for-byte three times (queue.rs:3564, 3667, 3952), each carrying the same magic pair of 40 iterations × 5ms poll interval. These are parallel code paths a human must keep in lockstep: retuning the park timeout (e.g. to deflake on a slow CI box) means editing the same constants in three places, and a missed copy drifts silently. Extract a single `async fn await_worker_parked(entered: &AtomicUsize)` (mirroring `await_queue_drained`) that owns the `40` and `5ms` constants once, and replace the three inline loops with a call. The two existing magic numbers become named locals (e.g. `PARK_POLL_ATTEMPTS`, `PARK_POLL_INTERVAL`) inside that one helper.
- [x] `crates/llama-agent/src/queue.rs:3564` — The 'wait until the worker has parked on the gate' polling loop (`for _ in 0..40 { if entered.load() == 1 { break } sleep(5ms) }`) is copied verbatim into both new tests. With the pre-existing copy in `streaming_submit_returns_full_at_capacity`, the idiom now appears three times — rule of three is met, and the module already extracts exactly this kind of poll-until-condition loop into a helper. Add a sibling helper next to `await_queue_drained`, e.g. `async fn await_worker_parked(entered: &AtomicUsize)`, and call it from all three sites instead of re-inlining the 6-line loop.

### Nits
- [x] `crates/llama-agent/src/queue.rs:968` — `submit_request` can now also fail via the cancellation arm of `enqueue_request` (returns `QueueError::WorkerError("Request cancelled")` when the token fires during backpressure), but the doc only mentions `WorkerError` "if the worker fails" — the cancellation failure mode is undocumented. Mention that a cancelled request (token fired while waiting for capacity) also returns `WorkerError`.