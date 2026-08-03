//! Regression tests asserting the `sah model` CLI subcommand and the `--model`
//! global flag have been removed.
//!
//! The model command surface (`list` / `show` / `use`) was deleted when Claude
//! became the only chat executor, and the `--model` global flag went with the
//! model-name lookup it fed. The Claude CLI `--model` switch is now a plain
//! configuration field (`model:` / `review.model` in `.sah/sah.yaml`). These
//! tests guard against either being reintroduced.

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
/// line introduces a `model` command.
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

/// `sah --help` must not advertise a `--model` global flag.
///
/// The switch is a configuration field now, so a flag would be a second source
/// of truth for the same value.
#[tokio::test]
async fn test_help_does_not_list_model_global_flag() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "help should succeed");

    assert!(
        !result.stdout.contains("--model"),
        "top-level help should no longer list a --model flag, got:\n{}",
        result.stdout
    );

    Ok(())
}

/// `sah --model <value>` must be rejected as an unknown argument.
#[tokio::test]
async fn test_model_global_flag_is_unrecognized() -> Result<()> {
    let result = run_sah_command_in_process(&["--model", "haiku", "doctor"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "`sah --model` should fail now that the flag is removed (stdout: {}, stderr: {})",
        result.stdout, result.stderr
    );
    assert!(
        result.stderr.contains("unexpected argument")
            || result.stderr.contains("unrecognized")
            || result.stderr.contains("--model"),
        "stderr should reject the removed --model flag, got: {}",
        result.stderr
    );

    Ok(())
}
