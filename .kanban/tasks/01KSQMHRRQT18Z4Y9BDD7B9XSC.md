---
assignees:
- claude-code
position_column: todo
position_ordinal: '9180'
title: code_context_mcp_e2e rebuild/detect tests time out under parallel nextest
---
crates/swissarmyhammer-tools: code_context_mcp_e2e_test::{test_mcp_detects_new_files, test_rebuild_index_is_synchronous_and_reports_stats, test_rebuild_index_layer_lsp_returns_zero_stats_with_note} plus tools_tests integration::code_context_ops_e2e::{qwen_embedding_find_duplicates_e2e, qwen_embedding_grep_code_e2e, qwen_embedding_lsp_layered_e2e} and integration::semantic_search_e2e::qwen_embedding_semantic_search_e2e.

Symptom: hit the 300s nextest timeout both in the full workspace run and when re-run isolated as a small group. These are heavy e2e tests (qwen embedding model load + LSP indexing + file watching). The rebuild/detect tests consistently hang at exactly 300s, indicating a wait that never completes (LSP/index/watcher) under contention rather than mere slowness.

Fix direction: investigate the LSP/index/watcher shared state and either serialize these e2e tests, give them isolated state, or add a real internal deadline. Confirm whether they share global LSP server / index state with each other.

NOTE: pre-existing, NOT introduced by the current llama-agent test-only branch. Surfaced by `cargo nextest run --workspace`. #test-failure