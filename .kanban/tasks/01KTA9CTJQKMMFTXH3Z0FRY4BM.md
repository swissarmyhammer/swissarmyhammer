---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff080
project: claude-hooks
title: Real-model hook E2E tests + fix PreToolUse deny to let the model continue
---
## Context
Adding real-Qwen E2E hook tests (below) surfaced a real bug in the live agentic loop, confirmed in code: a PreToolUse hook deny is classified as a runaway-guard FAILURE (`ProcessedToolCall.failed = true`, server.rs:2234; doc at 131-133 even says "a hook denial" counts as failed). The guard aborts a step where every tool call failed (server.rs:211: `failed_tool_calls == tool_calls`). So the COMMON case — model makes one tool call, a PreToolUse hook denies it — aborts the whole turn with an `agentic_loop_aborted` error instead of letting the model see the block reason and continue. This contradicts Claude Code's behavior and task #7's documented intent ("feed it back to the model so the loop continues with the model informed, matching Claude's blocked behavior"). USER DECISION: make it Claude-faithful — the model continues.

## Part A — Behavior fix (server.rs)
- In `process_tool_call`, the `PreToolUseOutcome::Deny` branch must NOT count as a runaway-guard failure: return `failed: false`. A deny is intentional, informed forward progress; the deny reason is already appended to the session (`model_feedback`) and broadcast to the client as a failed `ToolCallUpdate` (keep that UI signal — it's separate from the runaway-guard `failed` flag).
- Update the now-stale docs: `ProcessedToolCall.failed` doc (server.rs:131-133) and the deny-branch comment (~2218-2238) currently say a denial counts as failed — fix them to say a deny is forward progress (the model is informed and continues), and that only genuine tool-error results / hard dispatch errors count as failures for the runaway guard.
- Net effect: an all-denied step now returns `failed_tool_calls == 0`, so `AgenticLoopLimits::evaluate` returns `Continue`; the loop re-prompts the model with the deny reason in the Tool message and the turn ends normally (EndTurn). The per-turn `max_iterations` cap (32) remains the infinite-loop safety net for a model that only ever calls a denied tool — confirm no new unbounded loop.

## Part B — Fast regression tests (no model)
- Seam test: assert `process_tool_call` on a PreToolUse deny returns `failed == false` (and still `stop_turn == false`, still threads the deny reason via `model_feedback`). Update the existing scripted deny test accordingly (it previously tolerated/asserted the old semantics).
- Guard unit test: `AgenticLoopLimits::evaluate(1, AgenticStep { tool_calls: 1, failed_tool_calls: 0 })` → `Continue` (a single denied-but-not-failed call is progress); keep/confirm the existing all-genuinely-failed case still `Abort`s.

## Part C — Real-model (Qwen3-0.6B) E2E tests through the live loop
Reuse the `acp_agentic_loop.rs` harness (`build_real_model_server` rate-limit skip, `/no_think` proven read_file prompt, bounded retry + `NotificationCollector`, `#[serial]`, fixture `tests/fixtures/multi_turn/example.rs` = `fn main`/`hello`, `start_read_file_mcp_server()`). Each test: temp PROJECT dir with `.claude/settings.json`, temp `HOME` (HomeGuard/serial isolation as in the `hook_lifecycle` tests), session cwd = that project dir, read_file MCP attached, drive `server.prompt()` (NOT `process_tool_call` directly). Bounded-retry only on "did the model emit the tool call"; skip-with-warn if it never does; rate-limit skip; NO_HANG_BUDGET.
1. **PreToolUse deny → model CONTINUES (corrected).** Hook: PreToolUse matcher `read_file`, command `touch`es a marker and emits the deny JSON (permissionDecision deny). Assert: model emitted the tool call (hard guard); `server.prompt()` returns **Ok** (turn completed, NOT agentic_loop_aborted); the threaded-back Tool message carries the deny reason and does NOT contain the fixture content (`hello`/`fn main`) — real dispatch was prevented; the marker file exists (hook command ran). The model got to continue past the block.
2. **PostToolUse additionalContext reaches the model.** Hook: PostToolUse matcher `read_file`, command emits a unique `additionalContext` marker. Assert: tool executed (`hello` present), turn Ok, and the unique marker reached the model's Tool message.

## Keep
Leave the other fast scripted seam tests and `run_bounded_tool_loop` unit tests as-is.

## Acceptance criteria
- Part A fix applied; an all-denied step no longer aborts — the model continues and the turn ends normally.
- Parts B and C tests exist and pass (run the real model locally to prove test 1+2 pass or legitimately skip on rate-limit; report what you observed).
- `cargo clippy` clean; full llama-agent + agent-client-protocol-extras suites green.