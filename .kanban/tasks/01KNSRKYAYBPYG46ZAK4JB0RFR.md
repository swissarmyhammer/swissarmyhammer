---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffc980
title: Add tests for lsp_server find_executable and spawn_server
---
lsp_server.rs\n\nCoverage: 78.7% (48/61 lines)\n\nUncovered lines: 61, 114, 122-123, 125, 127, 174, 191, 194, 218, 226, 229, 232\n\nTwo areas:\n1. `find_executable` line 61 - debug log when executable IS found\n2. `spawn_server` (lines 151-235) - PATH-found and direct-executable branches: child.id(), try_wait handling\n\nTest scenarios:\n- start_lsp_server for unavailable language → started: false\n- find_executable(\"ls\") → verify Some returned\n- spawn_server with executable that exits immediately → Err(\"exited immediately\")\n- spawn_server with process that stays alive → Ok(())\n\nNote: Requires real process spawning, may need integration test with simple executables.\n\n#coverage-gap #coverage-gap