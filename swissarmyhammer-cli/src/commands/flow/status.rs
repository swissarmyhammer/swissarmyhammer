//! Check status of a workflow run command implementation

use super::shared::{parse_workflow_run_id, print_run_status};
use crate::cli::OutputFormat;
use crate::context::CliContext;
use std::time::Duration;
use swissarmyhammer::{Result, WorkflowRunStatus, WorkflowStorage};
use tokio::signal;

/// Execute the status workflow command
pub async fn execute_status_command(
    run_id: String,
    _format: OutputFormat,
    watch: bool,
    context: &CliContext,
) -> Result<()> {
    let storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    if watch {
        println!("👁️  Watching workflow run status (Press Ctrl+C to stop)...");

        loop {
            match storage.get_run(&run_id_typed) {
                Ok(run) => {
                    print_run_status(&run, context)?;

                    // Exit if workflow is completed
                    if run.status == WorkflowRunStatus::Completed
                        || run.status == WorkflowRunStatus::Failed
                        || run.status == WorkflowRunStatus::Cancelled
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Error getting run status: {}", e);
                    break;
                }
            }

            // Check for Ctrl+C
            if (tokio::time::timeout(Duration::from_secs(2), signal::ctrl_c()).await).is_ok() {
                println!("\n🛑 Stopped watching");
                break;
            }
        }
    } else {
        let run = storage.get_run(&run_id_typed)?;
        print_run_status(&run, context)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
    use swissarmyhammer_config::TemplateContext;

    async fn create_test_context() -> Result<CliContext> {
        let template_context = TemplateContext::new();
        let matches = clap::ArgMatches::default();
        CliContext::new(
            template_context,
            OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
    }

    #[tokio::test]
    async fn test_execute_status_command_invalid_run_id() -> Result<()> {
        let context = create_test_context().await?;

        let result = execute_status_command(
            "invalid-run-id".to_string(),
            OutputFormat::Table,
            false,
            &context,
        )
        .await;

        // Should fail with invalid run ID
        assert!(
            result.is_err(),
            "Status command with invalid run ID should fail"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_parse_workflow_run_id_invalid() {
        let result = parse_workflow_run_id("invalid-run-id");
        assert!(result.is_err(), "Invalid run ID should fail to parse");
    }
}
