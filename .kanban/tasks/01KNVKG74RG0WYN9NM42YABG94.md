---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffe580
title: Add tests for supervisor project detection logging
---
supervisor.rs:64-65\n\nCoverage: 96.1% (49/51 lines)\n\nUncovered lines: 64-65\n\nFunction: `start_servers()` — lines 64-65 are the `info!` log and the loop body that iterates detected projects and collects unique server specs.\n\nTest scenarios:\n- Call start_servers with a workspace containing detectable projects → verify the info log fires and specs are collected\n\nNote: This may require a temp dir with a Cargo.toml to trigger project detection.\n\n#coverage-gap #coverage-gap