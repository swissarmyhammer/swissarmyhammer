//! ACP server command handler
//!
//! Starts the Agent Client Protocol server over stdio for editor integration.

use llama_agent::acp::permissions::PermissionPolicy;
use llama_agent::acp::{AcpConfig, AcpServer, GracefulShutdownTimeout};
use llama_agent::{AgentAPI, AgentConfig, AgentServer};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{stdin, stdout};

/// Handle the ACP server command
pub async fn handle_command(
    config_path: Option<PathBuf>,
    permission_policy: Option<String>,
    allow_paths: Vec<PathBuf>,
    block_paths: Vec<PathBuf>,
    max_file_size: Option<u64>,
    terminal_buffer_size: Option<usize>,
    graceful_shutdown_timeout: Option<u64>,
) -> i32 {
    // Load ACP configuration
    let mut acp_config = match load_acp_config(config_path).await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load ACP configuration: {}", e);
            return crate::exit_codes::EXIT_ERROR;
        }
    };

    // Apply command-line overrides
    if let Some(policy) = permission_policy {
        match parse_permission_policy(&policy) {
            Ok(p) => acp_config.permission_policy = p,
            Err(e) => {
                eprintln!("Invalid permission policy: {}", e);
                return crate::exit_codes::EXIT_ERROR;
            }
        }
    }

    if !allow_paths.is_empty() {
        acp_config.filesystem.allowed_paths = allow_paths;
    }

    if !block_paths.is_empty() {
        acp_config.filesystem.blocked_paths = block_paths;
    }

    if let Some(size) = max_file_size {
        acp_config.filesystem.max_file_size = size;
    }

    if let Some(buffer_size) = terminal_buffer_size {
        acp_config.terminal.output_buffer_bytes = buffer_size;
    }

    if let Some(timeout_secs) = graceful_shutdown_timeout {
        acp_config.terminal.graceful_shutdown_timeout =
            GracefulShutdownTimeout::new(Duration::from_secs(timeout_secs));
    }

    // Create agent server with default configuration
    // TODO: Allow customization of AgentConfig via configuration file
    let agent_config = AgentConfig::default();
    let agent_server = match AgentServer::initialize(agent_config).await {
        Ok(server) => Arc::new(server),
        Err(e) => {
            eprintln!("Failed to initialize agent server: {}", e);
            return crate::exit_codes::EXIT_ERROR;
        }
    };

    // Create ACP server
    let (acp_server, _notification_rx) = AcpServer::new(agent_server, acp_config);
    let acp_server = Arc::new(acp_server);

    // Run with stdio
    tracing::info!("Starting ACP server over stdio");
    match acp_server.start_with_streams(stdin(), stdout()).await {
        Ok(_) => {
            tracing::info!("ACP server stopped successfully");
            crate::exit_codes::EXIT_SUCCESS
        }
        Err(e) => {
            eprintln!("ACP server error: {}", e);
            crate::exit_codes::EXIT_ERROR
        }
    }
}

async fn load_acp_config(
    config_path: Option<PathBuf>,
) -> Result<AcpConfig, Box<dyn std::error::Error>> {
    if let Some(path) = config_path {
        // Load from file
        let content = tokio::fs::read_to_string(&path).await?;
        let config: AcpConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    } else {
        // Use default configuration
        Ok(AcpConfig::default())
    }
}

fn parse_permission_policy(policy: &str) -> Result<PermissionPolicy, String> {
    match policy.to_lowercase().as_str() {
        "always-ask" | "alwaysask" => Ok(PermissionPolicy::AlwaysAsk),
        "auto-approve-reads" | "autoapprovereads" => Ok(PermissionPolicy::AutoApproveReads),
        _ => Err(format!(
            "Unknown permission policy '{}'. Valid options: always-ask, auto-approve-reads",
            policy
        )),
    }
}
