---
assignees:
- claude-code
comments:
- actor: wballard
  id: 01kvbanf295pvq0a46xetxhwws
  text: |-
    Implemented per card approach (one file: crates/llama-agent/tests/integration/acp_agentic_loop.rs).

    TDD: added deterministic #[test] no_progress_outcome_took_tool_path_is_false FIRST -> RED (E0599: no_progress/took_tool_path not found). Then implemented to GREEN.

    Changes:
    - Added impl ToolTurnOutcome with no_progress() (all-empty: tool_call_broadcast=false, tool_messages=0, tool_calls_executed=0, empty final_text/streamed_agent_text) and took_tool_path() = tool_call_broadcast || tool_messages >= 1.
    - run_tool_turn: kept the NO_HANG_BUDGET timeout .expect("tool-calling prompt must not hang") unchanged; replaced the inner .expect("prompt must succeed...") with a match on the prompt Result -> on Err, info!-log and return ToolTurnOutcome::no_progress(); on Ok, continue the existing success path.
    - Retry loop call site now uses outcome.took_tool_path() instead of the inline literal. Hard dispatch guards and final panic! unchanged.

    Note: test named ...took_tool_path... so the card's filter `... took_tool_path` matches it.

    Verification (fresh):
    - cargo build --tests -p llama-agent -> Finished, exit 0.
    - cargo nextest run -p llama-agent --test agent_tests took_tool_path -> 1 test run: 1 passed, 108 skipped, exit 0.
  timestamp: 2026-06-17T17:39:34.089456+00:00
- actor: wballard
  id: 01kvbbks0p1gnd64v9wvav4jjm
  text: |-
    Worked the Review Findings (2026-06-17 12:41) checklist. Scope held to crates/llama-agent/tests/integration/acp_agentic_loop.rs only; did not touch the unrelated working-tree edits (crates/mirdan/src/status.rs, builtin/validators/rust/rules/*.md).

    Three doc-comment warnings (init_tracing, drain, parse_llama_id): on inspection all three helpers ALREADY carry doc comments in the current file (HEAD + working tree), matching the review's exact suggested style:
    - init_tracing -> /// Initialize test tracing once; ignore the "already initialized" error.
    - drain -> /// Drain every notification currently buffered on the receiver into a Vec. (plus a body paragraph on Lagged resilience)
    - parse_llama_id -> /// Parse the ACP session id into the internal llama SessionId. ...
    The review's line numbers (72/76/96/400) are stale relative to the current file (helpers now at ~166/183/959); the findings predate the prior iteration. The requirement (documented helpers) is satisfied — no edit needed, would only duplicate. Checkboxes flipped to [x].

    DECLINED finding (acp_agentic_loop.rs nested-loop drain_remaining extraction): NOT refactored, knowingly declined per really-done proceed-with-justification contract. The loop { tokio::select! { ... while let Ok(n) = sub.try_recv() ... } } lives in NotificationCollector::start and is behavior-bearing: it concurrently drains the bounded broadcast channel during the awaited turn to prevent the oldest notifications (possibly the ToolCall) from being evicted. This test is being de-flaked right now; extracting/altering that draining logic risks introducing a subtle regression in the exact test we are stabilizing — not worth the risk for a style nit. Checkbox left UNCHECKED.

    No behavior changes made this iteration (pure findings triage); NO_HANG_BUDGET, the timeout .expect, the hard dispatch guards, and the run_tool_turn change from the prior iteration are all untouched.

    Verification (fresh, full output read):
    - cargo build --tests -p llama-agent -> Finished `dev` profile, exit 0 (compiles clean).
    - cargo nextest run -p llama-agent --test agent_tests took_tool_path -> 1 test run: 1 passed, 108 skipped, exit 0.

    Moving back to review.
  timestamp: 2026-06-17T17:56:07.318415+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbb80
title: 'De-flake acp_multi_turn: make run_tool_turn retry an aborted turn instead of panicking'
---
## What

The real-model ACP test `acp_multi_turn_dispatches_tool_and_threads_result` in `crates/llama-agent/tests/integration/acp_agentic_loop.rs` flakes CI (observed in run 27696055516). Root cause: the helper `run_tool_turn` (defined at `acp_agentic_loop.rs:406`) ends with

```rust
let response = tokio::time::timeout(NO_HANG_BUDGET, server.prompt(request))
    .await
    .expect("tool-calling prompt must not hang")
    .expect("prompt must succeed against a healthy model");   // panics here
```

With the canonical small Qwen3-0.6B test model, `server.prompt()` non-deterministically returns `Err` carrying `agentic_loop_aborted: "every tool call failed for 3 consecutive step(s)"` — the model emits a spurious failing tool call that trips the engine's (correct) no-progress abort guard. The second `.expect` panics **hard on the first such attempt**, bypassing the test's existing `MAX_ATTEMPTS = 4` retry/skip loop (`acp_agentic_loop.rs:556-630`), which only tolerates *comprehension* misses, not an `Err` return.

The sibling test `acp_hooks_real_model.rs` already documents this exact nondeterminism (see its module doc lines 47-49 and 305-309) and deliberately does not gate on the whole-turn abort flag. This card brings `run_tool_turn` in line: an aborted/errored turn becomes a **retryable no-progress attempt**, not a panic.

### Approach (scope: one file)
In `crates/llama-agent/tests/integration/acp_agentic_loop.rs`:
1. Add a `ToolTurnOutcome::no_progress()` constructor returning the all-empty outcome (`tool_call_broadcast: false`, `tool_messages: 0`, `tool_calls_executed: 0`, empty `final_text` / `streamed_agent_text`).
2. In `run_tool_turn`, keep the `NO_HANG_BUDGET` timeout `.expect("tool-calling prompt must not hang")` (a genuine hang must still fail loudly — that is a separate concern, see below), but replace the inner `.expect("prompt must succeed against a healthy model")` with a match: on `Err`, `info!`-log the error and `return ToolTurnOutcome::no_progress()`; on `Ok(response)`, continue with the existing success path (notifications, session lookup, meta parsing).
3. Extract the loop's tool-path predicate `tool_call_broadcast || tool_messages >= 1` into a method `ToolTurnOutcome::took_tool_path(&self) -> bool` and use it both at the call site (`acp_agentic_loop.rs:559`) and in the new unit test.

Net effect: a spurious-abort on an early attempt is retried with a fresh session (exactly like a comprehension miss); only a persistent failure across all 4 attempts reaches the existing final `panic!` at `acp_agentic_loop.rs:625` with its diagnostics. The hard dispatch guards (ToolCall broadcast, Tool-role message, `tool_calls_executed >= 1`, no markup leak) still fire on any attempt that does take the tool path, so a genuine loop break still fails the test.

### Explicitly out of scope
The second CI failure — `acp_hooks_real_model::pre_tool_use_deny_blocks_real_read_file_through_live_loop` timing out at `NO_HANG_BUDGET` (120s) — is a model-worker throughput / KV-cache-contention concern, not test brittleness. It is a separate investigation card and must NOT be addressed here (do not raise `NO_HANG_BUDGET` to mask it).

## Acceptance Criteria
- [x] `run_tool_turn` no longer panics when `server.prompt()` returns `Err`; it returns `ToolTurnOutcome::no_progress()` and logs the error at `info`.
- [x] The `NO_HANG_BUDGET` timeout `.expect("tool-calling prompt must not hang")` is retained unchanged (a hang still fails loudly).
- [x] The tool-path condition is a single `ToolTurnOutcome::took_tool_path()` method used by both the retry loop and the new test (no duplicated `tool_call_broadcast || tool_messages >= 1` literal).
- [x] An aborted/errored turn is treated as a retryable attempt: the test fails only when no attempt takes the tool path across all `MAX_ATTEMPTS`, reaching the existing final `panic!`.

## Tests
- [x] Add a deterministic (no real model) `#[test]` in `crates/llama-agent/tests/integration/acp_agentic_loop.rs` asserting `!ToolTurnOutcome::no_progress().took_tool_path()` (a no-progress outcome is classified as "did not take the tool path", so the loop retries rather than asserting on it) and that its `tool_messages`/`tool_calls_executed` are `0`.
- [x] `cargo nextest run -p llama-agent --test agent_tests took_tool_path` (the new unit test) → passes.
- [x] `cargo build --tests -p llama-agent` → compiles clean (the real-model `acp_multi_turn_dispatches_tool_and_threads_result` still builds; it is model-gated at runtime via `build_real_model_server`).

## Workflow
- Use `/tdd` — write the `took_tool_path` / `no_progress` unit test first (it won't compile until the method + constructor exist), then add them and refactor `run_tool_turn` to make it pass.

## Review Findings (2026-06-17 12:41)

### Warnings
- [x] `crates/llama-agent/tests/integration/acp_agentic_loop.rs` — `init_tracing` lacks a doc comment. RESOLVED: doc comment present — `/// Initialize test tracing once; ignore the "already initialized" error.`
- [x] `crates/llama-agent/tests/integration/acp_agentic_loop.rs` — `drain` lacks a doc comment. RESOLVED: doc comment present — `/// Drain every notification currently buffered on the receiver into a Vec.` (with body detailing Lagged resilience).
- [ ] `crates/llama-agent/tests/integration/acp_agentic_loop.rs` — Nested loops with mixed control flow: `loop` contains `tokio::select!` branches, one of which holds a `while let` loop. Suggested extracting the drain-and-break logic into `drain_remaining(sub)`. DECLINED (proceed-with-justification): this test is being stabilized for flakiness; the draining loop is behavior-bearing (it concurrently drains the bounded notification channel to avoid ToolCall eviction), and extracting/altering it risks introducing a subtle regression in the exact test we are de-flaking — not worth the risk for a style nit. See kanban comment recording this decision.
- [x] `crates/llama-agent/tests/integration/acp_agentic_loop.rs` — `parse_llama_id` lacks a doc comment. RESOLVED: doc comment present — `/// Parse the ACP session id into the internal llama SessionId. ...`