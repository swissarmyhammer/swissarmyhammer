---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9f80
project: local-review
title: 'fix(review): one failed turn permanently kills the shared llama queue — all later reviews fail "Queue is shutting down"'
---
## What

In the 2026-06-11 calcutron run (../calcutron/.sah/mcp.37798.log), after one fleet turn was abandoned at the 300s cap, the cascade was fatal to the whole serve process:

1. 19:20:28 — `prompt turn exceeded 300s and was abandoned` (the worker dropped its end; the underlying turn kept running).
2. The abandoned turn's response had nowhere to go: the 545s review call returned `review pipeline failed: Agent error: review agent connection failed: ... "failed to send response, receiver dropped"`.
3. That connection failure tore down the shared llama `AgentServer`'s queue: from 19:20:55 onward EVERY fleet task in EVERY subsequent review call failed instantly with `Request processing error: Queue is shutting down` — 90 of the 92 failures. Reviews 2–4 each failed 30/30 in ~15s with the completeness-guard error. The process never recovered; only a serve restart would fix it.

Investigate and fix the cascade in three layers (follow the actual paths before coding — the wiring spans crates):

- [x] **Abandoned turn must not poison the connection.** Found: `agent-client-protocol`'s dispatch loop (`incoming_protocol_actor`) treats a failed routing of an incoming response to its local oneshot awaiter (`ResponseRouter` → "failed to send response, receiver dropped") as connection-fatal — `connect_with` returns `Err`. Fix: new `agent_client_protocol_extras::TolerantResponseRouter` dispatch middleware claims `Dispatch::Response`, forwards to the awaiter, and logs-and-swallows delivery failure so an abandoned turn fails that turn only. Wired into `run_prompt_connection`'s client builder and both agent-side wrappers (`wrap_llama_into_handle`, `wrap_claude_into_handle`). The review drive's own client builder lives in `swissarmyhammer-validators` (concurrently being edited by another agent — out of this task's allowed scope); wiring it there is follow-up task 01KTW52WB3Q5KMNAWWJNYDRM59 (one line).
- [x] **Connection teardown must not shut down the shared cached AgentServer.** ROOT CAUSE FOUND (it was NOT the connection failure): `run_review_request` runs each review on a throwaway current-thread runtime inside `spawn_blocking`; the FIRST review initialized the shared cached `AgentServer` there, so the queue's lone worker task (`tokio::spawn` in `RequestQueue::spawn_workers`) lived on review 1's runtime. When review 1 ended (clean OR failed) and its runtime dropped, the worker was aborted, its receiver dropped, the channel closed → every later submit on the cached server failed `ShuttingDown`. Log evidence: "Worker 0 started" once at 19:11:41, zero "RequestQueue dropping" lines ever, "Queue sender closed, rejecting request" starting 19:20:55 right after review 1's tool_call completed at 19:20:45. Fix: `get_or_init_llama_agent_server` now runs `AgentServer::initialize` on a process-lifetime multi-thread runtime (`llama_server_runtime()` via `init_on_server_runtime`), so everything the shared server spawns matches the lifetime of the cache that shares it.
- [x] **Self-healing as a backstop.** `RequestQueue::is_closed()` (sender gone or channel closed = workers dead) + `AgentServer::is_healthy()`; `get_or_init_llama_agent_server` is refactored through generic `get_or_rebuild_cached`, which evicts and rebuilds an unhealthy cached server instead of handing out a corpse forever.

## Acceptance Criteria

- [x] A turn abandoned mid-review degrades to that one task's error; the same review's remaining tasks and all subsequent review calls in the same process still execute generation. (Subsequent calls: runtime pinning + self-heal. Same-review remaining tasks: `TolerantResponseRouter`; the review drive's one-line wiring is follow-up 01KTW52WB3Q5KMNAWWJNYDRM59 since that crate was off-limits during this task.)
- [x] After a forced queue shutdown, the next review call gets a working agent (rebuilt or never-killed), not instant 30/30 failures.
- [x] No path where dropping a single turn's receiver propagates to queue shutdown. (The queue's workers no longer live on any connection/review runtime; receiver drops are connection-local and now turn-local.)

## Tests

- [x] Unit tests at the right seams, scripted/fake agents, no model, all <10s: (a) `tolerant_routing::tests::abandoned_turn_does_not_kill_the_connection` — drops a turn's response receiver mid-flight, late response arrives, next turn on the same connection succeeds (reproduced the exact production error verbatim in RED); (b) `tests::server_init_tasks_survive_caller_runtime_drop` — background task spawned during init keeps answering after the initializing (per-review-style) runtime is dropped; (c) `tests::cache_rebuilds_unhealthy_server` / `tests::cache_reuses_healthy_server` + `queue::tests::worker_lifecycle_tests::is_closed_{false_while_workers_alive,true_after_worker_tasks_die}`.
- [x] `cargo test -p llama-agent -p swissarmyhammer-agent -p agent-client-protocol-extras` green (15 suites, 0 failures); `cargo clippy -p llama-agent -p swissarmyhammer-agent -p agent-client-protocol-extras --all-targets -- -D warnings` clean. (`swissarmyhammer-validators`/`-tools` were excluded per the concurrent-edit constraint; both still compile against the changes.)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. (Done: each piece stubbed, watched RED, then GREEN.)

## Evidence

mcp.37798.log marker counts: `Queue is full`=0 (backpressure fix holding), `fleet task failed`=92 (1 × 300s timeout, 1 × internal error, 90 × "Queue is shutting down"), `AgentMessage (`=20, GPU lock 21/21, reviews 2–4 each `-32603: incomplete review: 30/30 fan-out tasks failed (over 50% ...)` in ~15s. The completeness guard correctly refused to fake a clean pass — the failure surfacing works; the availability does not.