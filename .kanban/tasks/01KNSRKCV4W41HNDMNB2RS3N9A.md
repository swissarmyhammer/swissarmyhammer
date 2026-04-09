---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb280
title: Add tests for lsp_worker indexing loop
---
lsp_worker.rs\n\nCoverage: 75.9% (85/112 lines)\n\nUncovered lines: 103, 127-128, 149-150, 161-163, 167-168, 178, 181, 185, 192-193, 195-196, 202, 207, 221, 228, 232, 235-237, 241, 245\n\nMain function: `run_lsp_indexing_loop` (lines 114-213)\n- Worker loop: queries dirty files, processes via index_single_file, marks indexed\n- index_single_file: reads file, sends didOpen, collects/persists symbols, closes doc\n\nTest scenarios:\n- run_lsp_indexing_loop with shutdown flag set immediately → worker exits cleanly\n- Worker loop with no dirty files → idles (sleep + continue)\n- index_single_file with mock LSP client and real temp file → verify symbol persistence\n- Poison recovery on mutex lock (line 161-163)\n- Client unavailable (None) → skip path (line 167-168)\n\n#coverage-gap #coverage-gap