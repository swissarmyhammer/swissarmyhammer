---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa080
project: local-review
title: 'fix(review): 300s prompt-turn cap false-fires on local 35B — use progress-aware liveness'
---
## What

In the 2026-06-11 calcutron run (first run where `review working` had real scope, files=105), the first real local-qwen review died at `prompt turn exceeded 300s and was abandoned` (../calcutron/.sah/mcp.37798.log 19:20:28, validator=dead-code).

`PROMPT_TURN_TIMEOUT = 300s` in `crates/swissarmyhammer-validators/src/validators/pool.rs` is documented as a "generous backstop … tuned far above any legitimately slow turn so it never false-fires in normal operation". That holds for the remote backend; it is FALSE for local: a single legitimate turn on the 35B qwen (big review prompt + agentic loop + one shared GPU serializing decodes across fleet tasks) routinely needs >300s of wall clock. The timeout wraps the whole `run_prompt` (`new_session` → `prompt`), so queue wait + decode all count against 300s.

Fix — replace the wall-clock turn cap with progress-aware liveness:

- [x] In `worker_loop`/`run_prompt` (pool.rs), the worker already subscribes to the session's notification stream (`notifier.sender().subscribe()`). Abandon a turn only when NO notification/stream progress has arrived for the timeout window (idle timeout, reset on every received session update), not when total wall clock exceeds it. A turn that is actively streaming tokens is alive, however long it takes.
- [x] Keep a defensive absolute ceiling if desired, but it must be far above local-model reality (e.g. 30+ min) — document why. (`PROMPT_TURN_CEILING = 45 min`, documented.)
- [x] When a turn IS abandoned, actively cancel the underlying session (ACP `session/cancel` or the agent's cancel mechanism) so the agent stops decoding and the response sender isn't left to fire into a dropped receiver (see companion task on queue-shutdown cascade).

## Acceptance Criteria

- [x] A turn that streams a notification at least every N seconds never gets abandoned regardless of total duration. (`test_pool_streaming_turn_survives_beyond_idle_window`)
- [x] A turn with zero progress for the idle window is abandoned and degrades to a single-task error (fleet continues), as today. (`test_pool_stalled_turn_abandons_after_idle_window`)
- [x] Abandonment cancels the in-flight session rather than detaching it. (`test_pool_abandoned_turn_cancels_session`)

## Tests

- [x] pool.rs unit tests with a scripted agent: (a) slow-but-streaming turn (periodic updates, total > idle window) completes; (b) stalled turn (no updates) abandons after the idle window; (c) abandonment sends a cancel. Use short test-tuned durations (<10s, fake agent, no model). (Plus (d) the absolute ceiling abandons a streaming runaway. Implemented via `PoolConfig::with_idle_timeout`/`with_turn_ceiling` test-tuned windows; note the idle window must exceed claude_agent's fixed 500ms trailing notification drain.)
- [x] `cargo test -p swissarmyhammer-validators` green. (241 + 2 passed, 0 failed; clippy --all-targets -D warnings clean.)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. (Done: RED run showed (a)/(c)/(d) failing on the old wall-clock cap, (b) passing as behavior parity; GREEN after implementing liveness. Findings round: RED = typed-variant assertions + split builders fail to compile against old API; GREEN after `PoolError` + builder split.)

## Evidence

mcp.37798.log: GPU lock worked (21 waits/21 acquires), 20 AgentMessage replies streamed, zero "Queue is full" — the model was generating fine, just slower than 300s for the batch. 1 × `exceeded 300s`, then the cascade (see companion task) killed everything downstream: 90 × "Queue is shutting down", reviews 2–4 failed 30/30 instantly, each correctly rejected by the completeness guard ("incomplete review: 30/30 fan-out tasks failed").

## Review Findings (2026-06-11 15:25)

> ⚠️ 3/15 review tasks failed — results are INCOMPLETE.

### Blockers
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:488` — The entire in-test mock-agent harness (the `MockAgent` trait with nine default methods, `MockAgentAdapter`, `dispatch_mock_request`, `dispatch_mock_notification`, and `run_with_mock_agent`) is a near-verbatim copy of the public, reusable harness in `acp-conformance`'s `test_utils`. … **RESOLVED:** the copied trait/adapter/dispatchers were deleted from pool.rs; the pool tests now import `MockAgent`/`MockAgentAdapter` from `acp_conformance::test_utils` as a dev-dependency. Upstream contribution: `acp-conformance` now exports `test_utils` under a new `test-support` cargo feature (`#[cfg(any(test, feature = "test-support"))] pub mod test_utils`, `futures` made an optional dep) — the same exposure pattern `model-embedding` uses; `acp-conformance` added to `[workspace.dependencies]`. The upstream dispatch already had the `cx.spawn` offload, so no fork remains. Only the pool-specific notifier-forwarding client wiring (`run_client_against` + thin `run_with_mock_agent`/`run_with_playback_agent` wrappers) stays local, as the finding prescribed. (ScriptedAgent copies in fleet.rs/verify.rs are pre-existing and out of scope per review note.)

### Warnings
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:132` — `with_turn_liveness(idle_timeout, turn_ceiling)` swap-prone. **RESOLVED:** replaced by `with_idle_timeout(Duration)` and `with_turn_ceiling(Duration)` builder methods matching the `with_max_tokens` style; no other callers existed.
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:291` — `run_turn_with_liveness` nesting. **RESOLVED:** the select loop now has three one-line arms; the broadcast-recv policy is extracted into `fn note_progress(received, &session_slot, &mut last_progress, &mut liveness_open)` (Ok/Lagged/Closed policy documented on the helper) and the deadline arm into `fn abandon_turn(agent, &session_slot, ceiling_deadline, config) -> PoolError` (cancel-the-session + typed reason).
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:365` — stringly-typed abandonment. **RESOLVED:** introduced `pub enum PoolError { TurnIdle { idle_timeout }, TurnCeiling { turn_ceiling }, Agent(#[from] claude_agent::AgentError) }` (thiserror, `#[error(transparent)]` passthrough for agent errors); `PromptResult = Result<CollectedResponse, PoolError>`; re-exported from `validators::mod` and the crate root. Tests now assert `matches!(err, PoolError::TurnIdle { .. })` / `TurnCeiling` instead of substring-matching. Display text unchanged (lowercase, same phrasing) so existing log greps still work. fleet/verify/drive consume errors opaquely via Display — no behavior change; downstream crates (`swissarmyhammer-agent`, `swissarmyhammer-tools`) verified compiling.
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:734` — third `new_notifier` copy. **RESOLVED:** single `new_notifier` now lives in the existing shared `review::test_support` module (with the 64-slot buffer documented once); pool.rs, fleet.rs, and verify.rs import it.

### Nits
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:84` — dead intra-doc links. **RESOLVED:** `PROMPT_IDLE_TIMEOUT` / `PROMPT_TURN_CEILING` are now `pub const` (they document the production defaults, like `DEFAULT_MAX_TOKENS`); `cargo doc` shows zero pool.rs link warnings.
- [x] `crates/swissarmyhammer-validators/src/validators/pool.rs:402` — capitalized Display messages. **RESOLVED:** lowercased to `failed to create session: {e}` / `failed to execute prompt: {e}`. (The similarly capitalized messages in `claude-agent/src/lib.rs` are that crate's own seam, outside this finding's pool.rs scope.)

### Findings-round verification (2026-06-11)
- `cargo test -p swissarmyhammer-validators -p acp-conformance`: validators 241 + 2 doc-tests passed; acp-conformance 176 unit + 125 integration passed; 0 failures.
- `cargo clippy -p swissarmyhammer-validators -p acp-conformance --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean. `cargo check -p swissarmyhammer-agent -p swissarmyhammer-tools`: clean (PromptResult type change verified downstream).