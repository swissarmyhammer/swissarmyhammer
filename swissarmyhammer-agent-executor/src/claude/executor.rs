//! Claude Code CLI executor implementation

use crate::{ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse};
use agent_client_protocol::McpServer;
use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;

/// Executor that shells out to Claude Code CLI
#[derive(Debug, Clone)]
pub struct ClaudeCodeExecutor {
    claude_path: Option<std::path::PathBuf>,
    initialized: bool,
    #[allow(dead_code)] // Used for future MCP integration when Claude supports HTTP config
    mcp_server: McpServer,
}

impl ClaudeCodeExecutor {
    /// Create a new ClaudeCodeExecutor instance
    ///
    /// # Arguments
    ///
    /// * `mcp_server` - MCP server configuration using agent-client-protocol types
    ///
    /// The executor starts uninitialized and must be initialized before use.
    pub fn new(mcp_server: McpServer) -> Self {
        Self {
            claude_path: None,
            initialized: false,
            mcp_server,
        }
    }

    /// Get the path to the claude executable
    fn get_claude_path(&self) -> ActionResult<&std::path::PathBuf> {
        self.claude_path.as_ref().ok_or_else(|| {
            ActionError::ExecutionError("Claude executor not initialized".to_string())
        })
    }

    /// Execute Claude command using stdin approach (maintaining backward compatibility)
    async fn execute_claude_command(
        &self,
        claude_path: &std::path::PathBuf,
        prompt: String,
        system_prompt: Option<String>,
        _context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        
        use tokio::process::Command;

        tracing::debug!(
            "Executing Claude command: {} with prompt length: {}",
            claude_path.display(),
            prompt.len()
        );

        // Build command - flags first, then stdin marker at the end
        let mut cmd = Command::new(claude_path);
        cmd.args(["--dangerously-skip-permissions", "--print"]);

        // Disable built-in tools for deterministic behavior
        cmd.args(["--tools", ""]);

        // Extract URL from McpServer and create Claude-compatible JSON
        let (server_name, server_url) = match &self.mcp_server {
            McpServer::Http { name, url, .. } => (name, url),
            McpServer::Sse { name, url, .. } => (name, url),
            McpServer::Stdio {  .. } => {
                return Err(ActionError::ClaudeError(
                    "Claude executor requires HTTP MCP server, got Stdio".to_string()
                ))
            }
        };

        // Create MCP config with headers as object (not array) for Claude compatibility
        let mcp_config_json = serde_json::json!({
            "mcpServers": {
                server_name: {
                    "type": "http",
                    "url": server_url,
                    "headers": {}
                }
            }
        });

        let config_file = std::env::temp_dir().join(format!("sah-mcp-{}.json", std::process::id()));
        std::fs::write(&config_file, serde_json::to_string_pretty(&mcp_config_json).unwrap()).map_err(|e| {
            ActionError::ClaudeError(format!("Failed to write MCP config file: {}", e))
        })?;

        tracing::info!("Configuring Claude to use MCP server '{}' at {} (config: {:?})", server_name, server_url, config_file);
        cmd.args(["--strict-mcp-config"]);
        cmd.args(["--mcp-config", &config_file.to_string_lossy()]);

        // Log the full command for debugging
        tracing::info!("Claude command: {:?}", cmd.as_std());

        // Add system prompt parameter if provided
        if let Some(ref sys_prompt) = system_prompt {
            tracing::debug!(
                "Executing Claude command: {} with system prompt length: {}",
                claude_path.display(),
                sys_prompt.len()
            );
            cmd.args(["--append-system-prompt", sys_prompt]);
        }

        // Write prompt to temp file instead of stdin (workaround for Claude CLI --mcp-config bug)
        let prompt_file = std::env::temp_dir().join(format!("sah-prompt-{}.txt", std::process::id()));
        std::fs::write(&prompt_file, &prompt).map_err(|e| {
            ActionError::ClaudeError(format!("Failed to write prompt file: {}", e))
        })?;
        cmd.arg(prompt_file.to_string_lossy().to_string());

        // Execute Claude Code
        let child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ActionError::ClaudeError(format!("Failed to execute Claude: {e}")))?;

        // Wait for command completion
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| ActionError::ClaudeError(format!("Failed to wait for Claude: {e}")))?;

        // Check if Claude execution was successful
        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check for rate limiting
            if stderr.contains("rate limit") || stderr.contains("Rate limit") {
                let wait_time = std::time::Duration::from_secs(60); // Default wait time
                return Err(ActionError::RateLimit {
                    message: stderr.to_string(),
                    wait_time,
                });
            }

            return Err(ActionError::ClaudeError(format!(
                "Claude execution failed with exit code {}: {}",
                exit_code, stderr
            )));
        }

        // Extract response from stdout
        let response_text = String::from_utf8_lossy(&output.stdout).to_string();

        // Process the response
        let response_text = if response_text.trim().is_empty() {
            tracing::warn!("Empty response from Claude Code");
            "No response from Claude".to_string()
        } else {
            response_text.trim().to_string()
        };

        Ok(AgentResponse::success(response_text))
    }
}

#[async_trait]
impl AgentExecutor for ClaudeCodeExecutor {
    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        let claude_path = self.get_claude_path()?;

        // Get Claude CLI path from environment
        let claude_path_buf = std::env::var("SAH_CLAUDE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| claude_path.clone());

        // Convert system prompt to Option for backward compatibility
        let system_prompt_opt = if system_prompt.is_empty() {
            None
        } else {
            Some(system_prompt)
        };

        // Execute Claude command using the same approach as the original implementation
        self.execute_claude_command(
            &claude_path_buf,
            rendered_prompt,
            system_prompt_opt,
            context,
        )
        .await
    }

    fn executor_type(&self) -> AgentExecutorType {
        AgentExecutorType::ClaudeCode
    }

    async fn initialize(&mut self) -> ActionResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Find claude executable in PATH
        self.claude_path = Some(which::which("claude").map_err(|_| {
            ActionError::ExecutionError(
                "Claude CLI not found in PATH. Please install Claude Code CLI.".to_string(),
            )
        })?);

        self.initialized = true;
        tracing::debug!(
            "ClaudeCodeExecutor initialized with claude at: {:?}",
            self.claude_path
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> ActionResult<()> {
        // No resources to cleanup for CLI approach
        self.initialized = false;
        Ok(())
    }
}
