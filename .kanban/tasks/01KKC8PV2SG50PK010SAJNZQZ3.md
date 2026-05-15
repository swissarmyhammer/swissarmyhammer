---
assignees:
- assistant
position_column: done
position_ordinal: ff8880
title: 'Real LSP integration test: rust-analyzer documentSymbol end-to-end'
---
Add test_real_lsp_document_symbols to integration_test.rs that spawns real rust-analyzer, sends initialize/didOpen/documentSymbol, verifies symbols, persists to DB, checks lsp_indexed=1."