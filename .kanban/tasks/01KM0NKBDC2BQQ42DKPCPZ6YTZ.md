---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
title: 'nit: `prepare_shell_command` takes `&PathBuf` instead of `&Path`'
---
swissarmyhammer-tools/src/mcp/tools/shell/process.rs (line 577)\n\nThe function signature is:\n```rust\npub(crate) fn prepare_shell_command(\n    command: &str,\n    work_dir: &PathBuf,\n    environment: Option<&std::collections::HashMap<String, String>>,\n) -> Command\n```\n\nThe Rust API guidelines say to accept `&Path` not `&PathBuf` — `&PathBuf` coerces to `&Path` anyway, but accepting `&PathBuf` forces callers to have a `PathBuf` and cannot accept a `&Path` directly. The function only calls `work_dir` methods that are available on `Path`, so `&Path` is the correct type here.\n\nThis was the same in the original code and was carried through unchanged, but it is a straightforward improvement.\n\nSuggestion: Change `work_dir: &PathBuf` to `work_dir: &Path` and add `use std::path::Path;` to imports." #review-finding