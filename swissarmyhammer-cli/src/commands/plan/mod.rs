//! Plan command implementation
//!
//! Executes planning workflow for specific specification files

use crate::commands::flow::{run_workflow_command, WorkflowCommandConfig};
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use swissarmyhammer::error::{ErrorSeverity, PlanCommandError};
use swissarmyhammer::plan_utils::{validate_issues_directory, validate_plan_file_comprehensive};

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

    // Validate issues directory
    match validate_issues_directory() {
        Ok(_) => {
            tracing::debug!("Issues directory validation successful");
        }
        Err(e) => {
            // Display user-friendly error
            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", e.display_to_user(use_color));

            // Log the error for debugging
            e.log_error();

            return EXIT_ERROR;
        }
    }

    // Create a plan_filename variable with the validated file path
    let plan_var = format!("plan_filename={}", validated_file.path.display());

    let config = WorkflowCommandConfig {
        workflow_name: "plan".to_string(),
        vars: vec![plan_var],
        interactive: false,
        dry_run: false,
        test_mode: false,
        timeout_str: None,
        quiet: false,
    };

    tracing::info!("Executing plan workflow with validated file");
    tracing::debug!("Plan file path: {}", validated_file.path.display());
    tracing::debug!("Plan file size: {} bytes", validated_file.size);

    // Execute the workflow directly without unnecessary indirection
    let temp_context = swissarmyhammer_config::TemplateContext::new(); // Plan command doesn't need configuration
    let result = run_workflow_command(config, &temp_context).await;
    let exit_code = match result {
        Ok(()) => EXIT_SUCCESS,
        Err(_) => EXIT_ERROR,
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
