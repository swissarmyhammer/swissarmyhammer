---
position_column: done
position_ordinal: a9
title: Connect file watcher to OS notifications and LSP didChange
---
## What

`FanoutWatcher` exists in `swissarmyhammer-code-context/src/watcher.rs` with handler registration and DB dirty-marking. But:
1. No concrete `TsWatcherHandler` or `LspWatcherHandler` implementations exist (only a test `RecordingHandler`)
2. No connection to the `notify` crate for actual filesystem events
3. No `textDocument/didChange`/`didOpen`/`didClose` LSP notifications sent when files change

**Key files:**
- `swissarmyhammer-code-context/src/watcher.rs` — `FanoutWatcher`, `WatcherHandler` trait
- `swissarmyhammer-lsp/src/daemon.rs` — needs to receive file change notifications
- `swissarmyhammer-code-context/src/invalidation.rs` — 1-hop invalidation (IMPLEMENTED, needs watcher trigger)

**Approach:**
1. Add `TsWatcherHandler` that marks files dirty and triggers incremental re-parse
2. Add `LspWatcherHandler` that sends `didOpen`/`didChange`/`didClose` to running LSP servers
3. Connect `FanoutWatcher` to `notify::RecommendedWatcher` for real filesystem events
4. Wire watcher startup into leader initialization

## Acceptance Criteria
- [ ] File changes trigger automatic re-indexing of affected files
- [ ] LSP servers receive `didChange` notifications for open files
- [ ] Dirty flags set in DB trigger incremental invalidation (1-hop propagation)
- [ ] Watcher only runs in leader process

## Tests
- [ ] Unit test: file change → dirty flag set → re-index triggered
- [ ] `cargo test -p swissarmyhammer-code-context` passes