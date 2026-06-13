---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa780
project: local-review
title: 'fix(llama-agent): wire pin-on-save through the prime path so criterion-3 born-pinned save is structural, not budget-only'
---
## What

Close the prime→pin eviction race STRUCTURALLY (atomically), not just by budget headroom.

Today (after task 01KV0KFAD6V4E9HR9BZ9QMPZ3T) the race is closed only by the RAM-scaled byte budget (`max(2 GiB, total/4)`) plus the 64-entry floor so `evict()` does not fire during priming. That makes the race improbable on a high-RAM box but NOT impossible: on a constrained box (RAM clamped to the 2 GiB floor + large per-entry KV blobs) enough concurrent saves during priming can still push `cur_bytes() > max_bytes` and evict a not-yet-pinned prefix before its pin lands — the exact `failed to pin primed prefix state` failure.

The atomic born-pinned mechanism ALREADY EXISTS as the spec/seam: `crates/llama-agent/src/queue.rs` `insert_pinned` → `insert_inner(.., pin_on_save = true)` makes an entry born pinned so it is never an eviction candidate from the moment its bytes land. It is `#[cfg(test)]`-only today, exercised by the two store-level race tests (`pin_on_save_survives_concurrent_eviction_race`, `post_save_pin_loses_the_race_without_pin_on_save`). Those tests are the spec for this follow-up — keep them.

## Why it is a cross-crate change

The prime is two round-trips and the save site does not know the caller intends to pin:
- `prime_validator_prefix` (crates/swissarmyhammer-validators/src/review/fleet.rs) calls `pool.submit_primed(prefix)`; the prime turn's generation-path finalize-save happens INSIDE the queue worker DURING that turn.
- The pin is a SEPARATE `session/pin` ACP extension call made AFTER the turn completes (`pool.pin_session_scoped`).

So the worker's finalize-save currently passes `pin_on_save = false` (the sole production `insert_inner` caller is `insert`). Threading a "pin on save" intent requires a cross-crate change:
`run_validator_fleet`/`prime_validator_prefix` (validators) → `submit_primed` carries a pin-on-save intent → ACP `session/prompt` (or a new `session_fork` ext field) in `agent-client-protocol-extras` → queue worker finalize-save → `insert_inner(.., pin_on_save = true)`.

## Acceptance Criteria

- [x] A pin requested for a just-saved primed prefix CANNOT be defeated by another session's concurrent save, structurally (born-pinned at save time), independent of the byte/entry budget — i.e. the race is closed even when the budget is clamped to the 2 GiB / 64-entry floor and per-entry blobs are large.
- [x] The production prime path (`prime_validator_prefix` → `submit_primed` → ACP → queue worker finalize-save) reaches `insert_inner(.., pin_on_save = true)`; `insert_pinned` (or an equivalent born-pinned save) is no longer `#[cfg(test)]`-only dead code on the production path.
- [x] The two store-level race tests (`pin_on_save_survives_concurrent_eviction_race`, `post_save_pin_loses_the_race_without_pin_on_save`) still pass and are joined by an end-to-end fleet test (scripted agent, no real model, <10s) asserting the primed prefix is born pinned through the production path.
- [x] When pin-on-save lands, the separate post-turn `session/pin` confirm in `prime_validator_prefix` can be simplified (idempotent re-pin or status-only confirm) — do not leave two competing pin protocols.

## Resolution

Threaded a "pin on save" intent over ACP `_meta` (mirroring `MAX_TOKENS_META_KEY`), not a new `session_fork` ext field — the prime is a normal `session/prompt` turn whose save happens inside the queue worker during that turn, so the `_meta` channel is the consistent place. Chain:

- `agent-client-protocol-extras/src/lib.rs`: new `PIN_ON_SAVE_META_KEY` wire constant.
- validators `pool.rs`: `submit_primed` jobs carry `pin_on_save: true` (plain `submit`/`submit_forked` carry `false`); `run_prompt` sets the `_meta` key on the prime turn only.
- llama-agent `acp/server.rs`: `extract_request_pin_on_save(meta)` → `GenerationRequest.pin_on_save` (new field).
- llama-agent `queue.rs`: streaming prompt-boundary save threads `request.pin_on_save` into `save_prompt_boundary_state` → `insert_inner(.., pin_on_save)` — the born-pinned path is now production-reachable.
- fleet.rs `prime_validator_prefix`: doc + log clarified that the prefix is born pinned at save; `pin_session_scoped` is now an idempotent re-pin/confirm that returns the unpin guard (one pin protocol, not two). SessionPinGuard/unpin lifecycle preserved.
- claude-agent unaffected: it ignores the prompt `_meta` key (pin = no-op).

TDD: extended born-pinned store tests (kept), added `extract_request_pin_on_save` unit tests, a pool-level `_meta`-intent test, and a fleet-level born-pinned-through-production test (red-green-red verified). `cargo test -p llama-agent -p swissarmyhammer-validators -p agent-client-protocol-extras -p claude-agent` green; clippy --all-targets -D warnings clean on all four.

## Workflow
- Use `/tdd`. Extend the existing born-pinned store tests; add a fleet-level test through the scripted agent. No real model. Unit tests <10s.

## Context / Seam
- Born-pinned seam: `crates/llama-agent/src/queue.rs` `insert_inner` `pin_on_save` param + `insert_pinned`.
- Prime path: `crates/swissarmyhammer-validators/src/review/fleet.rs` `prime_validator_prefix`.
- ACP fork/pin contract: `crates/agent-client-protocol-extras` (`session_fork`).
- Predecessor task (budget-headroom mitigation that this completes): 01KV0KFAD6V4E9HR9BZ9QMPZ3T.

## Review Findings (2026-06-13 15:04)

> ⚠️ 7/45 review tasks failed — results are INCOMPLETE.

### Warnings
- [x] `crates/llama-agent/src/acp/server.rs:2565` — `prompt_inner` over 200 lines. RESOLVED — DEFERRED with rationale. This function is pre-existing-large; this task added exactly one line of logic (`let pin_on_save = extract_request_pin_on_save(...)`) plus one struct-field initializer. The suggested decomposition (`compute_turn_max_tokens` / `stream_turn` / `execute_tool_round`) is a large, risky refactor of agentic control flow this task did not touch — out of scope per the project's altitude norms (do not balloon a pin-on-save task into a control-flow refactor). The pin-on-save inspection IS already factored into the unit-testable `extract_request_pin_on_save`, mirroring `extract_request_max_tokens`.
- [x] `crates/llama-agent/src/agent.rs:992` — `generate` ~236 lines. RESOLVED — DEFERRED with rationale. This task did NOT touch `generate`'s body; the only agent.rs changes are `pin_on_save: ...` field initializers in scattered `GenerationRequest` struct literals (git diff: +10 lines, all struct-field additions). Extracting `append_assistant_tool_round` would be an unrelated refactor of a pre-existing-large function — out of scope.
- [x] `crates/llama-agent/src/queue.rs:2089` — `save_prompt_boundary_state` vs `save_session_state` snapshot duplication. RESOLVED — FIXED. Evaluated against the no-duplicate-but-different rule: the two savers are legitimately distinct call paths (batch post-generation vs. streaming pre-generation prompt-boundary) with distinct insert semantics (`insert` always-unpinned vs. `insert_inner(.., pin_on_save)` born-pinnable), so they are NOT collapsed. But the identical, riskiest block — the `unsafe copy_state_data` snapshot sequence (`get_state_size` → zeroed buffer → `copy_state_data` → 0-write guard → `truncate`) — WAS duplicated; extracted into a single owner `fn snapshot_ctx_state(worker_id, request_id, ctx) -> Option<Vec<u8>>` (`None` on the 0-write case). Both savers call through it, so a future fix to the unsafe handling lives in one place. Added a doc block on `save_session_state` documenting why both savers exist; dropped the now-unneeded `&mut` on its ctx.
- [x] `crates/llama-agent/src/queue.rs:2197` — `process_streaming_request_sync` oversized + three near-identical standard-stream arms. RESOLVED — FIXED (in scope: this task added the three `pin_on_save`-threading save closures). Extracted `fn run_standard_stream(..)` that builds the `save_prompt_boundary_state` closure ONCE; the three duplicated ~22-line blocks (non-MTP, draft-not-ready fallback, draft-create-error fallback) now collapse to single calls, so `pin_on_save` is threaded in one place and cannot drift.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:332` — `prime_validator_prefix` 64 lines / three inlined fallible phases. RESOLVED — FIXED (in scope: this task reworked this function). Extracted `submit_prime` / `confirm_saved_state` / `pin_prefix`, each owning its own warn-and-return and full `tracing` payloads; `prime_validator_prefix` is now a 3-line orchestrator.

### Nits
- [x] `crates/llama-agent/src/queue.rs:1` — inline drain poll budget in `test_streaming_worker_released_after_turn`. RESOLVED — FIXED. Added named `DRAIN_POLL_ATTEMPTS` / `DRAIN_POLL_INTERVAL` consts in the outer `tests` module (the `worker_lifecycle_tests` consts are in a nested module not visible here) and used them in the loop.
- [x] `crates/llama-agent/src/queue.rs:3158` — same drain-poll loop magic numbers (this anchor and :1 both point at the single `test_streaming_worker_released_after_turn` drain loop). RESOLVED — FIXED by the named `DRAIN_POLL_ATTEMPTS` / `DRAIN_POLL_INTERVAL` consts above.
- [x] `crates/llama-agent/src/validation/generation_request/session_validator.rs:35` — `create_test_request()` magic literals. RESOLVED — FIXED. Hoisted `TEST_MAX_TOKENS` / `TEST_TEMPERATURE` / `TEST_TOP_P` named consts documenting they are arbitrary-but-valid sample values.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1` — `wait_for()` ~3s timeout as two coupled literals. RESOLVED — FIXED. Named `POLL_INTERVAL` (10ms) and `POLL_TIMEOUT` (3s) and derived the retry count from their ratio, so the wait budget is expressed once.