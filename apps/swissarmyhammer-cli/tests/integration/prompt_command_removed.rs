//! Regression tests asserting the `sah prompt` CLI subcommand has been removed.
//!
//! The prompt command surface (`list` / `test` / `show` / `new` / `edit`) was
//! deleted. These tests guard against it being reintroduced: invoking `prompt`
//! must be rejected as an unknown subcommand, and it must not appear in help.

use anyhow::Result;

use crate::in_process_test_utils::run_sah_command_in_process;

/// `sah prompt` must be rejected as an unrecognized subcommand.
#[tokio::test]
async fn test_prompt_subcommand_is_unrecognized() -> Result<()> {
    let result = run_sah_command_in_process(&["prompt"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "`sah prompt` should fail now that the subcommand is removed (stdout: {}, stderr: {})",
        result.stdout, result.stderr
    );
    assert!(
        result.stderr.contains("unrecognized subcommand")
            || result.stderr.contains("invalid subcommand"),
        "stderr should mention an unrecognized subcommand, got: {}",
        result.stderr
    );

    Ok(())
}

/// `sah --help` must not advertise a `prompt` subcommand.
///
/// The check targets the subcommand listing specifically: clap renders each
/// subcommand as an indented `<name>  <description>` line. We assert no such
/// line introduces a `prompt` command. (The substring "prompt" still appears
/// legitimately inside other command descriptions, e.g. validate's, so a bare
/// substring check would be wrong.)
#[tokio::test]
async fn test_help_does_not_list_prompt_subcommand() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "help should succeed");

    let lists_prompt_subcommand = result
        .stdout
        .lines()
        .any(|line| line.trim_start().starts_with("prompt ") || line.trim() == "prompt");
    assert!(
        !lists_prompt_subcommand,
        "top-level help should no longer list a prompt subcommand, got:\n{}",
        result.stdout
    );

    Ok(())
}
