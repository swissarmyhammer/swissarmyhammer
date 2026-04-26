---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffd580
title: Add tests for get_definition live LSP path and LocationLink fallback
---
ops/get_definition.rs\n\nCoverage: 62.2% (28/45 lines)\n\nUncovered lines: 70, 99, 103, 105-106, 108-110, 114-116, 121, 126, 132, 167-168, 264\n\nThree areas:\n1. `try_live_lsp` (lines 99-141) - sends textDocument/definition, parses locations, reads source, enriches\n2. `try_lsp_index` line 167-168 - include_source branch reading source text for LSP index hits\n3. `try_parse_location_link` line 264 - targetRange fallback when targetSelectionRange absent\n\nTest scenarios:\n- Mock LSP returning definition → verify SourceLayer::LiveLsp\n- Null response → falls through to index\n- Insert LSP symbol, get_definition with include_source:true → verify source_text populated\n- LocationLink with only targetRange (no targetSelectionRange) → verify targetRange used\n\n#coverage-gap #coverage-gap