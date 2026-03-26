---
assignees:
- claude-code
depends_on:
- 01KM0DBV4WE1N98MN80MTNZQPB
position_column: done
position_ordinal: ffffffffbc80
title: Extract shared infrastructure
---
Move shared types into `infrastructure.rs` and `process.rs`, test helpers into `test_helpers.rs`. Re-export from mod.rs. No operations move yet. All files under `shell/` (flattened, no `execute/` subfolder).

- `shell/infrastructure.rs`: DefaultShellConfig, ShellExecuteRequest, ShellExecutionResult, OutputLimits, OutputBuffer + helpers (find_safe_truncation_point, append_to_buffer_impl, is_binary_content, format_output_content), ShellError + impls, OutputBuffer unit tests
- `shell/process.rs`: AsyncProcessGuard, OutputLineContext, all stream/spawn/format functions, AsyncProcessGuard tests
- `shell/test_helpers.rs`: TestCommandBuilder, assert_blocked, shared_tool, execute_op helpers, ResultValidator, parse_execution_result, extract_text, etc.

**Verify**: `cargo nextest run -p swissarmyhammer-tools`