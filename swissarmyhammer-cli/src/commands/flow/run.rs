//! Run a workflow command implementation

use super::params::{map_positional_to_params, merge_params, parse_param_pairs};
use super::shared::{
    create_local_workflow_run_storage, execute_workflow_with_progress, workflow_run_id_to_string,
};
use crate::context::CliContext;
use swissarmyhammer::{
    Result, SwissArmyHammerError, WorkflowExecutor, WorkflowName, WorkflowRunStatus,
};
use swissarmyhammer_common::{read_abort_file, remove_abort_file};
use tokio::signal;

/// Configuration for running a workflow command
pub struct RunCommandConfig {
    pub workflow: String,
    pub positional_args: Vec<String>,
    pub params: Vec<String>,
    pub vars: Vec<String>,
    pub interactive: bool,
    pub dry_run: bool,
    pub quiet: bool,
}

/// Execute the run workflow command
pub async fn execute_run_command(config: RunCommandConfig, context: &CliContext) -> Result<()> {
    run_workflow_command(config, context).await
}

/// Execute a workflow with given configuration
pub async fn run_workflow_command(config: RunCommandConfig, context: &CliContext) -> Result<()> {
    let workflow_name_typed = WorkflowName::new(&config.workflow);
    let workflow = context
        .workflow_storage
        .get_workflow(&workflow_name_typed)?;

    // Map positional arguments to required workflow parameters
    let positional_params = map_positional_to_params(&workflow, config.positional_args)?;

    // Parse --param key=value pairs
    let param_pairs = parse_param_pairs(&config.params)?;

    // Parse --var key=value pairs (deprecated but still supported)
    let var_pairs = parse_param_pairs(&config.vars)?;

    // Merge all parameter sources with proper precedence
    let variables = merge_params(positional_params, param_pairs, var_pairs);

    if config.dry_run {
        println!("🔍 Dry run mode - showing execution plan:");
        println!("📋 Workflow: {}", workflow.name);
        println!("🏁 Initial state: {}", workflow.initial_state);
        println!("🔧 Variables: {variables:?}");

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

    // Setup signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Execute workflow with signal handling
    let execution_result = tokio::select! {
        result = execute_workflow_with_progress(&mut executor, &mut run, config.interactive) => result,
        _ = shutdown_rx.recv() => {
            tracing::info!("Workflow execution interrupted by user");
            run.status = WorkflowRunStatus::Cancelled;
            Ok(())
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
