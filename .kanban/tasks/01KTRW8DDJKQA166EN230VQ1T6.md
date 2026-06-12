---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9880
project: local-review
title: Cross-process GPU inference lock (flock) so multiple sah serves share one local model without thrashing
---
## Context / goal

One machine, one local GPU model, but the harness spawns **many `sah serve` processes** — one per `claude` (interactive sessions + every `/finish` subagent). We do NOT want a daemon and do NOT want backend-aware orchestration branching. The goal: N serves coexist on one machine sharing a single resident copy of the model, taking turns on the one GPU.

## What's already true (confirmed by reading llama.cpp source — checkout 5a6ab38, Apple Silicon)

**The model weights are already shared across processes, no-copy** — so there is NOTHING to do for the memory side:
- `ggml/src/ggml-metal/ggml-metal-device.m:837` — `use_shared_buffers = has_unified_memory` (true on Apple Silicon). `ggml_metal_buffer_from_ptr` (~line 1561) wraps host memory with `newBufferWithBytesNoCopy:… MTLResourceStorageModeShared` (1589/1616).
- `src/llama-model-loader.cpp:1536-1552` — with `use_mmap`, each weight tensor is allocated directly over the mmap address (`ggml_backend_tensor_alloc(buf_mmap, cur, data)`, `data = mapping->addr()+offs`); the copy branch (`ggml_backend_tensor_set`) is only the non-mmap `else`.
- `use_mmap` defaults `true` and the binding (`llama-cpp-2 .../model/params.rs`) has a getter but **no setter** — it can't be disabled. So N serves mmap the same GGUF → OS page cache keeps one physical copy of the 35B weights, every process's Metal buffer references it.

(Optional confirmation: `vmmap`/`footprint` on 2–3 isolated qwen serves to show the weights as shared, not private — nice-to-have, not blocking.)

## The actual problem this task fixes

Memory is shared, but the **GPU is singular**. Each process has its own KV cache, compute buffers, and Metal command queue; N processes submitting generations at once just **timeshare the one device** — no throughput gain, plus context-switch/scheduler overhead and contention. Nothing currently serializes GPU work *across processes* (the existing gates — review `REVIEW_PIPELINE_GATE`, embedding `EMBEDDING_GATE` — are in-process semaphores only).

## Fix: a cross-process file lock around inference

Add a machine-wide **`flock`-based lock** that serializes generation across all serve processes — one generation turn on the machine at a time. This is the cross-process generalization of the in-process gates we already built.

- Lock file at a well-known, model/GPU-keyed path (e.g. under the sah data dir, name derived from the model id — NOT a second hardcoded literal; reuse the model-source identity). One GPU ⇒ one lock; if multi-GPU is ever in play, key by device.
- Acquire exclusive lock around the actual decode/generation in the **llama-agent queue worker** (the single point all sessions already funnel through — `crates/llama-agent/src/queue.rs`), release immediately after the turn. Granularity = per generation turn (a long generation holding the lock is correct — it's one GPU).
- Use `flock(2)` / `fcntl` advisory locks: **kernel-released on process death**, so a crashed subagent serve cannot wedge the machine (no stale-lock cleanup logic needed). Pick a Rust wrapper already in the tree if one exists (check `Cargo.lock` — e.g. `fs2`/`fd-lock`); do not hand-roll raw libc unless necessary.
- Compose cleanly with the existing in-process queue + backpressure (m8zac70) — the in-process worker is already 1-at-a-time for local; this just extends "one at a time" across processes.

## Also reconsider `use_mlock(true)` (separate axis, low-risk cleanup)

`ModelManager::default_model_params` (and the embedding `default_model_params`) set `.with_use_mlock(true)`. mlock is *residency*, orthogonal to mmap *sharing*. Across N processes each `mlock`ing the same shared mmap pages it's redundant and counts against macOS memlock limits (can fail the load). Evaluate whether to drop `use_mlock(true)` now that sharing is via the page cache + Metal shared buffer. Decide with evidence; don't blindly flip it.

## Acceptance criteria

- A cross-process lock serializes llama generation machine-wide: with 2+ serves driving the model, only one generation runs at a time (the others wait, not error).
- The lock auto-releases if a holding process is killed mid-generation (no manual stale-lock recovery) — covered by a test that takes the lock in a child process and kills it, asserting another acquirer proceeds.
- The lock lives at the single queue-worker chokepoint; no per-caller duplication; composes with the existing in-process gate (no deadlock/double-wait pathology).
- A decision recorded on `use_mlock(true)` (keep or drop, with rationale).
- `cargo test -p llama-agent` and `cargo clippy -p llama-agent --all-targets -- -D warnings` clean. The cross-process test must be deterministic and <10s (use a temp lock path + a short-lived child, not the real model).

Context: [[project_review_local_queue_full_silent_drop]] and the in-process gates from the review-OOM cluster — this is their cross-process counterpart.