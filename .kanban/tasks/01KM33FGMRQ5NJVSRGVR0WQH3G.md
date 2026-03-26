---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8380
title: 'nit: watcher test uses tokio::test but check_and_reload is synchronous'
---
swissarmyhammer-tools/src/mcp/tool_config.rs:312-404\n\nThe three watcher tests (`test_watcher_detects_file_change`, `test_watcher_no_reload_when_unchanged`, `test_watcher_deleted_file_reverts_to_all_enabled`) are annotated `#[tokio::test]` but `check_and_reload` is a synchronous `fn` that does no async work. Using `#[tokio::test]` spins up a full Tokio runtime unnecessarily.\n\nSuggestion: Change to `#[test]` (plain synchronous test). The only async-looking operation is `std::thread::sleep` which is already used (not `tokio::time::sleep`), confirming these are sync tests.\n\nVerification: `cargo nextest run --package swissarmyhammer-tools` passes with `#[test]`.\n\n#review-finding #review-finding