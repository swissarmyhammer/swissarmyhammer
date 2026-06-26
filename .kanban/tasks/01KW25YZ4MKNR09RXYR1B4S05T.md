---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: expect
title: Create swissarmyhammer-expect crate + core domain types
---
## What
Stand up the engine crate and its pure data model. Everything else in the `expect` project depends on these types.

- Create `crates/swissarmyhammer-expect/` (lib crate). Add to workspace `Cargo.toml` members and `[workspace.dependencies]`. Mirror the crate layout of `swissarmyhammer-validators` (the `review` engine's home).
- `src/lib.rs` — crate root, re-export the public types.
- `src/types.rs` — the pure domain model from `ideas/expect.md` §"The Verdict Ladder":
  - `Observation { path: String, checkpoints: Vec<Checkpoint>, trajectory: Trajectory }`
  - `Checkpoint { after: String, state: SurfaceState, duration: Duration }`
  - `SurfaceState` — enum/struct holding an adapter's authoritative read (stdout/exit/files for cli now; leave room for json body / db rows / a11y tree). Start with a cli-shaped variant + a generic JSON value.
  - `Trajectory` — what the driver did (for `observation get`); never the verdict source.
  - `CriterionVerdict { criterion, tier: VerdictTier, pass, score: Option<f32>, evidence: Vec<Evidence>, reason, confidence: Option<f32> }`
  - `ExpectationVerdict { path, criteria: Vec<CriterionVerdict>, reliability: Reliability }`
  - Enums: `Surface` (cli|http|browser|gui|file|db), `VerdictTier` (Deterministic|Tolerance|Judgment), `CriterionStatus` (pass|fail|error|escalated), `LedgerState` (approved|drifted|new|stale), `Reliability` (pass^k representation).
- All types `#[derive(Debug, Clone, Serialize, Deserialize)]` with serde rename to match the `.expect.md`/golden JSON wire forms. Use `serde` + `serde_json`; `Duration` via `serde` with explicit ms representation.
- `src/error.rs` — `ExpectError` enum (thiserror), the crate's error type.

## Acceptance Criteria
- [ ] `cargo build -p swissarmyhammer-expect` succeeds; crate is a workspace member.
- [ ] All domain types and enums above exist, are `pub`, and round-trip through `serde_json` (serialize→deserialize equals original).
- [ ] `Surface`, `VerdictTier`, `CriterionStatus`, `LedgerState` serialize to the lowercase string forms used in `ideas/expect.md` (e.g. `"cli"`, `"deterministic"`, `"drifted"`).
- [ ] No dependency on `swissarmyhammer-tools` (engine stays below the tool layer).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/types.rs` unit tests: serde round-trip for `Observation`, `ExpectationVerdict`, and each enum (assert exact JSON string for enum variants).
- [ ] `cargo nextest run -p swissarmyhammer-expect` passes.

## Workflow
- Use `/tdd` — write failing serde round-trip tests first, then define the types to make them pass.