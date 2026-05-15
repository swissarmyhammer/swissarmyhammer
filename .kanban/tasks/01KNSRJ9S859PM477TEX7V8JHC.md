---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffe380
title: Add tests for get_inbound_calls live LSP and cross-reference paths
---
ops/get_inbound_calls.rs\n\nCoverage: 60.6% (83/137 lines)\n\nUncovered lines: 90, 124, 129, 132-133, 135-137, 141-143, 148, 157, 186, 208, 215-216, 219, 221-223, 229-232, 235, 238, 240-244, 247, 260, 276, 312, 319-322, 328, 330-338, 345, 377, 396, 410\n\nUncovered functions:\n1. `try_live_lsp` (lines 124-167) - sends prepareCallHierarchy, parses response, calls fetch_incoming_calls_recursive\n2. `fetch_incoming_calls_recursive` (lines 208-261) - callHierarchy/incomingCalls with recursion\n3. `cross_reference_with_index` (lines 312-346) - merges LSP index callers not found by live LSP\n4. `try_lsp_index` (lines 356-385) - return path when callers exist\n5. `try_treesitter` - treesitter-sourced call edges\n\nTest scenarios:\n- Mock LSP returning prepareCallHierarchy + incomingCalls → verify callers\n- cross_reference_with_index: insert LSP symbol + call edge, call with empty callers → verify indexed caller appended\n- try_lsp_index: insert symbol + call edge → verify Some(InboundCallsResult)\n- try_treesitter: insert ts_chunk + treesitter edge → verify SourceLayer::TreeSitter\n\n#coverage-gap #coverage-gap