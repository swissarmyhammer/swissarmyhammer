---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa880
title: Add tests for config loading real filesystem paths
---
config.rs\n\nCoverage: 79.6% (43/54 lines)\n\nUncovered lines: 98-101, 103-104, 110, 129, 132, 161, 172\n\nMain function: `load_code_context_config()` (lines 98-111)\nUses real filesystem paths (~/.code-context/ and git root). Also `load_code_context_config_from_paths` Err warning paths.\n\nTest scenarios:\n- load_code_context_config() in a temp dir → verify returns non-empty config (builtin only)\n- load_code_context_config_from_paths with non-existent dir → Err warning logged, continues\n- merge_config_stack with all-invalid YAML entries → unwrap_or_default() returns default config\n- vfs.load_all() error path (may need a broken/unreadable directory)\n\n#coverage-gap #coverage-gap