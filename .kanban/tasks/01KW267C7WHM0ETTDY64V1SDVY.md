---
assignees:
- claude-code
depends_on:
- 01KW263S53NJ1YWNHGPYTTWEEC
- 01KW266J4AJCJH9HM4DYFJRK09
position_column: todo
position_ordinal: b480
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