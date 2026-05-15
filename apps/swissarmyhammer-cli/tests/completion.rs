//! End-to-end integration test for `sah completion <shell>`.
//!
//! Launches the compiled `sah` binary as a child process and asserts that
//! the runtime completion subcommand emits a non-empty script that mentions
//! the binary name `sah` for every supported shell.
//!
//! The full per-shell rendering contract is implemented in
//! [`swissarmyhammer_cli_completions::test_helpers::assert_compiled_binary_completion_works`]
//! and is exercised uniformly by every CLI's integration test. This test
//! also pins the binary-name contract — the script must register under
//! `sah`, not `swissarmyhammer-cli` or any other crate-derived name.

use std::path::Path;
use swissarmyhammer_cli_completions::test_helpers::assert_compiled_binary_completion_works;

/// `sah completion <shell>` must exit 0, emit a non-empty script to stdout,
/// and mention the binary name `sah` for every supported shell.
#[test]
fn completion_succeeds_for_every_supported_shell() {
    assert_compiled_binary_completion_works(Path::new(env!("CARGO_BIN_EXE_sah")), "sah");
}
