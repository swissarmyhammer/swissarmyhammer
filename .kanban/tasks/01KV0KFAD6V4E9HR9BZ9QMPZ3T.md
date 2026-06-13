---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
project: local-review
title: 'fix(llama-agent): prime+fork pin fails under a fixed 2GB KV cache budget — scale to RAM / reserve pinned pool'
---
## What

`failed to pin primed prefix state` fires (and every affected validator silently degrades to monolithic prompts, losing all KV reuse) when a freshly-primed prefix session is EVICTED between its prime-save and its pin call. Reported on a 128GB machine; reproduces less or not at all on smaller boxes — counterintuitive, and the budget is the reason.

**Root cause (verified in crates/llama-agent/src/queue.rs):**
- `set_pinned` returns `false` (→ pin failure) only when `entries.get_mut(id)` is `None` — i.e. the entry was already evicted.
- `SessionStateCache` has a HARDCODED `const MAX_SESSION_CACHE_BYTES: usize = 2 * 1024 * 1024 * 1024` (2GB) regardless of system RAM.
- Each cached entry is a full serialized KV blob from `get_state_data` (`CachedSession.state_bytes: Arc<[u8]>`). For a ~5k-token validator prefix on Qwen3.5-35B this is hundreds of MB; ~15 validators prime concurrently (`run_validator_fleet` via join_all in review/fleet.rs), but the single GPU serializes them, so each prime's `save → evict()` runs in turn. The LRU victim is the oldest UNPINNED entry — an earlier validator's state that hasn't been pinned yet. Validator 1's state is evicted by validator 3's save before validator 1's pin lands → pin fails.
- A 128GB machine makes it WORSE: (a) the 2GB cap ignores the extra RAM entirely; (b) a bigger box often loads the model with a larger `n_ctx`, so each serialized blob is bigger → fewer fit in 2GB → more eviction-before-pin races.

**Fixes (evaluate, smallest-first; (1)+(2) likely sufficient):**

- [ ] (1) Make `MAX_SESSION_CACHE_BYTES` configurable and default to a fraction of detected system RAM (e.g. via `sysinfo` total memory × ~0.25, floored at the current 2GB), instead of a hardcoded constant. On a 128GB box this alone holds all validator prefixes + forks.
- [ ] (2) Don't let a concurrent save's eviction defeat a pin: either reserve a separate pinned-byte allowance not counted against the unpinned eviction budget, OR pin atomically at save time when the caller intends to pin (pass an `intend_pin`/`pin_on_save` flag through the prime path so the entry is born pinned and is never an eviction candidate). This closes the prime→pin race regardless of budget.
- [ ] (3) Surface the pressure: when eviction drops an unpinned entry while the cache is near budget, log a warn with current bytes / budget / entry count so a future pin-failure cluster is diagnosable from the log (today eviction is silent — `grep evict` on the calcutron log returns 0 lines).
- [ ] (Stretch / separate task if it balloons) Resident-prefix strategy: since execution is GPU-serialized (one worker + GpuLock), avoid host-RAM full-KV snapshots entirely — keep each validator prefix resident in the live llama KV via sequence ids (`llama_kv_cache_seq_cp`) and run that validator's batches back-to-back against it. Eliminates the serialize/restore RAM cost that drives this bug. File as its own task if pursued.

## Acceptance Criteria

- [ ] On a high-RAM machine, priming ~15 validators each with a ~5k-token prefix and pinning all of them succeeds with zero `failed to pin primed prefix state` (the pins are not evicted by each other's saves).
- [ ] The cache budget is no longer a hardcoded 2GB; it scales with system RAM (verifiable: budget read back reflects a fraction of total memory, floored at 2GB).
- [ ] A pin requested for a just-saved entry cannot be defeated by another session's concurrent save (race closed by reserved pinned allowance or pin-on-save).
- [ ] Eviction near budget emits an observable warn.

## Tests

- [ ] crates/llama-agent/src/queue.rs unit tests (no model, <10s): (a) saturate the cache with N unpinned entries sized so total > budget, interleave saves and pins in the eviction-race order, assert every pin succeeds (reproduce the bug RED first with the old fixed budget + post-save pin); (b) pinned entries are never evicted even when a single save would exceed budget; (c) budget-from-RAM helper returns max(2GB, total×fraction) and is what the cache uses; (d) eviction-near-budget warn is emitted (tracing capture).
- [ ] `cargo test -p llama-agent` green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Evidence

Mechanism confirmed in crates/llama-agent/src/queue.rs: `set_pinned` (~line 360) false-on-missing; `MAX_SESSION_CACHE_BYTES` (~line 294) hardcoded 2GB; `evict()` (~line 370) drops LRU unpinned, skips pinned, protects MRU. Fleet concurrency in crates/swissarmyhammer-validators/src/review/fleet.rs (`run_validator_fleet` join_all → prime→status→pin per validator). Live 2026-06-13 qwen calcutron run (../calcutron/.sah/mcp.33532.log): on THIS machine failed-to-pin=0 (prefixes ~5047-5403 tokens, 12-15 primed, all pinned=true), but the same pattern tips into failure when blobs are larger relative to the fixed 2GB cap, which is what the 128GB report shows.