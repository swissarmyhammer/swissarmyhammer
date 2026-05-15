---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffbe80
title: 'test: mcp::tool_config::tests::test_watcher_deleted_file_reverts_to_all_enabled fails (no Tokio runtime)'
---
Panics at swissarmyhammer-tools/src/mcp/tools/shell/state.rs:155:29\n"there is no reactor running, must be called from the context of a Tokio 1.x runtime"\n\nTest needs #[tokio::test] attribute or a Tokio runtime handle.