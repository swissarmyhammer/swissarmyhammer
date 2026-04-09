---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb180
title: Add tests for get_references live LSP and index edge cases
---
ops/get_references.rs\n\nCoverage: 82.7% (86/104 lines)\n\nUncovered lines: 121, 123-127, 129-130, 134-135, 137-139, 145, 176, 181, 191, 260\n\nThree areas:\n1. `try_live_lsp` (lines 113-154) - builds URI, constructs params with includeDeclaration, sends textDocument/references, parses/enriches\n2. `try_lsp_index` (lines 176, 181) - call edge conversion when call_sites is empty\n3. `try_treesitter` (line 260)\n\nTest scenarios:\n- Mock LSP returning references → verify SourceLayer::LiveLsp\n- include_declaration:false properly set in request\n- Insert call edge with empty from_ranges → verify symbol's own range used as reference location\n\n#coverage-gap #coverage-gap