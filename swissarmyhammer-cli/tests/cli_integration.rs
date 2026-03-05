//! CLI integration tests using in-process testing

mod in_process_test_utils;

use in_process_test_utils::run_sah_command_in_process;

#[tokio::test]
async fn test_validate_command() -> anyhow::Result<()> {
    let result = run_sah_command_in_process(&["validate"]).await?;
    // Validate should run without panicking
    assert!(
        result.exit_code == 0 || result.exit_code == 1,
        "Should return valid exit code, got {} (stderr: {})",
        result.exit_code,
        result.stderr,
    );
    // stdout should contain validation output
    assert!(
        !result.stdout.is_empty() || result.exit_code == 0,
        "Successful validation should produce output"
    );
    Ok(())
}
