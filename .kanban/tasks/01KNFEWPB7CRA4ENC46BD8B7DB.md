---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe980
title: Inconsistent file_path_to_uri for relative paths across ops
---
swissarmyhammer-code-context/src/ops/get_implementations.rs vs get_hover.rs\n\nThe `file_path_to_uri` implementations handle relative paths differently:\n\n- get_hover.rs (and get_diagnostics.rs copy): resolves relative paths via `std::env::current_dir().join(path)` producing `file:///absolute/path`\n- get_implementations.rs: prepends `file:///` directly to relative paths, producing `file:///relative/path` which is an invalid file URI\n\nThe get_implementations.rs version:\n```rust\nfn file_path_to_uri(path: &str) -> String {\n    if path.starts_with('/') {\n        format!(\"file://{}\", path)\n    } else {\n        format!(\"file:///{}\", path)  // BUG: invalid URI\n    }\n}\n```\n\nThis means get_implementations will produce invalid URIs for relative paths, causing the LSP server to fail silently or return no results.\n\nSuggestion: Use the cwd-based resolution from get_hover.rs consistently, or better yet, resolve relative paths to workspace-root-relative absolute paths at the MCP handler layer before passing to ops." #review-finding