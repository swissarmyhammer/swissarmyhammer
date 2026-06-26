---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2xvxefemna564r48rpz6jp
  text: |-
    Implemented static doctor diagnostics in new crates/swissarmyhammer-expect/src/doctor.rs (TDD: 12 failing tests first, then GREEN).

    Design: diagnose(content, &DoctorFacts) -> Vec<FieldDiagnostic>. Pure static — dynamic facts (available_models, known_setup_commands) are INJECTED, so no real registry/system is hit. Production caller will populate available_models from ModelManager; this crate gained no new dependency. render(path, &diags) gives the ✗/→ human shape.

    Checks: unknown-key (Levenshtein did-you-mean, suppresses double-report of a required key a typo stands in for); required description+surface; surface/tiers/reliability/isolation validated by round-tripping through the real domain enums (allowed lists derived from the enums, never re-typed); body intent (prose vs all-mechanics) + ≥1 criterion; per-criterion checkability (vague-term + no-digit => Error with threshold suggestion). model: warning-not-error when missing. setup: Ok in known set / Error when unprovisionable / Warning when facts unavailable.

    Made spec::parse_bullet/parse_criterion pub(crate) for reuse. Verified: cargo nextest run -p swissarmyhammer-expect = 75 passed; cargo test --doc -p swissarmyhammer-expect = 4 passed; cargo clippy --all-targets -D warnings clean; fmt applied. Left in doing for review.
  timestamp: 2026-06-26T21:37:40.303006+00:00
depends_on:
- 01KW25ZK93AJ0CR17C9QJ11RXW
- 01KW261K28N0RA00X9P0APED21
position_column: doing
position_ordinal: '8280'
project: expect
title: Static doctor diagnostics for expectation specs (per-field, teaching)
---
## What
The static half of `check`: validate a `*.expect.md` without driving any system, returning structured per-field diagnostics that double as repair instructions. Per `ideas/expect.md` §"expect doctor" and §"Errors that teach".

- New `crates/swissarmyhammer-expect/src/doctor.rs`:
  - `diagnose(expectation_or_raw) -> Vec<FieldDiagnostic>` where `FieldDiagnostic { field: String, status: Ok|Warning|Error, message: String, allowed: Option<Vec<String>>, suggestion: Option<String>, line: Option<usize> }`.
  - Checks: unknown frontmatter key rejection (with did-you-mean); `description` + `surface` required; `surface`/`tiers`/`reliability`/`isolation` validated against their closed enums (list `allowed`); body must state intent AND contain ≥1 criterion (flag all-mechanics-no-intent and zero-criteria); each `Then` item must be checkable (flag "no observable signal" criteria like "feels fast", suggest a threshold).
  - **Dynamic validation**: `model:` validated against the LIVE registry via `swissarmyhammer_config::model::ModelManager::find_agent_by_name` — a missing pinned model is a **warning, not error** (grading falls back to default; golden compare catches divergence). `setup:` validated against the surface adapter / declared project: a `setup:` referencing a build target, fixture, or command that does not exist is a diagnostic (Error if it can't possibly provision, Warning if unverifiable).
  - Output formatting: a human render (the ✗/→ shape from the design example) plus the structured Vec for agent consumption.

## Acceptance Criteria
- [ ] `surfce:` typo ⇒ Error naming the key with `suggestion: "surface"` and `allowed: [cli,http,browser,gui,file,db]`.
- [ ] Missing `description` or `surface` ⇒ Error on that field.
- [ ] Body with no stated intent OR zero criteria ⇒ Error.
- [ ] A vague criterion ("the checkout feels fast") ⇒ Error with a concrete threshold suggestion; a deterministic criterion ⇒ Ok.
- [ ] `model:` not in the live registry ⇒ Warning (not Error) with available-models list + a suggestion.
- [ ] A `setup:` that references a non-existent build target/fixture/command ⇒ a diagnostic on the `setup` field (not silently accepted); a valid `setup:` ⇒ Ok.
- [ ] No system is driven and no model is consulted (pure static).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/doctor.rs` unit tests for each case above (typo, missing required, no-intent, no-criteria, vague criterion, missing model→warning, invalid `setup:`→diagnostic, valid `setup:`→ok). Mock/inject the model registry list and the surface/project facts so the tests are deterministic.
- [ ] `cargo nextest run -p swissarmyhammer-expect doctor` passes.

## Workflow
- Use `/tdd` — write the failing per-field assertion tests first.