---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9a80
title: '[warning] No integration test for run_operation end-to-end'
---
**File**: code-context-cli/src/ops.rs\n\n**What**: The test suite for `ops.rs` thoroughly covers `build_args` (arg map construction) but has zero tests for `run_operation` itself. The function that actually creates the `ToolContext`, calls `tool.execute()`, and formats output is entirely untested.\n\n**Why**: `build_args` tests only prove the arg map is correctly shaped. They do not verify that:\n- The tool actually accepts these args without error\n- The JSON output mode works\n- The text extraction from `Content::Text` items works\n- The `is_error` flag correctly maps to exit code 1\n- The `working_dir` is set correctly on the context\n\nAn integration test that runs at least one simple operation (e.g., `get status` on a temp directory) end-to-end would catch regressions in the glue layer.\n\n**Suggestion**: Add at least one `#[tokio::test]` that calls `run_operation` with a `Commands::Get { command: GetCommands::Status }` variant and verifies exit code 0.\n\n**Verify**: `cargo test -p code-context-cli ops::tests::test_run_operation_get_status`" #review-finding