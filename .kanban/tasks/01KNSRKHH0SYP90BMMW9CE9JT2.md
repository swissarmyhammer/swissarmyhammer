---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffcf80
title: Add tests for get_diagnostics pull diagnostics path
---
ops/get_diagnostics.rs\n\nCoverage: 76.1% (51/67 lines)\n\nUncovered lines: 118-119, 122, 137, 141, 143-144, 146-147, 151-153, 158, 170, 174, 202\n\nTwo areas:\n1. `try_pull_diagnostics` (lines 137-183) - builds URI, sends textDocument/diagnostic, handles null/error response, parses diagnostics, enriches/filters\n2. `parse_diagnostics_from_result` line 202 - direct-array format (not {items:[...]})\n\nTest scenarios:\n- Mock LSP returning diagnostic response with items → verify DiagnosticsResult populated\n- Null response → Ok(None)\n- Response with `error` field → Ok(None)\n- Response as direct array vs {items:[...]} wrapper\n- Severity filtering applied correctly\n\n#coverage-gap #coverage-gap