---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2ch55cwzwkzwqsqvnqzcj1
  text: |-
    Implemented the crate via TDD (serde round-trip tests first → RED confirmed missing types → GREEN).

    Files:
    - crates/swissarmyhammer-expect/Cargo.toml (lib; deps: serde, serde_json, thiserror only — NO swissarmyhammer-tools, engine stays below tool layer)
    - src/lib.rs (re-exports all public types)
    - src/types.rs (Observation, Checkpoint, SurfaceState{Cli(CliState)|Json}, Trajectory, CriterionVerdict, ExpectationVerdict, Evidence, Reliability; enums Surface, VerdictTier, CriterionStatus, LedgerState — all #[serde(rename_all="lowercase")]; Duration serialized as explicit `duration_ms` u64)
    - src/error.rs (ExpectError via thiserror: Io, Json, Expectation{path,message})
    - root Cargo.toml: added member + [workspace.dependencies] entry

    Verification: cargo build -p swissarmyhammer-expect OK; cargo nextest run -p swissarmyhammer-expect = 11 passed/0 failed; cargo fmt applied; cargo clippy -p swissarmyhammer-expect -- -D warnings clean.
  timestamp: 2026-06-26T16:34:41.964968+00:00
- actor: claude-code
  id: 01kw2cp204hcbvzn4qqx5sjqh7
  text: 'double-check (advisory) returned REVISE on one low finding: src/lib.rs lacked the crate-root re-exports (an earlier write op had silently no-op''d, leaving only the `pub mod` declarations). Fixed via edit op — lib.rs now `pub use error::ExpectError;` and `pub use types::{...}` for all 13 public types, mirroring swissarmyhammer-validators. Re-verified after fix: build OK, clippy -D warnings clean, nextest 11/11 passed. Left in doing for /review.'
  timestamp: 2026-06-26T16:37:22.564442+00:00
- actor: claude-code
  id: 01kw2cqjefhgaf4h7sv3f43mrx
  text: 'Implemented via TDD. Created crates/swissarmyhammer-expect (lib) with src/types.rs (Observation/Checkpoint/SurfaceState{Cli,Json}/CliState/Trajectory/Evidence/CriterionVerdict/ExpectationVerdict/Reliability + enums Surface/VerdictTier/CriterionStatus/LedgerState), src/error.rs (ExpectError via thiserror), src/lib.rs re-exports. Added to root Cargo.toml workspace members + deps. Enums use serde rename_all=lowercase; Duration serializes as explicit duration_ms u64. No dep on swissarmyhammer-tools. Green: cargo nextest 11/11 pass, fmt clean, clippy -D warnings clean. NOTE for future agents: the `files` write-file op silently no-op''d twice on lib.rs during this work (returned unchanged); `edit file` worked — use edit if a write appears to not take.'
  timestamp: 2026-06-26T16:38:12.175545+00:00
- actor: claude-code
  id: 01kw2d7e6spf6y4t2vbj0rad79
  text: 'Addressed the single review finding: added a runnable `# Examples` section to the crate-level docs in `crates/swissarmyhammer-expect/src/lib.rs`. The doctest deserializes an `Observation` from wire JSON (cli SurfaceState via `{"kind":"cli",...}`, `duration_ms` key) and constructs a `Reliability { required: 3, runs: vec![true, true, true] }`, asserting `.satisfied()`. Verified: `cargo test --doc -p swissarmyhammer-expect` -> 1 passed; `cargo fmt` applied; `cargo clippy -p swissarmyhammer-expect -- -D warnings` clean. Finding checkbox flipped to [x]. Left in doing for review.'
  timestamp: 2026-06-26T16:46:52.121084+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe380
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

## Review Findings (2026-06-26 11:38)

### Nits
- [x] `crates/swissarmyhammer-expect/src/lib.rs:1` — Crate-level documentation lacks code examples. The rule requires examples showing common use cases, but the current docs only explain the crate's purpose and reference the types. Add an `# Examples` section to the crate-level docs demonstrating: (1) deserializing an `Observation` from JSON, and (2) constructing a `Reliability` and checking `.satisfied()`. For example:

```rust
/// # Examples
/// ```
/// use serde_json::json;
/// # use swissarmyhammer_expect::*;
/// let json = json!({
///   "path": "spec.yaml",
///   "checkpoints": [...],
///   "trajectory": {...}
/// });
/// let obs: Observation = serde_json::from_value(json)?;
/// 
/// let reliability = Reliability { required: 3, runs: vec![true, true, true] };
/// assert!(reliability.satisfied());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
/// ```.