---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
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

- [ ] A pin requested for a just-saved primed prefix CANNOT be defeated by another session's concurrent save, structurally (born-pinned at save time), independent of the byte/entry budget — i.e. the race is closed even when the budget is clamped to the 2 GiB / 64-entry floor and per-entry blobs are large.
- [ ] The production prime path (`prime_validator_prefix` → `submit_primed` → ACP → queue worker finalize-save) reaches `insert_inner(.., pin_on_save = true)`; `insert_pinned` (or an equivalent born-pinned save) is no longer `#[cfg(test)]`-only dead code on the production path.
- [ ] The two store-level race tests (`pin_on_save_survives_concurrent_eviction_race`, `post_save_pin_loses_the_race_without_pin_on_save`) still pass and are joined by an end-to-end fleet test (scripted agent, no real model, <10s) asserting the primed prefix is born pinned through the production path.
- [ ] When pin-on-save lands, the separate post-turn `session/pin` confirm in `prime_validator_prefix` can be simplified (idempotent re-pin or status-only confirm) — do not leave two competing pin protocols.

## Workflow
- Use `/tdd`. Extend the existing born-pinned store tests; add a fleet-level test through the scripted agent. No real model. Unit tests <10s.

## Context / Seam
- Born-pinned seam: `crates/llama-agent/src/queue.rs` `insert_inner` `pin_on_save` param + `insert_pinned`.
- Prime path: `crates/swissarmyhammer-validators/src/review/fleet.rs` `prime_validator_prefix`.
- ACP fork/pin contract: `crates/agent-client-protocol-extras` (`session_fork`).
- Predecessor task (budget-headroom mitigation that this completes): 01KV0KFAD6V4E9HR9BZ9QMPZ3T.