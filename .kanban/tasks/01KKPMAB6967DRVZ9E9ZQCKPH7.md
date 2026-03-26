---
depends_on:
- 01KKPM9Z9M3410XCAFWY84PANW
position_column: done
position_ordinal: ffffffffee80
title: End-to-end integration tests
---
## What
Multi-process-style integration tests proving the full heb flow: leader election, proxy, publish, subscribe, SQLite persistence, leader death, follower promotion, reconnect.

**Files**: `heb/tests/integration.rs` (new)

## Acceptance Criteria
- [ ] Full publish → ZMQ delivery → SQLite persistence verified
- [ ] Leader transition: leader dies, follower promotes, bus resumes
- [ ] Events survive leader transitions (SQLite always has them)
- [ ] Discovery file lifecycle correct (created on win, removed on drop)

## Tests
- [ ] Open HebContext, publish event, verify via replay AND via subscriber
- [ ] Two contexts: leader publishes, follower subscribes and receives
- [ ] Leader drops → follower promotes → new subscriber connects → publish/subscribe works
- [ ] Events from before and after transition all present in SQLite
- [ ] Discovery file created on leader election, removed on leader drop
- [ ] `cargo test -p heb` and `cargo test --workspace` pass