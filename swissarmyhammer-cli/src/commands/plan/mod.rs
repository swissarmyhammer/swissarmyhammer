//! Plan command implementation
//!
//! Executes planning workflow for specific specification files

use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use swissarmyhammer::plan_utils::validate_plan_file_comprehensive;

use swissarmyhammer::{
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
            eprintln!("{}", e);
            tracing::error!("{}", e);
            return EXIT_ERROR;
        }
    };

    tracing::info!("Executing plan workflow with validated file");
    tracing::debug!("Plan file path: {}", validated_file.path.display());
    tracing::debug!("Plan file size: {} bytes", validated_file.size);

    // Load workflow from storage
    let storage = match FileSystemWorkflowStorage::new() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("{}", e);
            return EXIT_ERROR;
        }
    };

    let workflow_name = WorkflowName::new("plan");
    let workflow = match storage.get_workflow(&workflow_name) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("{}", e);
            return EXIT_ERROR;
        }
    };

    // Create executor and start workflow
    let mut executor = WorkflowExecutor::new();
    let mut run = match executor.start_workflow(workflow.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("{}", e);
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

        EXIT_ERROR
    }
}
