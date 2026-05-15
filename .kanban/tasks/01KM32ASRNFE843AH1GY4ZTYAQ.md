---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffbd80
title: 'clippy: ToolConfigWatcher missing Default impl (new_without_default)'
---
cargo clippy -p swissarmyhammer-tools --tests -- -D warnings fails with:\n\nerror: you should consider adding a `Default` implementation for `ToolConfigWatcher`\n  --> swissarmyhammer-tools/src/mcp/tool_config.rs:178:5\n\nFix: add `impl Default for ToolConfigWatcher { fn default() -> Self { Self::new() } }`