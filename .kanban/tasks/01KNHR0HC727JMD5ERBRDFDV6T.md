---
assignees:
- claude-code
position_column: todo
position_ordinal: bb80
title: 'Coverage: lsp_worker.rs — index_single_file and loop error-handling branches'
---
swissarmyhammer-code-context/src/lsp_worker.rs

Coverage: 75.9% (85/112)

The loop control flow (shutdown, idle, client-unavailable) is well-tested. The gap is in `index_single_file` and the per-file error handling inside `run_lsp_indexing_loop`.

Uncovered functions/branches:

1. `index_single_file` (lines 221-253) — The function reads a file, calls `send_did_open`, `collect_and_persist_file_symbols`, and `send_did_close`. It cannot be tested without a real LSP client, BUT the error paths and the `result.error` warning branch (lines 245-249) could be tested if the function were refactored to accept a trait instead of a concrete `LspJsonRpcClient`.

2. Per-file error handling in `run_lsp_indexing_loop` (lines 192-203) — When `index_single_file` fails, the loop calls `mark_lsp_indexed` as a fallback. The nested error path where `mark_lsp_indexed` itself fails (lines 196-200) is also uncovered.

3. The success path in the per-file loop (lines 185-189) — logging total_indexed count after successful indexing.

4. The `spawn_lsp_indexing_worker` error arm (line 103 in the thread closure) — when `run_lsp_indexing_loop` returns `Err`.

What to test:
- The per-file error + fallback-mark path can be tested by introducing a trait for the LSP client operations (or a mock). Without that refactor, the most pragmatic approach is:
  - Test `query_lsp_dirty_files` with edge cases (files whose paths end in `.rs` but contain dots elsewhere like `foo.bar.rs`).
  - Test the `extension_to_language_id` with `hh` extension (currently untested C++ header variant).
  - Test `spawn_lsp_indexing_worker` with dirty files + None client + immediate shutdown to verify the thread doesn't panic and files remain unindexed.

#coverage-gap #code-context