---
assignees:
- assistant
position_column: done
position_ordinal: r0
title: Fanout watcher + startup stale cleanup
---
## What
Implement `FanoutWatcherCallback` that broadcasts file changes to multiple handlers (TS + LSP). Implement the startup stale entry cleanup pass that runs before indexers start.

Files: `swissarmyhammer-code-context/src/watcher.rs`, `swissarmyhammer-code-context/src/cleanup.rs`

Spec: `ideas/code-context-architecture.md` — "Shared file watcher" + "Startup: stale entry cleanup" sections.

## Acceptance Criteria
- [ ] `FanoutWatcherCallback` implements `WorkspaceWatcherCallback`, broadcasts to `Vec<Box<dyn WatcherHandler>>`
- [ ] On file change, clears `ts_indexed`/`lsp_indexed` flags in `indexed_files` (durable dirty state)
- [ ] Startup cleanup: walk filesystem (rayon-parallelized hashing), delete stale DB entries, mark changed files dirty, upsert current file set
- [ ] Respects `.gitignore` via `ignore` crate
- [ ] Cleanup runs synchronously before watcher and indexers start

## Tests
- [ ] Unit test: fanout broadcasts to 2 mock handlers, both receive the event
- [ ] Unit test: startup cleanup — seed DB with 3 files, delete 1 from disk, modify 1, run cleanup, verify stale removed and modified marked dirty
- [ ] Unit test: CASCADE propagation — stale file deletion removes its chunks/symbols/edges
- [ ] `cargo test -p swissarmyhammer-code-context`