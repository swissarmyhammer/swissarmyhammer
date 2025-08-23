use crate::cli::FlowSubcommand;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use crate::flow;
use swissarmyhammer::error::{ErrorSeverity, PlanCommandError, SwissArmyHammerError};
use swissarmyhammer::plan_utils::{validate_issues_directory, validate_plan_file_comprehensive};

pub async fn run_plan(plan_filename: String) -> i32 {
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

    // Create a FlowSubcommand::Run with the validated plan_filename variable
    let plan_var = format!("plan_filename={}", validated_file.path.display());

    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![plan_var],
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };

    tracing::info!("Executing plan workflow with validated file");
    tracing::debug!("Plan file path: {}", validated_file.path.display());
    tracing::debug!("Plan file size: {} bytes", validated_file.size);

    // Execute the flow command with the plan workflow
    match flow::run_flow_command(subcommand).await {
        Ok(_) => {
            tracing::info!("Plan workflow execution completed successfully");
            EXIT_SUCCESS
        }
        Err(e) => {
            tracing::error!("Plan workflow execution failed: {}", e);
            // Check if this is an abort error (file-based detection)
            if let SwissArmyHammerError::ExecutorError(
                swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
            ) = &e
            {
                // Create and display a PlanCommandError for workflow failures
                let plan_error = PlanCommandError::WorkflowExecutionFailed {
                    plan_filename: plan_filename.clone(),
                    source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                        reason: abort_reason.clone(),
                    },
                };

                let use_color = crate::cli::Cli::should_use_color();
                eprintln!("{}", plan_error.display_to_user(use_color));
                plan_error.log_error();

                return EXIT_ERROR;
            }

            // For other workflow errors, also wrap them
            let plan_error = PlanCommandError::WorkflowExecutionFailed {
                plan_filename: plan_filename.clone(),
                source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                    reason: e.to_string(),
                },
            };

            let use_color = crate::cli::Cli::should_use_color();
            eprintln!("{}", plan_error.display_to_user(use_color));
            plan_error.log_error();

            EXIT_ERROR
        }
    }
}
