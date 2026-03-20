---
depends_on:
- 01KKPM7NJR5C0279ATM51B6KGR
position_column: done
position_ordinal: ffffffffff8480
title: Wire bus into LeaderGuard and FollowerGuard
---
## What
Integrate proxy, discovery, and publisher into the election lifecycle. Leader wins → starts proxy, writes discovery file, gets publisher. Follower loses → reads discovery file, gets publisher. Both expose `publish()`. Drop cleans up everything.

**Files**: `swissarmyhammer-leader-election/src/election.rs` (major changes), `src/lib.rs`

**Critical details**:
- `LeaderGuard<M>` gains: `ProxyHandle`, `Publisher<M>`, `zmq::Context`, `discovery_path`
- `FollowerGuard<M>` gains: `Publisher<M>`, `zmq::Context`
- `LeaderGuard::drop()` must stop proxy BEFORE releasing flock (field order matters)
- `try_promote()` must start new proxy + write new discovery file
- No discovery file = no leader = election always contested (design invariant)
- For `NullMessage`, proxy still runs but publish is effectively a no-op

## Acceptance Criteria
- [ ] Leader election starts proxy and writes discovery file automatically
- [ ] Both guards expose `publish(&self, msg: &M) -> Result<()>`
- [ ] `LeaderGuard::drop()` stops proxy, removes discovery + socket files
- [ ] `try_promote()` starts new proxy on successful promotion
- [ ] `cargo test --workspace` passes (consumers unchanged)

## Tests
- [ ] Leader publishes, separate subscriber receives
- [ ] Follower publishes through leader's proxy
- [ ] Leader drops → discovery file removed, socket files removed
- [ ] Follower promotes → new proxy starts, new discovery file written
- [ ] All existing election tests still pass