---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa780
title: Add tests for ts_callgraph edge cases
---
ts_callgraph.rs\n\nCoverage: 88.4% (76/86 lines)\n\nUncovered lines: 65-66, 68, 80, 95, 100, 163, 253, 285, 299\n\nMultiple functions:\n1. `extract_callee_name` (lines 65-68) - Python call node with no function/method field; non-call node\n2. Line 80 - after_dot.is_empty() → returns None (degenerate \"foo.\" input)\n3. Lines 95, 100 - parser.set_language fails → empty; parser.parse returns None → empty\n4. `resolve_callees` line 163 - empty callee_names → early return Ok(vec![])\n5. `generate_ts_call_edges` lines 285, 299 - caller_symbol=None → skip; first callee match → break\n\nTest scenarios:\n- extract_call_names with Python source → verify Python call nodes handled\n- Call with empty string after last dot (\"foo.\") → None\n- extract_call_names with invalid language → empty vec\n- resolve_callees with empty callee list → Ok(vec![])\n- generate_ts_call_edges: call site outside any chunk → skipped; multiple callees → only first edge\n\n#coverage-gap #coverage-gap