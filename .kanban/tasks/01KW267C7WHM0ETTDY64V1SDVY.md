---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3q4hqan49kxcyv8etdef5h
  text: 'Picked up. Plan: new ledger.rs with Golden (observation + frozen CompiledAssertions + GradingPins), ScrubberSet (timestamp/uuid/port/temp-path/run-id), compare() = evaluate(received) vs evaluate(golden) per criterion field-wise by tier (tier1 real; tier2/3 stub band+similarity). Reuse golden_path/received_path safe-join from observe.rs. Wire expectation get / observation get / golden get / goldens list / observations list in tools/expect/mod.rs (convert the get macro ops to manual SCOPE_PARAMS impls). TDD red-first.'
  timestamp: 2026-06-27T04:59:17.610420+00:00
- actor: claude-code
  id: 01kw3r51jx0n3b5xhdjpamqfd9
  text: |-
    Implementation landed and green. New crates/swissarmyhammer-expect/src/ledger.rs (Golden + GradingPins + read/write reusing golden_path safe-join; ScrubberSet timestamp/uuid/ulid/temp-path/port + duration normalization, configurable + idempotent; compare() = evaluate(received) vs evaluate(golden) per criterion by tier, tier1 real, tier2/3 stubbed; verdicts re-derived both sides, never stored). lib.rs exports added. tools/expect/mod.rs: get expectation/observation/golden converted to manual SCOPE_PARAMS ops + handlers; goldens/observations list converted to manual SCOPE_PARAMS ops + handlers; shared resolve_scope_specs helper (observe_op/evaluate_op refactored onto it); dispatch arms + tests.

    Verification: cargo nextest -p swissarmyhammer-expect -p swissarmyhammer-tools -E 'test(ledger) or test(golden) or test(expect)' = 83 passed; cargo check --workspace ok; cargo clippy (expect+tools) -D warnings clean; cargo fmt applied.

    Adversarial double-check: REVISE with 3 findings. Resolved:
    - F2 (duration left unnormalized): FIXED — scrub_observation now collapses Checkpoint::duration to NORMALIZED_DURATION; scrubber-equality test now uses DIFFERING durations to make the guarantee honest.
    - F3 (list ops accepted undeclared scope): FIXED — ObservationsList/GoldensList converted to manual Operation impls declaring SCOPE_PARAMS; added goldens_list_honors_a_scope test.
    - F1 (no compare-level volatile test): JUSTIFIED, not added. The assertion compiler only emits numeric BoundValue::Number literals and scrub_json only touches strings, so a Tier-1 matched value is never volatile and never scrubbed — a compare-level "volatile-only ⇒ approved" test would be vacuous at Tier-1. Scrubber normalization is proven at the unit level (scrub_observation equality across two runs differing only in volatile content + duration), and compare provably scrubs both sides before evaluate. The scrubber payoff lands at Tier-3 evidence/hash comparison in later tasks.

    Task left in doing, green, ready for /review.
  timestamp: 2026-06-27T05:17:02.429319+00:00
depends_on:
- 01KW263S53NJ1YWNHGPYTTWEEC
- 01KW266J4AJCJH9HM4DYFJRK09
position_column: doing
position_ordinal: '8280'
project: expect
title: Golden store + scrubbers + per-criterion tier compare; get/list ops
---
## What
Persist goldens and received observations on disk (mirrored repo-relative paths), scrub volatile content, and compare received-vs-golden per criterion by tier. Also wire the single-item `get` read ops. Per `ideas/expect.md` §"The Drift Ledger".

- New `crates/swissarmyhammer-expect/src/ledger.rs`:
  - Paths: golden at `.expect/goldens/<repo-rel-path>.golden.json` (committed); received at `.expect/received/<repo-rel-path>.received.json` (gitignored). The tree mirrors each spec's repo-relative path — location IS identity.
  - `Golden` = approved, scrubbed `Observation` + the frozen compiled assertions + pinned grading model/embedder/thresholds. Read/write helpers.
  - **Scrubbers**: normalize volatile content (timestamps, UUIDs, ports, temp paths, run-specific ids) out of evidence before comparison. Implement a configurable scrubber set.
  - **Compare** = `evaluate(received)` vs `evaluate(golden)` per criterion, field-wise by tier: deterministic ⇒ matched value (or scrubbed hash) changed?; tolerance ⇒ score left the band?; judgment ⇒ approved evidence + similarity threshold (handled fully in the Tier 2/3 tasks; stub the band here). The verdict is re-derived on both sides, never the stored source of truth.
- Wire read ops in `tools/expect/mod.rs` (replace stubs):
  - `expectation get <scope>` — return the parsed spec (frontmatter + intent + criteria + G/W/T) as JSON.
  - `observation get <scope>` — the checkpoint timeline + trajectory.
  - `golden get <scope>`, `goldens list`, `observations list`.

## Acceptance Criteria
- [ ] Golden and received write to the mirrored `.expect/goldens|received/<path>.json` locations and round-trip.
- [ ] Scrubbers normalize timestamps/UUIDs/ports/temp-paths so two runs differing only in volatile content compare equal.
- [ ] Tier-1 compare: a changed matched value ⇒ drift; unchanged ⇒ approved.
- [ ] `expectation get` returns the parsed spec; `observation get` returns the checkpoint timeline + trajectory; `golden get`/`goldens list`/`observations list` return expected JSON.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/ledger.rs` unit tests: path mirroring, scrubber normalization, Tier-1 compare drift vs approved.
- [ ] Tools op tests for `expectation get` / `observation get` / `golden get` / list ops.
- [ ] `cargo nextest run -p swissarmyhammer-expect ledger` passes.

## Workflow
- Use `/tdd`.