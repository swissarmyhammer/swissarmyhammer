---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvas5ry65mqnafkaqkhj5m5h
  text: |-
    Implemented via TDD.

    RED: added regression test `no_present_signal_yields_empty_vec` (docs present, `Query::new("")`, no embedding). It failed as expected — `search()` returned 2 arbitrary zero-score hits [a, b] instead of empty.

    GREEN: added a guard at the top of `fuse_and_rank` in crates/swissarmyhammer-search/src/lib.rs — `if specs.is_empty() { return Vec::new(); }`. Chose `fuse_and_rank` over `search()` because that is the single point where the present-signal table is consumed.

    Results:
    - `cargo test -p swissarmyhammer-search`: 43 passed; 0 failed (original 42 + new test).
    - `cargo clippy -p swissarmyhammer-search --all-targets`: clean, exit 0, no warnings.
  timestamp: 2026-06-17T12:33:54.118677+00:00
- actor: claude-code
  id: 01kvas8j1bb5nzvmnzv6apvb4q
  text: 'really-done verification complete: verification commands green (43 tests pass, clippy clean) and double-check adversarial sign-off returned PASS (guard correctly placed, contract satisfied, graceful-degradation cases unaffected, test is a true regression guard). Moved to review.'
  timestamp: 2026-06-17T12:35:25.355219+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb780
title: 'search(): return empty Vec when no signal is present (contract violation)'
---
**Source:** double-check of the `search` branch (hybrid BM25+trigram+RRF+cosine fusion). Severity: medium.

**File:** `crates/swissarmyhammer-search/src/lib.rs` — `search()` / `fuse_and_rank` (the `max == 0.0` branch).

**Problem:** The `search()` doc comment promises: *"An empty corpus, or a query with no present signals, yields an empty `Vec`."* But when docs exist and NO signal is present (e.g. `Query::new("")` with no embedding, or empty query text and no doc embeddings), `specs` is empty → `weights` empty → `max = 0.0`, so every doc is emitted with `score = 0.0` and the function returns up to `top_k` arbitrary, index-ordered hits. With the default (no `min_score` floor) the caller gets meaningless zero-score results that contradict the stated contract. This "no present signals AND non-empty corpus" branch has no test — `empty_query_text_still_returns_via_other_signals` always supplies an embedding, so cosine is present and the branch is never exercised.

**Fix:** In `search()` (or at the top of `fuse_and_rank`), short-circuit and return `Vec::new()` when `specs.is_empty()`, so behavior matches the documented "yields an empty Vec."

**Test (TDD):** Add a regression test — docs present, `Query::new("")` with no embedding, assert `search(...)` returns empty.

**Acceptance:** New test fails before the fix, passes after; existing 42 search tests still pass; `cargo clippy -p swissarmyhammer-search` clean.