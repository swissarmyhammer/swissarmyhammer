---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvax7hxr3p34dj7b38hgvqbv
  text: |-
    Decision: chose option (a) — documentation-only, no normalization.

    Rationale: the three raw Signals fields each already have DIFFERENT ranges — bm25 is unbounded (>=0), cosine is [-1,1], and trigram is the field-weighted aggregate Σ field.weight * trigram_dice(...) over [0, Σ weights]. Normalizing trigram alone to [0,1] would NOT make the raw signals consistent with each other; it would make them MORE inconsistent (one normalized, two not) while losing information and changing fusion-input ordering. The honest, lowest-risk fix is to document each field's true range so raw-threshold consumers (project convention: thresholds read Hit.signals.*) know exactly what each value means and never assume a shared [0,1] scale.

    Changes (crates/swissarmyhammer-search/src/lib.rs, docs + test only, no behavior change):
    - Expanded the Signals struct doc comment to warn that the three fields have different ranges and are not a common [0,1] scale.
    - Documented each field: bm25 = unbounded non-negative; trigram = field-weighted aggregate [0, Σ field weights], can exceed 1.0, NOT the [0,1] Dice range; cosine = [-1,1].
    - Added a note on score_doc that trigram is the field-weighted aggregate referencing Signals::trigram.
    - Added test trigram_signal_is_field_weighted_aggregate_exceeding_one: a production-path test via search() with a doc of two weight-5 fields equal to the query, asserting Hit.signals.trigram > 1.0 (actual value 10.0). Non-vacuous — it would fail under the rejected normalization option.
  timestamp: 2026-06-17T13:44:46.776266+00:00
- actor: claude-code
  id: 01kvax7qzgh1rr8dxjxra2daw2
  text: |-
    Verification (fresh runs):
    - cargo test -p swissarmyhammer-search → 45 passed; 0 failed; 0 ignored (incl. new trigram aggregate test). Doc-tests: 0 passed/0 failed.
    - cargo clippy -p swissarmyhammer-search --all-targets -- -D warnings → clean, no warnings.

    Adversarial double-check: PASS. Confirmed docs accurate vs implementation, test is production-path and non-vacuous, no behavior change, and the two sibling working-tree changes (specs.is_empty() guard, canonical_trigram_set presence detection) remain intact. Moving to review.
  timestamp: 2026-06-17T13:44:52.976290+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb980
title: Signals.trigram raw value can exceed [0,1] — document or normalize
---
**Source:** double-check of the `search` branch. Severity: low / nit.

**File:** `crates/swissarmyhammer-search/src/lib.rs` — `score_doc` (`trigram = Σ field.weight * trigram_dice(...)`).

**Problem:** `trigram_dice` is documented as returning `[0,1]`, but the per-doc raw `Signals.trigram` is a field-weighted SUM across fields and can exceed 1.0. A raw-threshold consumer reading `Hit.signals.trigram` and expecting the `[0,1]` Dice range (the project convention is raw-value thresholds read `Hit.signals.*`) could mis-threshold. Harmless for ranking.

**Fix (pick one):** either document on `Signals::trigram` that it is a field-weighted aggregate (range `[0, Σ field weights]`, not `[0,1]`), OR normalize by total field weight so the raw value stays bounded in `[0,1]`. Prefer whichever keeps the cosine/bm25 raw signals consistent with each other.

**Acceptance:** `Signals.trigram` semantics are unambiguous (doc comment or normalization); existing search tests pass; clippy clean.