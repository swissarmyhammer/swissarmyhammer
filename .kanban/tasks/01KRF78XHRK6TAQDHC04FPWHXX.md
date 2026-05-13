---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc180
project: semantic-search
title: MCP-layer e2e for LSP-driven code-context ops (search_symbol / get_callgraph / get_blastradius)
---
## What

Follow-up split-off from card 01KRF4DHGBV4H00JE2NZFSMRV9. That card landed real-pipeline e2e tests for `find_duplicates` and `grep_code` (see `swissarmyhammer-tools/tests/integration/code_context_ops_e2e.rs`). The LSP-layered bundle (`search_symbol`, `get_callgraph`, `get_blastradius`, and the layered ops `get_definition` / `get_hover` / `get_references` / `get_implementations` / `get_inbound_calls` that fall through to `lsp_index`) was deferred to keep the parent card focused.

The lower-level LSP-to-DB persistence path is already covered by `swissarmyhammer-code-context/tests/integration_test.rs::test_real_lsp_document_symbols` — it spawns a real `rust-analyzer` process, drives `LspJsonRpcClient`, and asserts that `lsp_symbols` and `lsp_indexed` are populated correctly.

What is NOT covered: the **MCP-tool-layer** path. Specifically, that `ToolRegistry → code_context tool → execute_search_symbol / execute_get_callgraph / execute_get_blastradius` actually returns useful JSON when run against a workspace where the real LSP indexing worker has populated `lsp_symbols` and `lsp_call_edges`.

## Approach

Model on `code_context_ops_e2e.rs` (the file landed by the parent card):

1. Guard with `detect_rust_analyzer()` — skip with a `println!` if not installed, same pattern as `test_real_lsp_document_symbols`.
2. Create a temp Rust project (Cargo.toml + `src/lib.rs` with a known call graph — e.g. `main -> foo -> helper` and `bar -> helper` so blast-radius has something to compute).
3. Open a real `CodeContextWorkspace`. Drive `index_discovered_files_async` to populate `ts_chunks` (so `check_ts_readiness` passes — every LSP-layered op gates on it).
4. Spawn `rust-analyzer` via `Command::new`, wrap stdin/stdout in `LspJsonRpcClient`, send `initialize` + `didOpen`, then drive `spawn_lsp_indexing_worker` (or `collect_and_persist_file_symbols` directly) to write `lsp_symbols` and `lsp_call_edges`.
5. Poll with a `wait_for_*` loop (see `wait_for_lsp_symbols` in `integration_test.rs`) until edges are present, with a generous timeout (rust-analyzer init is slow on cold cache).
6. Call `search_symbol`, `get_callgraph`, and `get_blastradius` through the MCP tool registry against known symbols. Assert each returns a non-empty response that mentions the expected symbol names / files.

### Slow-test gating

The whole test will take >5s on a warm machine and 30s+ on cold cache, plus it depends on rust-analyzer being installed. Options:
- Reuse the existing `qwen_embedding_*` naming convention if the test also drives the production indexer (which it should, to satisfy `check_ts_readiness`).
- Or add a new nextest profile / filter for LSP-gated tests (e.g. `slow-lsp`) in `.config/nextest.toml`.

## Acceptance Criteria

- [ ] One e2e test in `swissarmyhammer-tools/tests/integration/` that drives a real `rust-analyzer` + real indexing pipeline and asserts `search_symbol`, `get_callgraph`, `get_blastradius` return correct results.
- [ ] Test is excluded from the default nextest profile.
- [ ] Test skips gracefully (without failing) when `rust-analyzer` is not installed.
- [ ] All existing tests still pass.

## Tests

- [ ] `cargo nextest run --profile embedding-models --ignore-default-filter -p swissarmyhammer-tools --test tools_tests <test-name>` — passes on a machine with rust-analyzer.
- [ ] Same command on a machine without rust-analyzer logs `SKIPPED` and exits 0.

## References

- Parent card: 01KRF4DHGBV4H00JE2NZFSMRV9
- Reference real-LSP pattern: `swissarmyhammer-code-context/tests/integration_test.rs::test_real_lsp_document_symbols`
- Reference real-pipeline pattern at MCP layer: `swissarmyhammer-tools/tests/integration/code_context_ops_e2e.rs` (landed by parent card)

## Review Findings (2026-05-13 11:58)

### Nits
- [x] `swissarmyhammer-tools/tests/integration/code_context_ops_e2e.rs:9` — File-level docstring intro says "this file does the same for `find_duplicates` and `grep_code`" but the file now has three tests (LSP-layered was added). Also line 35 says "Both tests therefore drive `index_discovered_files_async`" — should be "All three tests". Suggested fix: update the intro paragraph to mention `qwen_embedding_lsp_layered_e2e` alongside the other two and change "Both" → "All three" on line 35. The dedicated "## LSP-layered ops" section already covers the LSP test correctly, so this is purely staleness in the top-of-file summary.

**Resolved 2026-05-13**: Top-of-file docstring intro updated to list all three tests (`find_duplicates`, `grep_code`, and `qwen_embedding_lsp_layered_e2e`). Line 35 "Both tests" → "All three tests". Line 41 "both tests are named" → "all three tests are named" (same staleness in the embedding-model-dependency section).

Overall assessment: meaningful real-pipeline coverage of all three MCP ops. The graceful-degradation path on 0 LSP call edges is genuine — every op is asserted on JSON shape AND on data populated from `lsp_symbols` (which works regardless of `callHierarchy` support), with edge-dependent assertions correctly gated on `edge_count > 0`. Pattern matches the lower-level `test_lsp_call_edges_known_graph` integration test. `cargo check` + `cargo clippy --tests -D warnings` both clean. Default-filter exclusion confirmed via `cargo nextest list`.
