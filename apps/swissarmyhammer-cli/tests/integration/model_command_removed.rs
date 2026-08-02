//! Regression tests asserting the `sah model` CLI subcommand has been removed.
//!
//! The model command surface (`list` / `show` / `use`) was deleted when Claude
//! became the only chat executor. These tests guard against it being
//! reintroduced: invoking `model` must be rejected as an unknown subcommand,
//! and it must not appear in help.

use anyhow::Result;

use crate::in_process_test_utils::run_sah_command_in_process;

/// `sah model` must be rejected as an unrecognized subcommand.
#[tokio::test]
async fn test_model_subcommand_is_unrecognized() -> Result<()> {
    let result = run_sah_command_in_process(&["model"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "`sah model` should fail now that the subcommand is removed (stdout: {}, stderr: {})",
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

/// `sah --help` must not advertise a `model` subcommand.
///
/// The check targets the subcommand listing specifically: clap renders each
/// subcommand as an indented `<name>  <description>` line. We assert no such
/// line introduces a `model` command. A bare substring check would be wrong,
/// because the `--model` global flag legitimately keeps its own help line.
#[tokio::test]
async fn test_help_does_not_list_model_subcommand() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "help should succeed");

    let lists_model_subcommand = result
        .stdout
        .lines()
        .any(|line| line.trim_start().starts_with("model ") || line.trim() == "model");
    assert!(
        !lists_model_subcommand,
        "top-level help should no longer list a model subcommand, got:\n{}",
        result.stdout
    );

    Ok(())
}
