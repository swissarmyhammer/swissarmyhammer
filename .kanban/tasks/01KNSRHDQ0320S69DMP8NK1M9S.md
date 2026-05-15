---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffd880
title: Add tests for get_type_definition live LSP path
---
ops/get_type_definition.rs:59-116\n\nCoverage: 23.8% (5/21 lines)\n\nUncovered lines: 70, 72-73, 75-77, 81-82, 84-86, 93-95, 101, 107\n\n```rust\nfn get_type_definition(ctx: &LayeredContext, opts: &GetTypeDefinitionOptions) -> Result<GetTypeDefinitionResult, CodeContextError>\n```\n\nThe entire live-LSP path is uncovered. When `has_live_lsp()` is true, builds a URI, sends `textDocument/typeDefinition`, parses locations, optionally reads source text, enriches with symbol info.\n\nTest scenarios:\n- Mock LSP returning a valid location response → verify parsing, source text inclusion, symbol enrichment\n- Mock LSP returning null/empty response → verify empty result\n- Mock LSP returning array-of-locations → verify multiple results\n\nRequires injecting a mock `SharedLspClient` into `LayeredContext`.\n\n#coverage-gap #coverage-gap