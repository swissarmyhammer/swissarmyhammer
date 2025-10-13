//! Claude Code CLI executor implementation

use crate::{ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse};
use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;

/// Executor that shells out to Claude Code CLI
#[derive(Debug)]
pub struct ClaudeCodeExecutor {
    claude_path: Option<std::path::PathBuf>,
    initialized: bool,
}

impl Default for ClaudeCodeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeExecutor {
    /// Create a new ClaudeCodeExecutor instance
    ///
    /// The executor starts uninitialized and must be initialized before use.
    pub fn new() -> Self {
        Self {
            claude_path: None,
            initialized: false,
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
    ) -> ActionResult<AgentResponse> {
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        tracing::debug!(
            "Executing Claude command: {} with prompt length: {}",
            claude_path.display(),
            prompt.len()
        );

        // Build command with system prompt if provided
        let mut cmd = Command::new(claude_path);
        cmd.args([
            "--dangerously-skip-permissions",
            "--print",
            "-", // Read from stdin
        ]);

        // Add system prompt parameter if provided
        if let Some(ref sys_prompt) = system_prompt {
            tracing::debug!(
                "Executing Claude command: {} with system prompt length: {}",
                claude_path.display(),
                sys_prompt.len()
            );
            cmd.args(["--append-system-prompt", sys_prompt]);
        }

        // Execute Claude Code with stdin input
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ActionError::ClaudeError(format!("Failed to execute Claude: {e}")))?;

        // Write prompt to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(prompt.as_bytes()).await.map_err(|e| {
                ActionError::ClaudeError(format!("Failed to write to Claude stdin: {e}"))
            })?;

            // Close stdin to signal end of input
            stdin.shutdown().await.map_err(|e| {
                ActionError::ClaudeError(format!("Failed to close Claude stdin: {e}"))
            })?;
        }

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
        _context: &AgentExecutionContext<'_>,
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
        self.execute_claude_command(&claude_path_buf, rendered_prompt, system_prompt_opt)
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
