---
position_column: done
position_ordinal: ffffffffffa780
title: BusMessage trait + make election types generic
---
## What
Add `BusMessage` trait and `NullMessage` to leader-election crate. Make `LeaderElection`, `LeaderGuard`, `FollowerGuard`, `ElectionOutcome` generic over `M: BusMessage` with `NullMessage` as the default type parameter. Existing consumers compile unchanged.

**Files**: `swissarmyhammer-leader-election/src/bus.rs` (new), `src/election.rs`, `src/lib.rs`

## Acceptance Criteria
- [ ] `BusMessage` trait with `topic()`, `to_frames()`, `from_frames()` exists
- [ ] `NullMessage` implements `BusMessage` as a no-op
- [ ] All election types have `<M: BusMessage = NullMessage>` parameter
- [ ] `cargo test --workspace` passes with ZERO changes to any consumer crate
- [ ] Bare `LeaderElection` (no angle brackets) still resolves correctly

## Tests
- [ ] `NullMessage` round-trip through `BusMessage` trait
- [ ] All existing election tests pass unchanged
- [ ] `cargo check -p swissarmyhammer-code-context` passes
- [ ] `cargo check -p swissarmyhammer-treesitter` passes