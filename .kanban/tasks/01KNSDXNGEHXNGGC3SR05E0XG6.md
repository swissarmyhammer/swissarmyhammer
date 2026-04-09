---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: '[warning] No test for doctor check_lsp_status'
---
**File**: code-context-cli/src/doctor.rs\n\n**What**: The `check_lsp_status` method is not tested individually. The `test_run_diagnostics` test covers it indirectly (since `run_diagnostics` calls it), but there is no targeted test verifying the LSP status check produces valid checks with correct structure, unlike the pattern in `shelltool-cli/src/doctor.rs` which has `test_check_shell_tool_health`.\n\n**Why**: If `cc_doctor::run_doctor` changes its return type or behavior, the lack of a focused test means the breakage surfaces only as a confusing failure in `test_run_diagnostics` rather than pointing directly at the LSP integration.\n\n**Suggestion**: Add a `test_check_lsp_status` test that calls `doctor.check_lsp_status()` and verifies the checks are non-panicking and structurally valid.\n\n**Verify**: `cargo test -p code-context-cli doctor::tests::test_check_lsp_status`" #review-finding