---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffba80
title: 'tracing-test added as dev-dependency but never used (no #[traced_test] annotations)'
---
**File**: `swissarmyhammer-shell/Cargo.toml` line 33\n**Layer**: Functionality / Dead code\n**Severity**: Low\n\nThe `tracing-test` crate was added to `[dev-dependencies]` but no test in `hardening.rs`, `performance.rs`, or `security.rs` uses the `#[traced_test]` attribute. Several tests exercise tracing log paths (e.g. `test_warn_high_overhead`, `test_log_shell_completion_no_panic`) but only check that the code does not panic -- they do not capture or assert on tracing output.\n\nEither:\n1. Remove `tracing-test` from dev-dependencies since it is unused, OR\n2. Add `#[traced_test]` to the tests that exercise warn/info/error tracing paths and add assertions on captured log output (e.g., verify that `test_log_shell_completion_no_panic` with exit code 137 actually emits the expected warning).\n\nOption 2 would make these tests meaningfully stronger. #review-finding