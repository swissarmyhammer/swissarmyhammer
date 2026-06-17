---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
project: kv-prefix-reuse
title: Rollback-aware donor selection in find_best_prefix_match
---
## What
Make cross-session donor selection account for recurrent-state rollback feasibility, so the zero-rollback pinned prime donor is preferred over a sibling whose marginally-longer LCP requires an infeasible recurrent rollback.

Today `find_best_prefix_match` (`crates/llama-agent/src/queue.rs:224`) ranks purely by `lcp` (line 243), tie-breaking only toward the caller's own session. A donor's real value is the number of tokens that survive the KV trim: the trim does `clear_kv_cache_seq(Some(0), Some(lcp), None)` which removes positions `[lcp, donor_len)`, i.e. a recurrent rollback of `donor_len - lcp`. On a recurrent/hybrid model `seq_rm` returns `Ok(false)` when that rollback exceeds the `n_rs_seq` window (=64, `model.rs:436`) and the whole donor is discarded (`queue.rs:2644`).

Changes (all in `crates/llama-agent/src/queue.rs`):
- Add a `max_rollback: usize` field to `SessionStateStore` (default `usize::MAX` = pure-attention behavior). Extend `SessionStateStore::new` (`queue.rs:143`) to accept it. Update ALL call sites so the crate compiles — including BOTH production-style constructors: `RequestQueue::new` at `queue.rs:1225` and the `#[cfg(test)] with_executor` at `queue.rs:1347`, plus the ~40 in-crate unit-test `SessionStateStore::new(...)` calls (all pass `usize::MAX`). NOTE: production wiring of the REAL window is the separate dependent task — this task defaults to MAX so non-recurrent behavior is unchanged.
- In `find_best_prefix_match`, score each candidate by `effective_reuse = if (cached_tokens.len() - lcp) <= self.max_rollback { lcp } else { 0 }`. Skip candidates scoring 0. Pick max `effective_reuse`; break ties by (a) pinned entry, then (b) caller's own session id (preserve existing tie-break), then (c) smaller rollback distance.
- Update the `reusing cached state … (lcp=… of … new tokens)` log (`queue.rs:2573`) to also emit `donor_pinned`, `donor_len`, and `rollback=donor_len-lcp`.

## Acceptance Criteria
- [ ] `SessionStateStore` carries `max_rollback`; `new()` takes it; default path uses `usize::MAX`; both `queue.rs:1225` and `queue.rs:1347` updated.
- [ ] With `max_rollback=64`, given a pinned prime donor (`tokens=[prefix]`, `len=P`) and a sibling donor (`tokens=[prefix+extra+fileA]`, `len=P+~670`) and a new prompt `[prefix+extra+fileB]`, `find_best_prefix_match` returns the PRIME (rollback 0), not the sibling.
- [ ] With `max_rollback=usize::MAX` the selector still returns the max-LCP donor — no regression to existing cross-session / continuation tests.
- [ ] The reuse log line includes donor_pinned, donor_len, rollback.

## Tests
- [ ] In `crates/llama-agent/src/queue.rs` tests (near `find_best_prefix_match_returns_cross_session_donor`, ~line 5048): add `find_best_prefix_match_prefers_zero_rollback_prime_under_recurrent_window` — RED today (selector returns the sibling), GREEN after.
- [ ] Add `find_best_prefix_match_unbounded_rollback_keeps_max_lcp` proving `usize::MAX` preserves old behavior.
- [ ] Update all `SessionStateStore::new(...)` call sites so the crate compiles.
- [ ] `cargo test -p llama-agent find_best_prefix_match` passes; full `cargo test -p llama-agent` green.

## Workflow
- Use `/tdd` — write the failing selection test first, watch it pick the sibling (RED), then implement scoring to make it pick the prime (GREEN).