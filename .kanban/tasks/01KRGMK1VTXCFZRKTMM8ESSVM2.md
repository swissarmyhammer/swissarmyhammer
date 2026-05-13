---
assignees:
- claude-code
depends_on:
- 01KRGMHZV8KEVPBW2BPP70D8SZ
position_column: done
position_ordinal: ffffffffffffffffffffffffc480
project: rebuild-index
title: Indexer progress event types and reporter trait
---
Add structured progress events to `swissarmyhammer-code-context` (or wherever the indexer lives — see MEMORY.md, the live indexer is in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs::index_discovered_files_async`). The indexer must never format a progress string itself — it emits typed events, consumers choose how to render.

## Event type

```rust
pub enum IndexProgress {
    Discovering { found: u64 },
    Chunking { file: PathBuf, done: u64, total: u64 },
    Embedding { batch: u64, batches: u64, chunks_in_batch: u64 },
    Done { files: u64, chunks: u64, elapsed: Duration },
}
```

The `total`/`batches` fields are best-effort — `0` when unknown, populated once discovery finishes.

## Reporter trait

```rust
pub trait ProgressReporter: Send + Sync {
    fn report(&self, event: IndexProgress);
}

// no-op default — existing callers (MCP bootstrap, watcher) use this so they don't have to change
pub struct NoopReporter;
impl ProgressReporter for NoopReporter { fn report(&self, _: IndexProgress) {} }
```

The trait is intentionally tiny and synchronous — events are emitted from inside indexing tasks, so we can't have an `async fn report`. Reporter impls that need to do async work (sending JSON-RPC notifications, redrawing TUIs from a tokio task) should buffer via an `mpsc` channel internally.

## Wiring

Refactor `index_discovered_files_async(...)` to take `reporter: Arc<dyn ProgressReporter>`. Existing call sites (`server.rs`, `watcher.rs`) pass `Arc::new(NoopReporter)`. Emit events at the obvious spots:
- before discovery (`Discovering { found: 0 }`)
- after discovery completes (`Discovering { found: N }`)
- per file chunked (`Chunking { file, done, total }`)
- per embedding batch (`Embedding { batch, batches, chunks_in_batch }`)
- at the end (`Done { ... }`)

## Tests

- Unit: a vec-collecting test reporter; run the indexer end-to-end on a tiny workspace fixture, assert the recorded event sequence matches the expected shape (final event is `Done`, `Chunking.done` monotonically increases, etc.). Pattern after the existing `semantic_search_e2e.rs` integration test.
- Make sure `NoopReporter` compiles and is exported.

## Out of scope

- The MCP `notifications/progress` mapper (next card)
- The CLI TUI (later card)

#code-context #indexer #rebuild-index

## Review Findings (2026-05-13 09:37)

### Warnings
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1910-1928` — Lifecycle asymmetry on the DB-query-failure early return. All other paths emit two `Discovering` events (open `found: 0`, then post-discovery `found: N`) before any other event. On DB query failure the function emits only the first `Discovering { found: 0 }` and then jumps straight to `Done`, skipping the second `Discovering`. A consumer that relies on "second Discovering means discovery completed" will never see the signal on this path. Two reasonable fixes: (a) emit `Discovering { found: 0 }` immediately before the early `Done` in the error branch so every path has exactly two Discovering events, or (b) tighten the trait/event docstrings to make explicit that the only universally guaranteed terminal signal is `Done` and the second `Discovering` is best-effort. Either way, add a regression test that drives the DB-failure path (e.g. by closing/poisoning the shared connection) and asserts the event sequence the design now mandates. **Resolved:** picked option (a) — added a `Discovering { found: 0 }` emission right before the early `Done` on the DB-failure path with an explanatory comment, plus a regression test (`test_indexer_db_query_failure_still_emits_framing_events`) that drops the `indexed_files` table to drive the SQL-prepare failure branch and asserts the exact three-event lifecycle `Discovering(0), Discovering(0), Done(0,0,_)`.

### Nits
- [x] `swissarmyhammer-code-context/src/lib.rs:123` — `pub use progress::{...}` is wedged between `pub use ops::status::{...}` (line 119) and `pub use ops::workspace_symbol_live::{...}` (line 124), breaking the `ops::*` grouping. Move the `progress::` re-export below the last `ops::` line (after line 127) so the `ops::` block stays contiguous. **Resolved:** moved the `pub use progress::{...}` line below the last `ops::` re-export so the `ops::` block is contiguous.
- [x] `swissarmyhammer-code-context/src/lib.rs:115` vs `src/progress.rs:34` — Two very similarly named types are now exported from the crate: `IndexingProgress` (a snapshot status carrying file counts, returned by `search_code`) and `IndexProgress` (the event enum added in this task). Both surface in `pub use` at the crate root and a downstream caller importing both will confuse them. Consider renaming the new enum to something less collision-prone (e.g. `IndexProgressEvent`) or at least adding a `// NOTE: distinct from IndexingProgress in ops::search_code` comment on the `pub use progress::` line. **Resolved:** kept the `IndexProgress` name (it's the public surface area established by the task spec and already wired into tests and consumers) and added a `// NOTE: distinct from IndexingProgress (snapshot status) ...` disambiguation comment immediately above the `pub use progress::` re-export.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:3030` — Stale comment: `// These tests require access to index_discovered_files_async (pub(crate))`. The function is now `pub` (line 1810), not `pub(crate)`. Either update the comment to reflect the current visibility or remove the parenthetical entirely. **Resolved:** removed the stale `(pub(crate))` parenthetical from the comment.