//! End-to-end integration tests for `avp completion <shell>`.
//!
//! Launches the compiled `avp` binary as a child process and asserts that
//! the runtime completion subcommand emits a non-empty script that mentions
//! the binary name `avp` for every supported shell.
//!
//! The full per-shell rendering contract is implemented in
//! [`swissarmyhammer_cli_completions::test_helpers::assert_compiled_binary_completion_works`]
//! and is exercised uniformly by every CLI's integration test.

use std::path::Path;
use swissarmyhammer_cli_completions::test_helpers::assert_compiled_binary_completion_works;

/// `avp completion <shell>` must exit 0, emit a non-empty script to stdout,
/// and mention the binary name `avp` for every supported shell.
#[test]
fn completion_succeeds_for_every_supported_shell() {
    assert_compiled_binary_completion_works(Path::new(env!("CARGO_BIN_EXE_avp")), "avp");
}
