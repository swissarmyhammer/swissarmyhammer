---
depends_on:
- 01KKPM87XAZYJ4K2BQKY2TQADN
position_column: done
position_ordinal: ffffffffff8580
title: Verify consumers compile unchanged
---
## What
Verify that swissarmyhammer-code-context and swissarmyhammer-treesitter compile and pass tests with zero source changes after the leader-election crate gains generics and ZMQ.

**Files**: No changes expected. If issues arise (e.g. `Send`/`Sync` bounds, re-export problems), fix them in leader-election.

**Consumers**:
- `swissarmyhammer-code-context/src/workspace.rs`: uses `elect()`, stores `LeaderGuard` and `FollowerGuard`
- `swissarmyhammer-treesitter/src/unified.rs`: uses `try_become_leader()`, stores `LeaderGuard`, re-exports types in `lib.rs` and `query/mod.rs`

## Acceptance Criteria
- [ ] `cargo check -p swissarmyhammer-code-context` passes with zero changes
- [ ] `cargo check -p swissarmyhammer-treesitter` passes with zero changes
- [ ] `cargo test -p swissarmyhammer-code-context` passes
- [ ] `cargo test -p swissarmyhammer-treesitter` passes
- [ ] `cargo test --workspace` passes

## Tests
- [ ] Run full workspace test suite
- [ ] Verify re-exports in treesitter resolve correctly