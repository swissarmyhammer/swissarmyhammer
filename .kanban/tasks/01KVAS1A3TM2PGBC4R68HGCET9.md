---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvatf24yh5kz5rmpbe6c8wm9
  text: 'Picked up. Confirmed the disagreement: tokenize() uses unicode_words() which strips punctuation, so a delimiter-only query like "_-_" yields no tokens -> canonical form "" -> empty canonical trigram set (trigram_dice always 0), yet raw char_trigrams("_-_") = [[''_'',''-'',''_'']] is non-empty. So the old `trigram_present = !char_trigrams(q.text()).is_empty()` marked a no-op signal present.'
  timestamp: 2026-06-17T12:56:27.038606+00:00
- actor: claude-code
  id: 01kvatf9hwf3522f431460ww02
  text: |-
    TDD: added test `degenerate_short_query_trigram_presence_matches_canonical_scoring` in lib.rs. RED -> failed with `got [Hit{id:"a",score:1.0,signals:{bm25:-0.0,trigram:0.0,...}}, ...]` proving the no-op trigram signal was being ranked. Fix: made `score::canonical_trigram_set` pub(crate) and changed lib.rs search() to `trigram_present = !score::canonical_trigram_set(q.text()).is_empty()`. GREEN -> all 44 tests pass.

    Verification (verbatim):
    - `cargo test -p swissarmyhammer-search`: test result: ok. 44 passed; 0 failed; 0 ignored. Doc-tests ok.
    - `cargo clippy -p swissarmyhammer-search --all-targets`: Finished, exit 0, no warnings.

    Did not touch the sibling `if specs.is_empty() { return Vec::new(); }` guard in fuse_and_rank (confirmed intact).
  timestamp: 2026-06-17T12:56:34.620158+00:00
- actor: claude-code
  id: 01kvatskm1emh2rap9y20s9fc2
  text: 'double-check (adversarial) returned PASS: independently confirmed RED->GREEN (reverting only the presence fix makes the new test fail; full suite 44/44 GREEN), pub(crate) correctly scoped (no external callers of canonical_trigram_set), clippy clean. Note: the `specs.is_empty()` guard in fuse_and_rank is part of the working-tree change from the sibling task (not preexisting at HEAD); this card neither added nor reverted it. Moved to review.'
  timestamp: 2026-06-17T13:02:12.609811+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb880
title: 'search(): trigram-presence detection should use canonical, not raw, trigrams'
---
**Source:** double-check of the `search` branch. Severity: low.

**File:** `crates/swissarmyhammer-search/src/lib.rs` — `search()`.

**Problem:** `trigram_present = !tokenize::char_trigrams(q.text()).is_empty()` decides presence on the RAW query text, but the actual trigram signal (`score::trigram_dice` / `canonical_trigram_set`) is computed on the CANONICAL form (`tokenize().join(" ")`), which can have a different character length. For degenerate short queries a query can be marked "present" yet always score 0 (or vice versa). It cannot misrank normal queries — it only affects whether a no-op signal participates — but the two code paths should agree on what "has trigrams" means.

**Fix:** Compute `trigram_present` from the canonical form (e.g. `!score::canonical_trigram_set(q.text()).is_empty()`, exposing a small helper if needed) so presence detection and scoring share one definition.

**Acceptance:** presence and scoring use the same canonical trigram definition; existing search tests pass; clippy clean. Consider a small unit test on a degenerate short query.