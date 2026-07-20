---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky0hsd4rnrp1g5y6fraw6qeg
  text: 'Finish loop started (single-task mode). Iteration 1: delegating to /implement. Plan per card: ReviewProgressEvent in swissarmyhammer-validators threaded through run_review_over_agent → run_fleet; MCP bridge in review_op.rs via spawn_drain_task gated on progress_token; unit test in fleet/tests.rs + e2e review_progress_notifications_test.rs.'
  timestamp: 2026-07-20T19:59:29.944482+00:00
- actor: claude-code
  id: 01ky0j4fhn73r0w4e4nphjzfdv
  text: 'Picked up; research done. Call chain confirmed: run_review_over_agent (drive.rs) → run_pipeline_in_connection → run_review (synthesize.rs, loops batches) → run_fleet (fleet.rs). Plan: (1) ReviewProgressEvent + ReviewProgressSender alias in fleet.rs, threaded as Option through run_review_over_agent/run_pipeline_in_connection (owned) and run_review/run_fleet (Option<&sender>); Planned{total_pairs} emitted per batch after plan_fan_out (bridge accumulates totals), PairStarted per (validator,file) in submit_fan_out, PairDone per pair in collect_fan_out after each task resolves (covers failed + monolithic-fallback since it is one emit site). (2) Bridge in review_op.rs: spawn_review_progress_bridge(context) honoring token+sink > token+peer > none (code_context precedence), two channels: engine events → mapping task (cumulative planned/completed state, monotonic progress) → ProgressNotificationParam channel → existing spawn_drain_task/spawn_in_process_drain_task; spawned in execute_review on the outer runtime, only the UnboundedSender crosses into spawn_blocking. TDD: fleet unit test (2 validators, val-ok 2 files + val-bad 1 file with ScriptedReply::Error → 3 pairs, Planned+3 started+3 done), mapping unit tests in review_op.rs, e2e HTTP test reusing tests/integration/review_fixture.rs via #[path] like review_global_subscriber.rs, using handle.server().set_review_factories.'
  timestamp: 2026-07-20T20:05:32.853422+00:00
- actor: claude-code
  id: 01ky0k6jn0bajjfgxfcdgtnv5y
  text: 'Implementation landed, all green. Engine: ReviewProgressEvent + ReviewProgressSender in fleet.rs (no rmcp dep), threaded run_review_over_agent → run_pipeline_in_connection → run_review → run_fleet; Planned{total_pairs} per batch after plan_fan_out, PairStarted per (validator,file) in submit_fan_out, PairDone per pair at the single resolution point in collect_fan_out (covers success, monolithic fallback, fork-failed, dropped, unparseable — one emit site after `match collected`). Bridge: review_op.rs spawn_review_progress_bridge(context) with token+sink > token+peer > none precedence; two channels (engine events → mapping task with cumulative planned/completed counters → ProgressNotificationParam → existing spawn_drain_task/spawn_in_process_drain_task); execute_review spawns it on the outer runtime BEFORE spawn_blocking, threads only the sync sender in, and awaits the drain after the run (also on the early-error path, since the sender is moved and dropped either way). Messages: "Planned N (validator, file) review pairs" / "Reviewing {file} against {validator}" / "Reviewed {file} against {validator}" — full paths, never truncated. TDD: fleet unit test watched RED (0 events) then GREEN; mapping tests watched RED (missing fn) then GREEN. Tests green: cargo nextest run -p swissarmyhammer-validators review → 141 passed; -p swissarmyhammer-tools --test review_progress_notifications_test → 1 passed (e2e HTTP, token echo, monotonic, per-pair started/done, progress==total); full validators crate 306 passed; tools -E ''test(review)'' 36 passed. cargo fmt clean; cargo clippy --all-targets -D warnings clean on both crates. Also updated the one external caller crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs (passes None); cargo check --workspace --all-targets green. Awaiting double-check verdict.'
  timestamp: 2026-07-20T20:24:10.144574+00:00
- actor: claude-code
  id: 01ky0kewdggsymcz9r7cavbxdz
  text: 'really-done complete: verification commands re-run green (cargo nextest run -p swissarmyhammer-validators review → 141 passed; cargo nextest run -p swissarmyhammer-tools --test review_progress_notifications_test → 1 passed) and the adversarial double-check agent returned PASS. Its pass confirmed: single PairDone emit site covers every resolution path (fork-failed fallback, dropped channel, unparseable) and its count equals the Planned total by construction; monotonicity holds under any Planned/PairDone interleaving (completed only increments, total = planned.max(progress)); no sender clone survives the pipeline so the awaited drain cannot hang (early-error and panic paths drop it too); all None call sites byte-identical to pre-change behavior; validators crate gained no rmcp dependency (Cargo.toml untouched). Noted on-contract: PairStarted repeats the current progress value (non-decreasing, matching the code_context precedent). Work complete and green — leaving the task in doing for /review.'
  timestamp: 2026-07-20T20:28:42.288211+00:00
- actor: claude-code
  id: 01ky0kg0ft6rdn43vfgyzev050
  text: 'Iteration 1 implement landed green. ReviewProgressEvent (Planned/PairStarted/PairDone) + ReviewProgressSender in fleet.rs, threaded through synthesize.rs run_review + drive.rs run_review_over_agent (None = unchanged behavior); MCP bridge spawn_review_progress_bridge in review_op.rs (sink > peer precedence, monotonic cumulative counters, drain awaited after run so notifications flush before the result); execute_review spawns bridge on outer runtime before spawn_blocking. Tests: fleet unit (2 validators × 3 files incl. failing task), review_op mapping/gating units, new e2e review_progress_notifications_test.rs. cargo nextest: validators 306 passed, tools review tests 36 passed, e2e 1 passed; fmt+clippy clean. Next: /test then checkpoint /commit then /review.'
  timestamp: 2026-07-20T20:29:19.226900+00:00
- actor: claude-code
  id: 01ky0rgeh98nt6e239p2cdta67
  text: 'Test-gate note: the rdeps(swissarmyhammer-validators) sweep was stopped after ~1h — it descended into swissarmyhammer-agent''s real-model agent_tests binary (llama/qwen, minutes per test). Lesson for this card''s gates: scope to `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools`; the only swissarmyhammer-agent change is a one-line test-callsite (None param) covered by cargo check --workspace --all-targets. Re-running the leaner gate now.'
  timestamp: 2026-07-20T21:56:56.489743+00:00
position_column: doing
position_ordinal: '8280'
title: 'Review MCP tool: emit notifications/progress per validator×file pair during the fan-out'
---
## What

The `review` MCP tool currently returns one batch result at the end and emits **zero** MCP notifications while it runs. The server already has all the progress plumbing — `crates/swissarmyhammer-tools/src/mcp/progress.rs` (`spawn_drain_task`, `spawn_in_process_drain_task`) and `ToolContext.progress_token` / `ToolContext.peer` / `ToolContext.progress_sink` in `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs` — but only `code_context` `op: "rebuild index"` is wired to it. Wire the review pipeline to it so a client that passes `_meta.progressToken` gets frequent `notifications/progress` as the review evaluates.

**Granularity:** the engine's fan-out unit (`plan_fan_out` in `crates/swissarmyhammer-validators/src/review/fleet.rs`) is one `ValidatorTask` per validator (RuleSet) × file batch; all of a validator's rules go to the agent in one prompt, so the finest real evaluation grain is the **(validator, file) pair**. Emit one progress event per pair — on task submission (`submit_fan_out`) for every file the task covers ("reviewing <file> against <validator>") and on task completion (`collect_fan_out`, including failed/degraded tasks) — with `total` = the number of planned (validator, file) pairs.

Implementation shape:

1. **Engine events (`swissarmyhammer-validators`, no rmcp dependency):** define a small `ReviewProgressEvent` (e.g. `Planned { total_pairs }`, `PairStarted { validator, file }`, `PairDone { validator, file }`) next to the fleet code, and thread an `Option<tokio::sync::mpsc::UnboundedSender<ReviewProgressEvent>>` through `run_review_over_agent` (`crates/swissarmyhammer-validators/src/review/drive.rs`) into `run_fleet` / `submit_fan_out` / `collect_fan_out` (`fleet.rs`). `None` = today's behavior. UnboundedSender::send is sync, so it works inside the nested runtime.
2. **MCP bridge (`swissarmyhammer-tools`):** in `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`, when a progress token is present, create the channel, map each `ReviewProgressEvent` to a `ProgressNotificationParam` (echoing the token, monotonic `progress` = completed-pair count, `total` = planned pairs, human `message` naming the validator and file — full paths, never truncated), and forward via the existing `spawn_drain_task(peer, rx)` (or `context.progress_sink` when set, matching the code_context precedence). **Runtime caveat:** `run_review_request` runs the pipeline inside `spawn_blocking` with a nested current-thread runtime — spawn the drain task on the outer runtime *before* entering `spawn_blocking` and hand only the sender in.
3. **Wiring:** `execute_review` in `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs` already receives `&ToolContext`; pass the token/peer/sink (or the constructed sender) into `run_review_request`. No token and no sink → pass `None`, emit nothing (same contract as rebuild index's NoopReporter).

Out of scope: progress for the verify/synthesize stages (can be a follow-up event variant later); changing the returned `ReviewReport` shape.

Subtasks:
- [x] `ReviewProgressEvent` + optional sender threaded through `run_review_over_agent` → `run_fleet` (drive.rs, fleet.rs)
- [x] Emit `Planned` after `plan_fan_out`, `PairStarted` per (validator, file) in `submit_fan_out`, `PairDone` per pair in `collect_fan_out` — including failed/monolithic-fallback tasks so progress always reaches `total`
- [x] Bridge events → `ProgressNotificationParam` in `review_op.rs`, drained through `spawn_drain_task` / `progress_sink`, gated on `progress_token`
- [x] Pass context through `execute_review` in tools `review/mod.rs`
- [x] Unit + e2e tests (below)

## Acceptance Criteria
- [ ] A `tools/call` for `review` (`op: "review working"`) carrying `_meta.progressToken` receives at least one `notifications/progress` per planned (validator, file) pair before the final result arrives
- [ ] Every notification echoes the request's progress token; `progress` is monotonically non-decreasing on the wire and reaches `total` (the planned pair count) even when fan-out tasks fail
- [ ] Each notification `message` names the validator and the file path, untruncated
- [ ] A call with no `progressToken` (and no in-process sink) emits zero progress notifications and returns the identical `ReviewReport` as today
- [ ] `swissarmyhammer-validators` gains no rmcp/MCP dependency; event emission with a `None` sender is a no-op on the existing code path

## Tests
- [x] Unit test in `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` (scripted agent, existing harness): run the fleet with a progress sender over 2 validators × known files; assert one `Planned` with the correct pair total, and `PairStarted`/`PairDone` for every (validator, file) pair including a deliberately failing task
- [x] New e2e `crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs` modeled on `rebuild_index_progress_notifications_test.rs`: real in-process HTTP MCP server + scripted review agent, client overrides `on_progress`, calls `review` with a `progressToken`; assert ≥1 notification per pair, token echo, monotonic progress
- [x] Run: `cargo test -p swissarmyhammer-validators review::fleet` and `cargo test -p swissarmyhammer-tools --test review_progress_notifications_test` — both green; keep unit tests under 10s (scripted agents only, no real model)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review #mcp #progress