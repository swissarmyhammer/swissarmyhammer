---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffbb80
title: 'Fix compile error: missing field `tool_config_watcher` in McpServer initializer at server.rs:846'
---
Compile error in `swissarmyhammer-tools/src/mcp/server.rs` line 846.\n\nThe `McpServer` struct initializer is missing the `tool_config_watcher` field, which is defined on the struct (line 90) but not populated in one of the constructor paths.\n\nError:\n```\nerror[E0063]: missing field `tool_config_watcher` in initializer of `mcp::server::McpServer`\n   --> swissarmyhammer-tools/src/mcp/server.rs:846:9\n    |\n846 |         McpServer {\n    |         ^^^^^^^^^ missing `tool_config_watcher`\n```\n\nThe field is correctly initialized in the other constructor (line 286) with:\n```rust\ntool_config_watcher: Arc::new(Mutex::new(super::tool_config::ToolConfigWatcher::new())),\n```\n\nThe initializer at line 846 needs the same field added. #test-failure