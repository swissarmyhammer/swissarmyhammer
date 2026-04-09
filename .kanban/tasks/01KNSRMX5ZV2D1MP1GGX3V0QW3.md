---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa980
title: Add tests for get_callgraph location resolution and defaults
---
ops/get_callgraph.rs\n\nCoverage: 83.0% (73/88 lines)\n\nUncovered lines: 42, 44, 193, 195, 197, 199, 202, 210, 212-214, 218-221\n\nTwo areas:\n1. `CallGraphOptions::default()` (lines 42, 44) - Default trait impl\n2. `try_resolve_by_location` (lines 193-221) - parses file:line:char string, queries lsp_symbols by position\n\nTest scenarios:\n- CallGraphOptions::default() → verify default fields\n- get_callgraph with \"file:line:char\" symbol string where symbol exists → verify correct root\n- \"file:line:char\" where no symbol at those coordinates → Ok(None), fallback to name-match\n- Invalid file:line:char (non-numeric) → Ok(None)\n- fetch_edges with CallGraphDirection::Both → both inbound and outbound edges\n\n#coverage-gap #coverage-gap