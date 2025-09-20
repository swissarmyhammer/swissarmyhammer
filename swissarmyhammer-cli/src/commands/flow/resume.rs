//! Resume a paused workflow run command implementation

use super::shared::{
    execute_workflow_with_progress, parse_workflow_run_id, workflow_run_id_to_string,
};
use swissarmyhammer::{Result, WorkflowExecutor, WorkflowRunStatus, WorkflowStorage};
use tokio::signal;

/// Execute the resume workflow command
pub async fn execute_resume_command(run_id: String, interactive: bool, quiet: bool) -> Result<()> {
    let mut storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    // Get the run
    let mut run = storage.get_run(&run_id_typed)?;

    // Check if run can be resumed
    if run.status == WorkflowRunStatus::Completed {
        println!("❌ Cannot resume completed workflow");
        return Ok(());
    }

    if run.status == WorkflowRunStatus::Failed {
        println!("❌ Cannot resume failed workflow");
        return Ok(());
    }

    println!("🔄 Resuming workflow: {}", run.workflow.name);
    println!("🔄 From state: {}", run.current_state);

    // Set quiet mode in context for actions to use
    if quiet {
        run.context
            .insert("_quiet".to_string(), serde_json::Value::Bool(true));
    }

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Setup signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Resume workflow execution
    let execution_result = tokio::select! {
        result = execute_workflow_with_progress(&mut executor, &mut run, interactive) => result,
        _ = shutdown_rx.recv() => {
            tracing::info!("Workflow execution interrupted by user");
            run.status = WorkflowRunStatus::Cancelled;
            Ok(())
        }
    };

    // Store the updated run
    storage.store_run(&run)?;

    match execution_result {
        Ok(_) => match run.status {
            WorkflowRunStatus::Completed => {
                tracing::info!("✅ Workflow resumed and completed successfully");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            WorkflowRunStatus::Failed => {
                tracing::error!("❌ Workflow resumed but failed");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            WorkflowRunStatus::Cancelled => {
                tracing::warn!("🚫 Workflow resumed but was cancelled");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            _ => {
                tracing::info!("⏸️  Workflow resumed and paused");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
        },
        Err(e) => {
            tracing::error!("❌ Workflow resume failed: {}", e);
            run.fail();
            storage.store_run(&run)?;
            return Err(e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_resume_command_invalid_run_id() -> Result<()> {
        let result = execute_resume_command("invalid-run-id".to_string(), false, false).await;

        // Should fail with invalid run ID
        assert!(
            result.is_err(),
            "Resume command with invalid run ID should fail"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_resume_command_quiet_mode() -> Result<()> {
        let result = execute_resume_command("invalid-run-id".to_string(), false, true).await;

        // Should still fail with invalid run ID
        assert!(
            result.is_err(),
            "Resume command in quiet mode should fail with invalid run ID"
        );
        Ok(())
    }
}
