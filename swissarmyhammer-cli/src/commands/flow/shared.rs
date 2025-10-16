//! Shared utilities for flow command subcommands

use swissarmyhammer::{Result, SwissArmyHammerError, WorkflowRunId, WorkflowRunStorageBackend};
use swissarmyhammer::{WorkflowExecutor, WorkflowRunStatus};
use swissarmyhammer_workflow::{ExecutorError, WorkflowRun};

/// Helper to convert WorkflowRunId to string
pub fn workflow_run_id_to_string(id: &WorkflowRunId) -> String {
    id.to_string()
}

/// Convert ExecutorError to SwissArmyHammerError
pub fn handle_executor_error(
    executor_error: ExecutorError,
    _context: &str,
) -> SwissArmyHammerError {
    // Convert ExecutorError directly to SwissArmyHammerError using From trait
    swissarmyhammer_common::SwissArmyHammerError::from(executor_error)
}

/// Execute workflow with progress display and interactive support
pub async fn execute_workflow_with_progress(
    executor: &mut WorkflowExecutor,
    run: &mut WorkflowRun,
    interactive: bool,
) -> Result<()> {
    if interactive {
        println!("🎯 Interactive mode - press Enter to continue at each step");

        while run.status == WorkflowRunStatus::Running {
            println!(
                "📍 Current state: {} - {}",
                run.current_state,
                run.workflow
                    .states
                    .get(&run.current_state)
                    .map(|s| s.description.as_str())
                    .unwrap_or("Unknown state")
            );

            println!("Press Enter to execute this step...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            // Execute single step
            executor.execute_state(run).await.map_err(|e| {
                handle_executor_error(
                    e,
                    &format!("Failed to execute state '{}'", run.current_state),
                )
            })?;

            println!("✅ Step completed");

            if run.status != WorkflowRunStatus::Running {
                break;
            }
        }
    } else {
        // Non-interactive execution
        executor.execute_state(run).await.map_err(|e| {
            handle_executor_error(
                e,
                &format!(
                    "Failed to execute workflow '{}' at state '{}'",
                    run.workflow.name, run.current_state
                ),
            )
        })?;
    }

    Ok(())
}

/// Create local workflow run storage backend
pub fn create_local_workflow_run_storage() -> Result<Box<dyn WorkflowRunStorageBackend>> {
    use std::fs;

    // Create local .swissarmyhammer/workflow-runs directory
    let local_dir = std::path::PathBuf::from(".swissarmyhammer/workflow-runs");
    fs::create_dir_all(&local_dir).map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to create .swissarmyhammer/workflow-runs directory: {e}"),
    })?;

    let run_storage = swissarmyhammer_workflow::FileSystemWorkflowRunStorage::new(&local_dir)
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to create local workflow run storage: {e}"),
        })?;

    Ok(Box::new(run_storage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_run_id_to_string() {
        let id = WorkflowRunId::new();
        let id_str = workflow_run_id_to_string(&id);

        // Test that the conversion produces a valid string
        assert!(!id_str.is_empty());
        assert_eq!(id_str, id.to_string());
    }
}
