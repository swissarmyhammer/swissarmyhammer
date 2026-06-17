---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvbn3qhebd6vtsh9ft0y2zxm
  text: Picked up by /finish (scoped-batch on $kv-prefix-reuse). Keystone — driven first because the other 4 cards depend on the max_rollback field this introduces (kanban depends_on edges were dropped by a known bug, so ordering is by prose/ordinal). Starting /implement.
  timestamp: 2026-06-17T20:42:07.278207+00:00
- actor: wballard
  id: 01kvbnmf5a94ysqmxcfrkpkc9f
  text: 'Implemented (TDD). Plumbing: added `max_rollback: usize` to `SessionStateStore`; `new()` now takes it; both production constructors (RequestQueue::new, cfg(test) with_executor) and all 27 in-crate test call sites pass `usize::MAX` (behavior-preserving — crate builds clean). Selector: `find_best_prefix_match` now scores by effective_reuse (lcp if `donor_len - lcp <= max_rollback`, else skip), max over key `(effective_reuse, pinned, is_current, Reverse(rollback))` — pinned then own-session then smaller-rollback tie-breaks. Added `donor_pinned`/`donor_len` to PrefixMatch; reuse info log now emits donor_pinned, donor_len, rollback=donor_len-lcp. RED→GREEN: new `find_best_prefix_match_prefers_zero_rollback_prime_under_recurrent_window` failed (returned "sibling", max-LCP) before the scoring change, passes after; `find_best_prefix_match_unbounded_rollback_keeps_max_lcp` green throughout (no regression). Running full gate next.'
  timestamp: 2026-06-17T20:51:15.754649+00:00
- actor: wballard
  id: 01kvbnxb5dkj03fqsgfakwwdwe
  text: 'Verification gate GREEN: `cargo fmt` OK; `cargo clippy -p llama-agent -- -D warnings` clean (Finished, no warnings); `cargo test -p llama-agent --lib find_best_prefix_match` 7/7 pass (both new tests + 5 existing); full `cargo test -p llama-agent --lib` = 1110 passed, 0 failed. Adversarial double-check returned PASS with no findings (confirmed tie-break order via Candidate::key() = (effective_reuse, pinned, is_current, Reverse(rollback)); own-session tie-break preserved; zero-score skip via continue; donor_len-lcp cannot underflow; all 31 SessionStateStore::new call sites 3-arg; no model-detection/MTP scope creep). Moving to review.'
  timestamp: 2026-06-17T20:56:06.573730+00:00
- actor: wballard
  id: 01kvbq177fjy381981w6hgvdhj
  text: |-
    Addressed the two genuine in-scope review findings in crates/llama-agent/src/queue.rs (clarity-only, behavior-preserving):

    1. Warning (find_best_prefix_match selector): Lifted the nested `Candidate` struct + its `key()` preference-key method OUT of the method to module-private scope (just before `struct SessionStateStore`). Added a `Candidate::evaluate(id, entry, new_tokens, target_session_id, max_rollback) -> Option<Candidate>` constructor that folds in the prior inline filtering (no fingerprint → skip, lcp==0 → skip, rollback > max_rollback → skip). `find_best_prefix_match` is now `self.entries.iter().filter_map(Candidate::evaluate).max_by_key(|c| c.key())`. The preference key `(effective_reuse, pinned, is_current, Reverse(rollback))` is unchanged; donor_len/donor_pinned/post-touch lcp recompute intact. Flipped the Warning checkbox to [x].

    2. Nit (new test): Replaced the hardcoded rollback window `64` in `find_best_prefix_match_prefers_zero_rollback_prime_under_recurrent_window` with module-level `const RECURRENT_WINDOW_SIZE: usize = 64;` in `free_fn_unit_tests`, used at the `SessionStateStore::new(...)` arg and the rollback assert; also updated the stale "64 window" comment to reference the const. Flipped the Nit checkbox to [x].

    Did NOT touch the two struck-through FALSE-POSITIVE blockers, the prepare_streaming_kv_cache warning, or the test-budget-constants nit (all out of scope).

    Verification gate GREEN: cargo fmt OK; cargo clippy -p llama-agent -- -D warnings clean (Finished, no warnings); cargo test -p llama-agent --lib = 1110 passed, 0 failed (unchanged — refactor preserved behavior).

    Adversarial double-check: returned REVISE but the HIGH finding ("not behavior-preserving vs HEAD") is an artifact of reviewing the whole uncommitted tree against HEAD — it flagged the task's already-implemented max_rollback/Candidate/donor_pinned work (committed in this card's prior comments) as if new. My two edits are behavior-preserving relative to that already-landed code, which is the correct baseline. The LOW max_by_key-vs-first-wins tie-break finding was confirmed unobservable (HashMap iteration order is already non-deterministic both before and after; no test pins it) — left as-is to keep behavior identical per the clarity-only mandate. The LOW stale-comment finding was fixed. Moving back to review.
  timestamp: 2026-06-17T21:15:42.191256+00:00
- actor: claude-code
  id: 01kvbq2xqa826g7bnvm7aeay7z
  text: Review complete → done. Engine surfaced 2 in-scope items (Candidate extraction warning + name the 64 const nit) — both fixed and verified green (1110 lib tests pass, clippy -D warnings clean). The 2 "blocker" duplicate-definition findings were grep-verified FALSE POSITIVES (find_best_prefix_match defined once @234, the test once @5261; engine self-reported INCOMPLETE/1-of-15-failed and cited nonexistent line numbers). The prepare_streaming_kv_cache length warning + test-budget-constant nit are pre-existing code outside this task's scope. Not re-running the full engine, which would only reproduce that noise on the uncommitted tree. Acceptance criteria all met.
  timestamp: 2026-06-17T21:16:37.994492+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbd80
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
- [x] `SessionStateStore` carries `max_rollback`; `new()` takes it; default path uses `usize::MAX`; both `queue.rs:1225` and `queue.rs:1347` updated.
- [x] With `max_rollback=64`, given a pinned prime donor and a longer sibling donor, `find_best_prefix_match` returns the PRIME (rollback 0).
- [x] With `max_rollback=usize::MAX` the selector still returns the max-LCP donor — no regression.
- [x] The reuse log line includes donor_pinned, donor_len, rollback.

## Tests
- [x] `find_best_prefix_match_prefers_zero_rollback_prime_under_recurrent_window` — RED (returned "sibling") → GREEN (returns "prime", lcp=200).
- [x] `find_best_prefix_match_unbounded_rollback_keeps_max_lcp` — green (no regression).
- [x] All `SessionStateStore::new(...)` call sites updated (27 sites).
- [x] `cargo test -p llama-agent --lib` → 1110 passed; clippy -D warnings clean.

## Workflow
- Use `/tdd` — write the failing selection test first, watch it pick the sibling (RED), then implement scoring to make it pick the prime (GREEN).

## Review Findings (2026-06-17 15:56)

> ⚠️ Engine self-reported 1/15 review tasks failed — results INCOMPLETE.

**Orchestrator verification (do not chase ghosts):** The two "Blockers" below are FALSE POSITIVES — verified by grep that `find_best_prefix_match` is defined exactly once (`queue.rs:234`) and `find_best_prefix_match_picks_deepest_lcp_across_sessions` exactly once (`queue.rs:5261`); the cited line numbers (730, 2100) do not contain those definitions, and Rust could not have compiled 1110 passing tests with real same-scope duplicates. Dismiss both. The two "Warnings" about `prepare_streaming_kv_cache` (~180 lines) and the test-budget constants are PRE-EXISTING code this task did not introduce → out of scope; do not address here. Genuine, in-scope items to fix: the nit naming the `64` rollback window as a test const, and (optional, borderline) extracting the `Candidate` struct/scoring helper out of `find_best_prefix_match` for clarity.

### Blockers
- [ ] ~~`crates/llama-agent/src/queue.rs:730` — duplicate `find_best_prefix_match_picks_deepest_lcp_across_sessions`~~ FALSE POSITIVE (defined once at 5261).
- [ ] ~~`crates/llama-agent/src/queue.rs:2100` — duplicate `find_best_prefix_match`~~ FALSE POSITIVE (defined once at 234).

### Warnings
- [x] `crates/llama-agent/src/queue.rs:234` — find_best_prefix_match is ~85 lines with a nested `Candidate` struct. Consider moving `Candidate` + its preference-key method out as a standalone type and simplifying the function to call a pure scoring helper. (In scope — added by this task.)
- [ ] ~~`crates/llama-agent/src/queue.rs:2581` — prepare_streaming_kv_cache ~180 lines~~ OUT OF SCOPE (pre-existing; this task only touched the log line).

### Nits
- [ ] ~~`crates/llama-agent/src/queue.rs:5021` — test budgets (100, 25) as named constants~~ OUT OF SCOPE (pre-existing test pattern).
- [x] `crates/llama-agent/src/queue.rs:5144` — extract the hardcoded rollback window `64` in the new test to a named const (e.g. `RECURRENT_WINDOW_SIZE`). (In scope — added by this task.)