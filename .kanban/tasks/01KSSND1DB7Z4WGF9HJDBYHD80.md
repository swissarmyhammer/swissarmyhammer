---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd280
project: command-events
title: Thread ambient txn into forward-edit store/changed emission
---
## What

Review of the Tier-2 command-events work found the "a command's N changes share one `txn` → one atomic UI re-render" contract is only met for undo/redo + the watcher, NOT for forward edits.

`EntityCache::write/delete/archive/unarchive` (`crates/swissarmyhammer-entity/src/cache.rs` ~447/479/507/553) hardcode `EventProvenance::user()` and never read the StoreContext ambient transaction (`current_transaction()`). So when a command's `execute` opens an ambient txn (the execute-bracketing task `01KS613VPH2G4ZWKZPGW9ZCJAA`, done) and the callback makes forward entity writes, those writes emit `store/changed` with `txn: null, origin: user` — uncorrelated with the command's `commands/executed` txn. The undo *stack* group is shared (push reads the ambient), but the emitted *event* txn is not. So frontend txn-batching will NOT coalesce a forward command's N writes (each has txn null → flushes as a singleton).

## Why it wasn't caught
`crates/swissarmyhammer-command-service/tests/integration/mcp_notifications_e2e.rs` publishes hand-built `McpNotification`s into the bridge rather than driving real bus→bridge flow, so it asserts correlation of canned payloads and passes despite the live gap. (Also: `undo_redo_notifies_dependents_e2e.rs`'s `World::reconcile` reimplements `reconcile_post_undo_caches` rather than calling it — divergence risk.)

## Acceptance Criteria
- [ ] Forward entity writes (`EntityCache::write/delete/archive/unarchive`) stamp the StoreContext ambient txn + an `origin` reflecting the caller when one is set, falling back to `user`/null otherwise
- [ ] A command whose `execute` makes N forward writes emits N `store/changed` all sharing the command's `txn` (matching the `commands/executed` txn)
- [ ] `mcp_notifications_e2e.rs` is strengthened to drive REAL writes through the execute-bracketed path (not publish canned events) so it would fail if the txn weren't threaded
- [ ] `undo_redo_notifies_dependents_e2e.rs` calls the production `reconcile_post_undo_caches` (or a shared helper) rather than reimplementing it

## Notes
Discovered during the Tier-2 review. Mostly latent until the command-service execute-bracketing is wired into the app bootstrap (cut-over), but the contract gap + the synthetic test should be fixed so the correlation is real once wired. Relates to `01KS613VPH2G4ZWKZPGW9ZCJAA`, `01KS5F8THM5EQMKFSF6GFAE55C`, `01KS5G3AKZXDN7K6YR415E0V4K`.