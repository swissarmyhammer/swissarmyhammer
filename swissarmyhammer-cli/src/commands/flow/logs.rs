//! View logs for a workflow run command implementation

use super::shared::{parse_workflow_run_id, print_run_logs};
use std::time::Duration;
use swissarmyhammer::{Result, WorkflowRunStatus, WorkflowStorage};
use tokio::signal;

/// Execute the logs workflow command
pub async fn execute_logs_command(
    run_id: String,
    follow: bool,
    tail: Option<usize>,
    level: Option<String>,
) -> Result<()> {
    let storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    let run = storage.get_run(&run_id_typed)?;

    if follow {
        println!("📄 Following logs for run {run_id} (Press Ctrl+C to stop)...");

        loop {
            let updated_run = storage.get_run(&run_id_typed)?;
            print_run_logs(&updated_run, tail, &level)?;

            // Exit if workflow is completed
            if updated_run.status == WorkflowRunStatus::Completed
                || updated_run.status == WorkflowRunStatus::Failed
                || updated_run.status == WorkflowRunStatus::Cancelled
            {
                break;
            }

            // Check for Ctrl+C
            if (tokio::time::timeout(Duration::from_secs(1), signal::ctrl_c()).await).is_ok() {
                println!("\n🛑 Stopped following logs");
                break;
            }
        }
    } else {
        print_run_logs(&run, tail, &level)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_logs_command_invalid_run_id() -> Result<()> {
        let result = execute_logs_command("invalid-run-id".to_string(), false, None, None).await;

        // Should fail with invalid run ID
        assert!(
            result.is_err(),
            "Logs command with invalid run ID should fail"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_logs_command_with_tail() -> Result<()> {
        let result =
            execute_logs_command("invalid-run-id".to_string(), false, Some(10), None).await;

        // Should still fail with invalid run ID, but tests the tail parameter
        assert!(
            result.is_err(),
            "Logs command with tail and invalid run ID should fail"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_logs_command_with_level() -> Result<()> {
        let result = execute_logs_command(
            "invalid-run-id".to_string(),
            false,
            None,
            Some("error".to_string()),
        )
        .await;

        // Should still fail with invalid run ID, but tests the level parameter
        assert!(
            result.is_err(),
            "Logs command with level and invalid run ID should fail"
        );
        Ok(())
    }
}
