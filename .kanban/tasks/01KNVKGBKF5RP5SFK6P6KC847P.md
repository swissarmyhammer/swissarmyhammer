---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: Add tests for yaml_loader hardcoded fallback path
---
yaml_loader.rs:64-74\n\nCoverage: 72.5% (29/40 lines)\n\nUncovered lines: 64-74\n\nFunction: `load_lsp_servers()` — the fallback block when no YAML configs are found. Creates a hardcoded rust-analyzer OwnedLspServerSpec with default values.\n\nTest scenarios:\n- Call load_lsp_servers with paths that contain no .yaml/.yml files → verify returns vec with one rust-analyzer spec\n- Verify the fallback spec has correct fields (command, args, extensions, etc.)\n\n#coverage-gap #coverage-gap