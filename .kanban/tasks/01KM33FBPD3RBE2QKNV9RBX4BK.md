---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffd580
title: 'nit: missing doc comment on pub list_tools() in server.rs'
---
swissarmyhammer-tools/src/mcp/server.rs:921-923\n\nThe public `list_tools()` method on `McpServer` has a doc comment in the block above it (lines 916-920) that is separated from the `pub async fn list_tools` signature by a blank line comment marker. The actual `pub async fn list_tools` on line 921 has no `///` doc directly attached, so `cargo doc` may not associate the comment correctly.\n\nAlso, the hot-reload path (the `ServerHandler::list_tools` impl at line 1525) has a comment starting with `/ Hot reload:` (single slash, not `//`) which is a syntax typo — it will be treated as a division operator causing a compile error, or is already silently ignored depending on context. Check this carefully.\n\nSuggestion: Fix the single-slash comment `/ Hot reload:` to `// Hot reload:`. Attach the doc comment directly to the `pub async fn list_tools` signature.\n\nVerification: `cargo build` confirms no compile warning; `cargo doc` shows the description.\n\n#review-finding #review-finding