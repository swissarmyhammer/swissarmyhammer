---
assignees:
- claude-code
depends_on:
- 01KW26600YD3PM22S8F4VJJTE5
- 01KW26A5EV162ZV58HNFFJPKBB
position_column: todo
position_ordinal: bd80
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