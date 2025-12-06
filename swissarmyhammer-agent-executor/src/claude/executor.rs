//! Claude Code CLI executor implementation
//! sah rule ignore test_rule_with_allow

use crate::{ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse};
use agent_client_protocol::McpServer;
use async_trait::async_trait;
use swissarmyhammer_config::model::AgentExecutorType;

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

    /// Build MCP configuration JSON string for Claude CLI
    fn build_mcp_config(&self) -> ActionResult<String> {
        let (server_name, server_url) = match &self.mcp_server {
            McpServer::Http { name, url, .. } => (name, url),
            McpServer::Sse { name, url, .. } => (name, url),
            McpServer::Stdio { .. } => {
                return Err(ActionError::ClaudeError(
                    "Claude executor requires HTTP MCP server, got Stdio".to_string(),
                ))
            }
        };

        let mcp_config_json = serde_json::json!({
            "mcpServers": {
                server_name: {
                    "type": "http",
                    "url": server_url,
                    "headers": {}
                }
            }
        });

        let mcp_config_string = serde_json::to_string(&mcp_config_json).map_err(|e| {
            ActionError::ClaudeError(format!("Failed to serialize MCP config: {}", e))
        })?;

        tracing::info!(
            "Configuring Claude to use MCP server '{}' at {}",
            server_name,
            server_url
        );

        Ok(mcp_config_string)
    }

    /// Build Claude CLI command with all necessary arguments
    fn build_claude_command(
        &self,
        claude_path: &std::path::Path,
        mcp_config: &str,
        system_prompt: Option<&str>,
    ) -> tokio::process::Command {
        use tokio::process::Command;

        let mut cmd = Command::new(claude_path);
        cmd.args(["--dangerously-skip-permissions"]);
        cmd.args(["--mcp-config", mcp_config]);
        cmd.args(["--strict-mcp-config"]);
        cmd.args(["--print"]);
        cmd.args(["--tools", ""]);

        if let Some(sys_prompt) = system_prompt {
            tracing::debug!(
                "Executing Claude command: {} with system prompt length: {}",
                claude_path.display(),
                sys_prompt.len()
            );
            cmd.args(["--append-system-prompt", sys_prompt]);
        }

        tracing::debug!("Claude command: {:?}", cmd.as_std());

        cmd
    }

    /// Execute command with prompt via stdin
    async fn execute_with_prompt(
        &self,
        mut cmd: tokio::process::Command,
        prompt: String,
    ) -> ActionResult<std::process::Output> {
        use tokio::io::AsyncWriteExt;

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ActionError::ClaudeError(format!("Failed to execute Claude: {e}")))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(prompt.as_bytes()).await.map_err(|e| {
                ActionError::ClaudeError(format!("Failed to write prompt to stdin: {e}"))
            })?;
        }

        child
            .wait_with_output()
            .await
            .map_err(|e| ActionError::ClaudeError(format!("Failed to wait for Claude: {e}")))
    }

    /// Process command output and handle errors
    fn process_command_output(&self, output: std::process::Output) -> ActionResult<AgentResponse> {
        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if stderr.contains("rate limit") || stderr.contains("Rate limit") {
                let wait_time = std::time::Duration::from_secs(60);
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

        let response_text = String::from_utf8_lossy(&output.stdout).to_string();

        let response_text = if response_text.trim().is_empty() {
            tracing::warn!("Empty response from Claude Code");
            "No response from Claude".to_string()
        } else {
            response_text.trim().to_string()
        };

        Ok(AgentResponse::success(response_text))
    }

    /// Execute Claude command using stdin approach
    async fn execute_claude_command(
        &self,
        claude_path: &std::path::Path,
        prompt: String,
        system_prompt: Option<String>,
        _context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        tracing::debug!(
            "Executing Claude command: {} with prompt length: {}",
            claude_path.display(),
            prompt.len()
        );

        let mcp_config = self.build_mcp_config()?;
        let cmd = self.build_claude_command(claude_path, &mcp_config, system_prompt.as_deref());
        let output = self.execute_with_prompt(cmd, prompt).await?;
        self.process_command_output(output)
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

        let claude_path_buf = std::env::var("SAH_CLAUDE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| claude_path.clone());

        let system_prompt_opt = if system_prompt.is_empty() {
            None
        } else {
            Some(system_prompt)
        };

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
        self.initialized = false;
        Ok(())
    }
}
