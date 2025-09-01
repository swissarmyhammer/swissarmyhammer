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

    // Create subcommand for plan workflow
    let plan_var = format!("plan_filename={}", validated_file.path.display());
    let subcommand = crate::cli::FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![plan_var],
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };

    // Execute the flow command with the plan workflow
    let temp_context = swissarmyhammer_config::TemplateContext::new(); // Plan command doesn't need configuration
    let exit_code = crate::commands::flow::handle_command(subcommand, &temp_context, false).await;

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
