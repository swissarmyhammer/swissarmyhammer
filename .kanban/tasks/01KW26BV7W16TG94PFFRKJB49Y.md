---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4a2h65nmg6re0tszhg3ga0
  text: |-
    Implemented the resolved-action replay cache (Stagehand model).

    Files:
    - crates/swissarmyhammer-expect/src/replay.rs (new): ReplayKey, CachedAction, ReplaySource{Cached,Resolved,Drifted}, ResolvedAction, ReplayCache{load,save,resolve_or_replay}. TDD'd RED->GREEN with a closure stub counting agent invocations.
    - crates/swissarmyhammer-expect/src/observe.rs: cache_path() resolver (.expect/cache/<id>.cache.json) reusing the safe-join expect_artifact_path + tests.
    - crates/swissarmyhammer-expect/src/drive.rs: observe_with_driver now resolves the driven action through the cache by default (resolve_action/replay_inputs/surface_method); agent hit only on miss/drift; drift surfaced via DRIFT_STEP_PREFIX trajectory step + tracing::warn; cache save is best-effort. Integration tests with a CountingDriver.
    - lib.rs: pub mod replay + re-exports.

    Decisions:
    - Cache key = (normalized target + method) digest for lookup; the state snapshot is stored as the drift fingerprint (Jaccard distance) so MISS vs DRIFT stay distinguishable. CachedAction carries all three (target+state+method). Domain-separated sha256 (length-prefixed) like spec_hash so "ab"+"c" != "a"+"bc".
    - Drift threshold MAX_REPLAY_DRIFT = 0.15 (Open Question 9): replay only when state similarity within threshold, else re-resolve+surface DRIFT ("a wrong cached click is worse than a slow click").
    - Persistence: COMMITTED under .expect/cache/ (like goldens, unlike gitignored received) so a fresh CI checkout replays deterministically; documented in cache_path.

    Verify: cargo nextest -p swissarmyhammer-expect = 226 passed; replay/cache/drive filter = 34 passed; cargo check --workspace ok; clippy -D warnings clean; fmt applied.
  timestamp: 2026-06-27T10:30:14.469328+00:00
- actor: claude-code
  id: 01kw4ab35tym31xb96k2sjn8ts
  text: 'Adversarial double-check: all 5 acceptance criteria verified met with non-vacuous tests; design sound (lookup-key vs state-fingerprint split, domain-separated digest, best-effort save). One low-severity finding: the cache-key test name and AC#4 wording over-claimed that the state snapshot is part of the *lookup key*, when it is deliberately the *drift fingerprint* (the correct design — keeps MISS vs DRIFT distinguishable). Resolved: renamed the test to key_is_target_plus_method_with_state_as_the_drift_fingerprint and extended it to assert that the same target+method with a far-drifted state re-resolves as Drifted (state participates as the fingerprint). Re-ran: 226 passed, clippy -D warnings clean, fmt applied. Task left green in `doing` for /review.'
  timestamp: 2026-06-27T10:34:55.034944+00:00
depends_on:
- 01KW26600YD3PM22S8F4VJJTE5
- 01KW26A5EV162ZV58HNFFJPKBB
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffd80
project: expect
title: Resolved-action cache for deterministic replay (Stagehand model)
---
## What
Turn the fuzzy agent-driven step into a fast, mostly-deterministic CI gate by caching resolved actions and replaying them without the model. Per `ideas/expect.md` §"Determinism comes from not calling the model" (the Stagehand idea — the highest-leverage item in the survey).

- New `crates/swissarmyhammer-expect/src/replay.rs`:
  - Cache each resolved action keyed by a hash of (normalized target + state snapshot + method). On replay, execute the cached action WITHOUT the model; re-resolve via the agent only on cache miss or fingerprint drift.
  - "A wrong cached click is worse than a slow click" — define the fingerprint-drift threshold (Open Question 9) so a stale action doesn't replay into a changed program and mask a regression.
  - A fallback re-resolve (cache miss/drift) is surfaced as DRIFT, never silently applied.
  - Persist the cache under `.expect/` (decide committed vs gitignored; align with the golden's reproducibility goal).
- Integrate into the agent driver path so the cached path is the default and the agent is only hit on miss/drift.

## Acceptance Criteria
- [ ] A second run with an unchanged target+state replays the cached action with NO model call.
- [ ] A fingerprint drift (changed state snapshot) triggers a re-resolve AND is surfaced as drift, not silently applied.
- [ ] Cache key is (normalized target + state snapshot + method); collisions resolve correctly.
- [ ] The cached path is default; the agent is invoked only on miss/drift.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/replay.rs` tests with a stub agent counting invocations: hit⇒0 calls, miss⇒1 call, drift⇒re-resolve+drift-surfaced.
- [ ] `cargo nextest run -p swissarmyhammer-expect replay` passes.

## Workflow
- Use `/tdd`.