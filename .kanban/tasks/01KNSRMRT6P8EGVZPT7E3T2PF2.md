---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffe180
title: Add tests for workspace_symbol_live LSP path
---
ops/workspace_symbol_live.rs\n\nCoverage: 83.5% (66/79 lines)\n\nUncovered lines: 74, 76-78, 115, 119, 121-122, 126-128, 136-137\n\nMain function: `try_live_lsp` (lines 115-142)\nSends `workspace/symbol`, handles null/empty response, parses symbols, truncates to max_results.\n\nTest scenarios:\n- Mock LSP returning workspace/symbol results → verify SourceLayer::LiveLsp, correct symbols\n- Null response → falls through to LSP index\n- Empty response → falls through\n- Results exceeding max_results → verify truncation\n\n#coverage-gap #coverage-gap