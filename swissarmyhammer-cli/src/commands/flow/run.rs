//! Run a workflow command implementation

use super::params::{map_positional_to_params, merge_params, parse_param_pairs};
use crate::context::CliContext;
use swissarmyhammer::{Result, SwissArmyHammerError, WorkflowName};
use serde_json::json;

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
pub async fn execute_run_command(
    config: RunCommandConfig,
    context: &CliContext,
    cli_tool_context: std::sync::Arc<crate::mcp_integration::CliToolContext>,
) -> Result<()> {
    run_workflow_command(config, context, cli_tool_context).await
}

/// Execute a workflow with given configuration
pub async fn run_workflow_command(
    config: RunCommandConfig,
    context: &CliContext,
    cli_tool_context: std::sync::Arc<crate::mcp_integration::CliToolContext>,
) -> Result<()> {
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

    // Build arguments for MCP flow tool
    let mut tool_arguments = serde_json::Map::new();
    tool_arguments.insert("flow_name".to_string(), json!(config.workflow));
    tool_arguments.insert("parameters".to_string(), json!(variables));
    tool_arguments.insert("interactive".to_string(), json!(config.interactive));
    tool_arguments.insert("dry_run".to_string(), json!(config.dry_run));
    tool_arguments.insert("quiet".to_string(), json!(config.quiet));

    // Call the MCP flow tool
    let result = cli_tool_context
        .execute_tool("flow", tool_arguments)
        .await
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Flow tool execution failed: {}", e),
        })?;

    // Check if execution resulted in error
    if result.is_error.unwrap_or(false) {
        let error_message =
            crate::mcp_integration::response_formatting::format_error_response(&result);
        eprintln!("{}", error_message);
        return Err(SwissArmyHammerError::Other {
            message: "Workflow execution failed".to_string(),
        });
    }

    // Format and display the success result
    let success_message =
        crate::mcp_integration::response_formatting::format_success_response(&result);
    if !config.quiet {
        println!("{}", success_message);
    }

    Ok(())
}
