---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffcc80
title: 'nit: All `return` statements in `mod.rs` dispatch arms are redundant'
---
swissarmyhammer-tools/src/mcp/tools/shell/mod.rs (lines 143-170)\n\nEvery arm in the `match op_str` block uses an explicit `return`, making the match a statement rather than an expression:\n```rust\nmatch op_str {\n    \"execute command\" | \"\" => {\n        return execute_command::execute_execute_command(...).await;\n    }\n    ...\n    other => {\n        return Err(McpError::invalid_params(...));\n    }\n}\n```\n\nSince the match is the last statement in the function and every arm returns, this should be written as a match expression without `return`:\n```rust\nmatch op_str {\n    \"execute command\" | \"\" => execute_command::execute_execute_command(...).await,\n    ...\n    other => Err(McpError::invalid_params(...)),\n}\n```\n\nSuggestion: Remove redundant `return` keywords and use the match as an expression. This is idiomatic Rust per Clippy's `needless_return` lint." #review-finding