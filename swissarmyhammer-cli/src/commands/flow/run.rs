//! Run a workflow command implementation

use super::shared::{
    create_local_workflow_run_storage, execute_workflow_with_progress, parse_duration,
    workflow_run_id_to_string,
};
use crate::context::CliContext;
use crate::parameter_cli;
use std::collections::HashMap;
use std::future;
use swissarmyhammer::{
    Result, SwissArmyHammerError, WorkflowExecutor, WorkflowName, WorkflowRunStatus,
};
use swissarmyhammer_common::{read_abort_file, remove_abort_file};
use tokio::signal;
use tokio::time::timeout;

/// Configuration for running a workflow command
pub struct WorkflowCommandConfig {
    pub workflow_name: String,
    pub vars: Vec<String>,
    pub interactive: bool,
    pub dry_run: bool,
    pub timeout_str: Option<String>,
    pub quiet: bool,
}

/// Execute the run workflow command
pub async fn execute_run_command(
    workflow: String,
    vars: Vec<String>,
    interactive: bool,
    dry_run: bool,
    timeout: Option<String>,
    quiet: bool,
    context: &CliContext,
) -> Result<()> {
    let config = WorkflowCommandConfig {
        workflow_name: workflow,
        vars,
        interactive,
        dry_run,
        timeout_str: timeout,
        quiet,
    };

    run_workflow_command(config, context).await
}

/// Execute a workflow with given configuration
pub async fn run_workflow_command(
    config: WorkflowCommandConfig,
    context: &CliContext,
) -> Result<()> {
    let workflow_name_typed = WorkflowName::new(&config.workflow_name);
    let workflow = context
        .workflow_storage
        .get_workflow(&workflow_name_typed)?;

    // Resolve workflow parameters with enhanced parameter system
    let workflow_variables = parameter_cli::resolve_workflow_parameters_interactive(
        &workflow,
        &config.vars,
        config.interactive && !config.dry_run,
    )
    .unwrap_or_else(|e| {
        eprintln!("Warning: Failed to resolve workflow parameters: {e}");
        HashMap::new()
    });

    // Parse additional variables not handled by workflow parameters (backward compatibility)
    let mut variables = workflow_variables;
    for var in config.vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].to_string();
            // Add variable, allowing later values to override earlier ones
            variables.insert(key, serde_json::Value::String(parts[1].to_string()));
        } else {
            return Err(SwissArmyHammerError::Other { message: format!(
                "Invalid variable format: '{var}'. Expected 'key=value' format. Example: --var input=test"
            ) });
        }
    }

    // Parse timeout
    let timeout_duration = if let Some(timeout_str) = config.timeout_str {
        Some(parse_duration(&timeout_str)?)
    } else {
        None
    };

    if config.dry_run {
        println!("🔍 Dry run mode - showing execution plan:");
        println!("📋 Workflow: {}", workflow.name);
        println!("🏁 Initial state: {}", workflow.initial_state);
        println!("🔧 Variables: {variables:?}");
        if let Some(timeout) = timeout_duration {
            println!("⏱️  Timeout: {timeout:?}");
        }
        println!("📊 States: {}", workflow.states.len());
        println!("🔄 Transitions: {}", workflow.transitions.len());

        // Show workflow structure
        println!("\n📈 Workflow structure:");
        for (state_id, state) in &workflow.states {
            println!(
                "  {} - {} {}",
                state_id,
                state.description,
                if state.is_terminal { "(terminal)" } else { "" }
            );
        }

        return Ok(());
    }

    tracing::info!("🚀 Starting workflow: {}", workflow.name);

    // Check for abort file before starting workflow
    let current_dir = std::env::current_dir().map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to get current directory: {}", e),
    })?;
    if let Some(abort_reason) =
        read_abort_file(&current_dir).map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })?
    {
        // Clean up the abort file after detection
        let _ = remove_abort_file(&current_dir).map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        });
        return Err(SwissArmyHammerError::from(
            swissarmyhammer_workflow::ExecutorError::Abort(abort_reason),
        ));
    }

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Create workflow run
    let mut run =
        executor
            .start_workflow(workflow.clone())
            .map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to start workflow '{}': {}", workflow.name, e),
            })?;

    // Set initial variables
    run.context.set_workflow_vars(variables.clone());

    // Set agent configuration from template context
    let agent_config = context.template_context.get_agent_config(None);
    run.context.set_agent_config(agent_config);

    // Set quiet mode in context for actions to use
    if config.quiet {
        run.context
            .insert("_quiet".to_string(), serde_json::Value::Bool(true));
    }

    // Set timeout in context for actions to use
    if let Some(timeout_duration) = timeout_duration {
        run.context.insert(
            "_timeout_secs".to_string(),
            serde_json::Value::Number(serde_json::Number::from(timeout_duration.as_secs())),
        );
    }

    // Setup signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Execute workflow with timeout and signal handling
    let execution_result = if let Some(timeout_duration) = timeout_duration {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, config.interactive) => result,
            _ = timeout(timeout_duration, future::pending::<()>()) => {
                tracing::warn!("Workflow execution timed out");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            },
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    } else {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, config.interactive) => result,
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    };

    // Create local workflow run storage (only store failed runs for debugging)
    let mut run_storage = create_local_workflow_run_storage()?;

    match execution_result {
        Ok(_) => match run.status {
            WorkflowRunStatus::Completed => {
                tracing::info!("✅ Workflow completed successfully");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Don't store successful runs to avoid accumulating thousands of runs
                // Only failed runs are stored for debugging purposes
            }
            WorkflowRunStatus::Failed => {
                tracing::error!("❌ Workflow failed");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Store failed runs for debugging
                if let Err(storage_err) = run_storage.store_run(&run) {
                    tracing::warn!("Failed to store failed run: {}", storage_err);
                }
            }
            WorkflowRunStatus::Cancelled => {
                tracing::warn!("🚫 Workflow cancelled");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Store cancelled runs for debugging
                if let Err(storage_err) = run_storage.store_run(&run) {
                    tracing::warn!("Failed to store cancelled run: {}", storage_err);
                }
            }
            _ => {
                tracing::info!("⏸️  Workflow paused");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
        },
        Err(e) => {
            tracing::error!("❌ Workflow execution failed: {}", e);
            run.fail();

            // Store failed runs for debugging
            if let Err(storage_err) = run_storage.store_run(&run) {
                tracing::warn!("Failed to store failed run: {}", storage_err);
            }

            // Return the error to allow proper exit code handling in main.rs
            return Err(e);
        }
    }

    Ok(())
}
