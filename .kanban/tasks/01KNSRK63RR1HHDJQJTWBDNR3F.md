---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffdd80
title: Add tests for get_implementations live LSP path
---
ops/get_implementations.rs\n\nCoverage: 74.5% (41/55 lines)\n\nUncovered lines: 49, 51-52, 54-56, 60, 66, 139, 146, 185-188\n\nTwo areas:\n1. Live-LSP path (lines 48-73) - builds URI, sends textDocument/implementation, parses locations, enriches\n2. `enrich` helper (lines 185-189) + `parse_locations` Location/LocationLink branches (lines 139, 146)\n\nTest scenarios:\n- Mock LSP returning implementation results → verify SourceLayer::LiveLsp with populated implementations\n- LSP returns null → falls through to treesitter\n- parse_locations with single-object Location input\n- parse_locations with LocationLink input\n\n#coverage-gap #coverage-gap