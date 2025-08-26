//! Plan command implementation
//!
//! Executes planning workflow for specific specification files

use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use swissarmyhammer::error::{ErrorSeverity, PlanCommandError};
use swissarmyhammer::plan_utils::validate_plan_file_comprehensive;
use swissarmyhammer::workflow::{
    FileSystemWorkflowStorage, WorkflowExecutor, WorkflowName, WorkflowStorageBackend,
};

/// Help text for the plan command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the plan command
pub async fn handle_command(
    plan_filename: String,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> i32 {
    run_plan(plan_filename).await
}

async fn run_plan(plan_filename: String) -> i32 {
    // Comprehensive plan file validation
    let validated_file = match validate_plan_file_comprehensive(&plan_filename, None) {
        Ok(file) => file,
        Err(e) => {
            // Display user-friendly error with color support
            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", e.display_to_user(use_color));

            // Log the error for debugging
            e.log_error();

            // Return appropriate exit code based on severity
            return match e.severity() {
                ErrorSeverity::Warning => EXIT_WARNING,
                ErrorSeverity::Error => EXIT_ERROR,
                ErrorSeverity::Critical => EXIT_ERROR,
            };
        }
    };

    tracing::info!("Executing plan workflow with validated file");
    tracing::debug!("Plan file path: {}", validated_file.path.display());
    tracing::debug!("Plan file size: {} bytes", validated_file.size);

    // Load workflow from storage
    let storage = match FileSystemWorkflowStorage::new() {
        Ok(s) => s,
        Err(e) => {
            let plan_error = PlanCommandError::WorkflowExecutionFailed {
                plan_filename: plan_filename.clone(),
                source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                    reason: format!("Failed to create workflow storage: {e}"),
                },
            };
            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", plan_error.display_to_user(use_color));
            plan_error.log_error();
            return EXIT_ERROR;
        }
    };

    let workflow_name = WorkflowName::new("plan");
    let workflow = match storage.get_workflow(&workflow_name) {
        Ok(w) => w,
        Err(e) => {
            let plan_error = PlanCommandError::WorkflowExecutionFailed {
                plan_filename: plan_filename.clone(),
                source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                    reason: format!("Failed to load workflow: {e}"),
                },
            };
            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", plan_error.display_to_user(use_color));
            plan_error.log_error();
            return EXIT_ERROR;
        }
    };

    // Create executor and start workflow
    let mut executor = WorkflowExecutor::new();
    let mut run = match executor.start_workflow(workflow.clone()) {
        Ok(r) => r,
        Err(e) => {
            let plan_error = PlanCommandError::WorkflowExecutionFailed {
                plan_filename: plan_filename.clone(),
                source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                    reason: format!("Failed to start workflow: {e}"),
                },
            };
            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", plan_error.display_to_user(use_color));
            plan_error.log_error();
            return EXIT_ERROR;
        }
    };

    // Set plan_filename variable
    let mut variables = std::collections::HashMap::new();
    variables.insert(
        "plan_filename".to_string(),
        serde_json::Value::String(validated_file.path.display().to_string()),
    );
    run.context.set_workflow_vars(variables);

    // Execute the workflow
    let result = executor.resume_workflow(run).await;
    let exit_code = match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Workflow execution failed: {}", e);
            EXIT_ERROR
        }
    };

    if exit_code == EXIT_SUCCESS {
        tracing::info!("Plan workflow execution completed successfully");
        EXIT_SUCCESS
    } else {
        tracing::error!(
            "Plan workflow execution failed with exit code: {}",
            exit_code
        );

        // Create and display a PlanCommandError for workflow failures
        let plan_error = PlanCommandError::WorkflowExecutionFailed {
            plan_filename: plan_filename.clone(),
            source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                reason: format!("Workflow execution failed with exit code {}", exit_code),
            },
        };

        let use_color = crate::cli::Cli::should_use_color();
        eprintln!("{}", plan_error.display_to_user(use_color));
        plan_error.log_error();

        EXIT_ERROR
    }
}
