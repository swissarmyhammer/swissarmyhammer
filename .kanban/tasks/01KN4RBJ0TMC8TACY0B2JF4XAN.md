---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffd080
title: test_log_shell_execution_no_panic and test_log_shell_completion_no_panic only assert no-panic
---
**File**: `swissarmyhammer-shell/src/security.rs` lines 1017-1036\n**Layer**: Tests / Effectiveness\n**Severity**: Low\n\nBoth `test_log_shell_execution_no_panic` and `test_log_shell_completion_no_panic` only verify the functions don't panic. They don't assert that any logging actually occurred, or that the suspicious-exit-code branch (exit code 137) actually triggers a warning. These are smoke tests, not behavioral tests.\n\nWith `tracing-test` now available as a dev-dependency (even though unused), these could be upgraded to capture tracing output and verify:\n- `log_shell_execution` emits an info-level \"Shell command execution started\" span\n- `log_shell_completion` with exit_code=137 emits a warn-level \"unusual exit code\" message\n\nThis would transform them from smoke tests into actual behavioral assertions. #review-finding