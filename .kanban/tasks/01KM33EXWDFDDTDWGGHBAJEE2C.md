---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffd180
title: 'nit: ToolConfigWatcher doc comment is misplaced above Default impl, not the struct'
---
swissarmyhammer-tools/src/mcp/tool_config.rs:162-180\n\nThe doc comment explaining `ToolConfigWatcher` and its `check_and_reload` method is attached to the `impl Default for ToolConfigWatcher` block rather than the `struct ToolConfigWatcher` declaration that follows it:\n\n```rust\n/// Watches tool config files for changes...  ← doc on the Default impl\nimpl Default for ToolConfigWatcher {\n    fn default() -> Self { Self::new() }\n}\n\npub struct ToolConfigWatcher {  ← struct has no doc comment\n    ...\n}\n```\n\nThis means `cargo doc` will not render the explanation on the struct page — it will appear on the `Default` impl instead. All public items must have doc comments directly on the item itself.\n\nSuggestion: Move the doc comment to the `struct ToolConfigWatcher` line and add a minimal `/// Create a default watcher.` to the `Default` impl if desired.\n\nVerification: `cargo doc --open` shows the description on the struct, not the impl.\n\n#review-finding #review-finding