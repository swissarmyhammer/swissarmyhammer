---
assignees:
- claude-code
position_column: todo
position_ordinal: b380
project: diagnostics
title: Abort follower diagnostics-bus subscriber on promotion
---
## What
`spawn_follower_diagnostics_subscriber` (in `crates/swissarmyhammer-tools/src/mcp/server.rs`) starts a `tokio::task::spawn_blocking` loop running `swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus`. It runs for the process lifetime and is NOT stopped when the follower is promoted to leader.

Because the bus backend address is deterministic by workspace hash, after a follower→leader promotion the orphaned subscriber silently reconnects to the (now own) proxy and debug-logs the leader's own diagnostics. A ZMQ ipc disconnect surfaces as `EAGAIN`, not the `Disconnected` error string the loop breaks on, so the loop only ends at process/context teardown.

## Impact (low)
One lingering blocking task per promoted-follower + benign self-logging at debug. No wrong output, no deadlock, no hot spin (the loop blocks on a 500ms `recv_timeout`). Surfaced as an advisory by double-check on ^cxz8vs4 (cross-process fan-out + leader watcher); intentionally deferred there to avoid scope creep.

## Fix
Thread a cancellation signal into `subscribe_diagnostics_over_bus` (e.g. an `Arc<AtomicBool>` / `tokio_util::CancellationToken` checked each loop iteration) so the follower subscriber can be stopped. `spawn_blocking` tasks cannot be force-aborted mid-`recv`, so the loop must cooperatively check the flag (it already wakes every ≤500ms on `recv_timeout`). Have the re-election loop (`handle_promotion_result` / `spawn_reelection_loop` in server.rs) signal cancellation when the workspace is promoted, before the cold re-spawn starts the leader-side publish path.

## Acceptance Criteria
- [ ] A follower-started diagnostics-bus subscriber stops within ~1 loop interval of promotion (no lingering blocking task, no self-logging of the new leader's own diagnostics).
- [ ] Unit/integration coverage for the cancellation (the subscriber loop exits when the flag is set).

#diagnostics