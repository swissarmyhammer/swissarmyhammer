---
assignees:
- claude-code
depends_on:
- 01KRGMHZV8KEVPBW2BPP70D8SZ
- 01KRGMK1VTXCFZRKTMM8ESSVM2
position_column: done
position_ordinal: ffffffffffffffffffffffffc580
project: rebuild-index
title: Make `rebuild index` synchronous (await the actual rebuild)
---
Today `build status` returns immediately after `UPDATE indexed_files SET ts_indexed = 0`. The "rebuild" then happens whenever the leader's background indexing worker gets around to noticing. After this card, `rebuild index` should:

1. Reset the indexed flags for the requested layer (today's behavior)
2. Actually run the indexer to completion against the dirty files
3. Return the final stats (files indexed, chunks produced, embeddings written, elapsed)

The MCP tool call doesn't return until the work is done.

## Implementation

In `execute_rebuild_index` (renamed in the first card), after `rebuild_index(&ws.db(), layer)`:

- Build a `ProgressReporter` (no-op for now — wired up in the next card)
- Call `index_discovered_files_async(&ws, reporter).await` synchronously
- Collect `IndexProgress::Done { files, chunks, elapsed }` from the reporter (or have the indexer return it directly — pick whichever is cleaner)
- Return a `RebuildIndexResult { files_marked, files_indexed, chunks_written, elapsed_ms, layer, hint }` instead of just `files_marked`

## Caveat: don't double-run

The leader's background indexer is already watching for dirty files. If we mark dirty and the background worker picks them up at the same time as `rebuild index` is processing, we get contention. Options:

- (preferred) `rebuild index` takes a short-lived advisory lock that the background worker also respects, blocking it until we're done. Add to the existing leader mutex if there is one.
- Alternative: `rebuild index` *is* the worker run — pause the background worker, kick the rebuild, resume.

Pick the simpler one after a quick look at how the background indexer is scheduled (see `swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` and `server.rs` bootstrap).

## Tests

- E2E pattern from `swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs`: create a workspace with a couple of source files, call `rebuild index`, assert the response includes non-zero `files_indexed` and `chunks_written` *synchronously* (no sleep, no polling).
- Regression: after the call, `get status` reports `ts_indexed_percent == 100`.

## Depends on

- Rename card
- Progress event types card (passes a `NoopReporter` for now)

#code-context #indexer #rebuild-index

## Review Findings (2026-05-13 15:30)

### Warnings
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:2406` — When `layer=lsp` (and partly when `layer=both`), the synchronous indexer driven here is only the tree-sitter indexer (`index_discovered_files_async`). For `layer=lsp` the dirty bits flipped were `lsp_indexed=0`, but the tree-sitter indexer queries `WHERE ts_indexed = 0`, so the dirty set is empty and the response is always `files_indexed=0, chunks_written=0, elapsed_ms~0`. The LSP rebuild stays fire-and-forget despite the op's new "synchronous" contract — users invoking `rebuild index` with `layer=lsp` will be misled. Pick one: (a) reject `layer=lsp` with a typed error until LSP-side sync is implemented, (b) explicitly document in the response and the description that the synchronous contract applies only to the tree-sitter layer, or (c) also drive the LSP indexer to completion here.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/description.md:12` — The op description still says "rebuild index: Mark files for re-indexing by resetting indexed flags." That is now stale — the op also runs the indexer to completion and returns synchronous stats. Update the description and (ideally) document the new response shape (`files_indexed`, `chunks_written`, `elapsed_ms`) so callers know what to expect.
- [x] `swissarmyhammer-tools/tests/code_context_mcp_e2e_test.rs:344` — The new synchronous-contract test only exercises `layer=treesitter`. Add at minimum a case for `layer=both` (the default — assert non-zero `files_indexed` and `ts_indexed_percent==100`), and a case for `layer=lsp` that locks down whatever behavior is chosen for the warning above (error, zero-stats with documented contract, or real sync). Without it, a future regression on the default layer goes uncaught.

### Nits
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:2387-2395` — The "we don't add an advisory lock" comment is good. Consider adding one sentence explaining that `chunks_written` is per-run (not net-new), so concurrent rebuild/bootstrap/watcher runs may each report non-zero values for the same logical work — that's the price of the lock-free design and worth flagging for future maintainers reading the stats.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1820-1830` — The MODEL NOTE block lives in `index_discovered_files_async` (the thin async wrapper) but really describes what `build_default_embedder` does. Moving it next to `build_default_embedder` keeps the wrapper minimal and concentrates the model lore in one place.