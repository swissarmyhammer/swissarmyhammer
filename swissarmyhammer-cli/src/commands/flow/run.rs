//! Run a workflow command implementation

use super::params::{map_positional_to_params, merge_params, parse_param_pairs};
use super::shared::{execute_workflow_with_progress, workflow_run_id_to_string};
use crate::context::CliContext;
use swissarmyhammer::{
    Result, SwissArmyHammerError, WorkflowExecutor, WorkflowName, WorkflowRunStatus,
};
use swissarmyhammer_common::{read_abort_file, remove_abort_file};
use swissarmyhammer_config::agent::AgentExecutorType;
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
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

    // Start MCP server if using LlamaAgent (before creating executor)
    let agent_config = context.template_context.get_agent_config(None);
    let mcp_server = if agent_config.executor_type() == AgentExecutorType::LlamaAgent {
        tracing::info!("Starting MCP server for LlamaAgent");
        let mode = McpServerMode::Http { port: None }; // Use random port
        let server = start_mcp_server(mode, None)
            .await
            .map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to start MCP server for LlamaAgent: {}", e),
            })?;
        tracing::info!("MCP server started on port {}", server.info.port.unwrap_or(0));
        Some(server)
    } else {
        None
    };

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

    // Store MCP server port if started (for LlamaAgent executors to access)
    // The actual MCP server handle lifecycle is managed by the CLI layer
    if let Some(server) = &mcp_server {
        let port = server.info.port.unwrap_or(0);
        run.context.insert(
            "_mcp_server_port".to_string(),
            serde_json::Value::Number(serde_json::Number::from(port)),
        );
        tracing::debug!("Stored MCP server port {} in workflow context", port);
    }

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

    match execution_result {
        Ok(_) => match run.status {
            WorkflowRunStatus::Completed => {
                tracing::info!("✅ Workflow completed successfully");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            WorkflowRunStatus::Failed => {
                tracing::error!("❌ Workflow failed");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            WorkflowRunStatus::Cancelled => {
                tracing::warn!("🚫 Workflow cancelled");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
            _ => {
                tracing::info!("⏸️  Workflow paused");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
        },
        Err(e) => {
            tracing::error!("❌ Workflow execution failed: {}", e);
            run.fail();

            // Shutdown MCP server if it was started
            if let Some(mut server) = mcp_server {
                tracing::debug!("Shutting down MCP server after workflow failure");
                let _ = server.shutdown().await;
            }

            return Err(e);
        }
    }

    // Shutdown MCP server if it was started
    if let Some(mut server) = mcp_server {
        tracing::info!("Shutting down MCP server");
        server.shutdown().await.map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to shutdown MCP server: {}", e),
        })?;
    }

    Ok(())
}
