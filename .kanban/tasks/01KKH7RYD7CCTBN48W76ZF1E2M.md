---
position_column: done
position_ordinal: ffbc80
title: 'fix: single DB write queue behind leader election'
---
The indexing worker opens its own `Connection::open(db_path)` (line 72 of `indexing.rs`). The LSP worker uses the leader's `SharedDb`. Two connections = write contention under WAL = `SQLITE_BUSY` = files stuck at `ts_indexed=0` = 91% stall.

**Root cause**: No centralized write queue. Multiple workers open independent connections and race for SQLite's single-writer lock.

**Fix**: Introduce a DB write queue owned by the leader. All writers (indexing worker, LSP worker, cleanup) submit write operations to the queue rather than holding their own connections. The leader election already determines who writes — the write queue makes that guarantee structural rather than implicit.

Design options:
- `mpsc::channel` of write operations processed by a dedicated writer thread
- Or a `SharedDb` wrapper that serializes all writes through a single connection + mutex

Either way: one connection, one writer, no contention. Workers become pure producers of write intents.

**Files**: `swissarmyhammer-code-context/src/indexing.rs`, `swissarmyhammer-code-context/src/lsp_worker.rs`, `swissarmyhammer-code-context/src/lib.rs`

#bug #code-context