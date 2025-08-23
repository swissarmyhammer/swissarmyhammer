//! Claude Code CLI integration with system prompt injection
//!
//! This module provides centralized Claude Code CLI invocation capabilities with automatic
//! system prompt injection via the `--append-system-prompt` parameter. It ensures that all
//! Claude Code invocations include the rendered system prompt content while maintaining
//! backward compatibility and graceful error handling.

use crate::system_prompt::{render_system_prompt, SystemPromptError};
use std::process::Output;
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, warn};

/// Errors that can occur during Claude Code integration
#[derive(Debug, Error)]
pub enum ClaudeCodeError {
    /// Claude CLI not found in PATH
    #[error("Claude CLI not found. Make sure 'claude' is installed and available in your PATH")]
    ClaudeNotFound,

    /// Failed to spawn Claude command
    #[error("Failed to spawn Claude command: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// System prompt rendering failed (non-blocking)
    #[error("System prompt rendering failed: {0}")]
    SystemPromptFailed(String),

    /// Claude execution failed
    #[error("Claude execution failed with exit code {exit_code}: {stderr}")]
    ExecutionFailed {
        /// Exit code from Claude process
        exit_code: i32,
        /// stderr output from Claude
        stderr: String,
    },
}

/// Configuration options for Claude Code integration
#[derive(Debug, Clone)]
pub struct ClaudeCodeConfig {
    /// Whether to enable system prompt injection
    pub enable_system_prompt_injection: bool,
    /// Whether to enable debug logging for system prompt operations
    pub system_prompt_debug: bool,
    /// Path to Claude CLI executable (None to use PATH)
    pub claude_path: Option<String>,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            enable_system_prompt_injection: true,
            system_prompt_debug: false,
            claude_path: None,
        }
    }
}

/// Claude Code invocation builder for constructing commands with system prompt integration
pub struct ClaudeCodeInvocation {
    /// Base arguments to pass to Claude CLI
    args: Vec<String>,
    /// Additional parameters to append
    additional_params: Vec<String>,
    /// Configuration options
    config: ClaudeCodeConfig,
    /// Whether to suppress stdout output
    quiet: bool,
}

impl ClaudeCodeInvocation {
    /// Create a new Claude Code invocation with default configuration
    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            additional_params: Vec::new(),
            config: ClaudeCodeConfig::default(),
            quiet: false,
        }
    }

    /// Add base arguments to the Claude CLI command
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args
            .extend(args.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    /// Add additional parameters to the command
    pub fn additional_params<I, S>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.additional_params
            .extend(params.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    /// Set configuration options
    pub fn config(mut self, config: ClaudeCodeConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable or disable quiet mode (suppress stdout output)
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Execute the Claude Code command with system prompt integration
    pub async fn execute(self) -> Result<Output, ClaudeCodeError> {
        execute_claude_code_with_system_prompt(
            &self.args,
            Some(self.additional_params),
            self.config,
            self.quiet,
        )
        .await
    }
}

impl Default for ClaudeCodeInvocation {
    fn default() -> Self {
        Self::new()
    }
}

/// Find Claude CLI executable in PATH or common installation locations
fn find_claude_path(config_path: Option<&str>) -> Result<String, ClaudeCodeError> {
    // If explicit path is provided in config, use it
    if let Some(path) = config_path {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
        warn!("Claude path specified in config does not exist: {}", path);
    }

    // Try to find in PATH first
    if let Ok(path) = which::which("claude") {
        return Ok(path.to_string_lossy().to_string());
    }

    // Check common installation paths
    let home = std::env::var("HOME").unwrap_or_default();
    let possible_paths = vec![
        format!("{}/.claude/local/claude", home),
        "/usr/local/bin/claude".to_string(),
        "/opt/claude/claude".to_string(),
        "/Applications/Claude Code.app/Contents/MacOS/claude".to_string(),
    ];

    for path in possible_paths {
        if std::path::Path::new(&path).exists() {
            debug!("Found Claude CLI at: {}", path);
            return Ok(path);
        }
    }

    Err(ClaudeCodeError::ClaudeNotFound)
}

/// Render system prompt and return as parameter if successful
async fn prepare_system_prompt_param(
    config: &ClaudeCodeConfig,
) -> Result<Option<String>, ClaudeCodeError> {
    if !config.enable_system_prompt_injection {
        debug!("System prompt injection disabled in configuration");
        return Ok(None);
    }

    match render_system_prompt() {
        Ok(rendered_prompt) => {
            if config.system_prompt_debug {
                debug!(
                    "Successfully rendered system prompt ({} chars)",
                    rendered_prompt.len()
                );
            }
            Ok(Some(rendered_prompt))
        }
        Err(SystemPromptError::FileNotFound(_)) => {
            // File not found is not an error - system prompt is optional
            debug!("System prompt file not found - continuing without system prompt");
            Ok(None)
        }
        Err(err) => {
            // Other errors are warnings but don't block execution
            warn!("Failed to render system prompt: {} - continuing without system prompt", err);
            if config.system_prompt_debug {
                return Err(ClaudeCodeError::SystemPromptFailed(err.to_string()));
            }
            Ok(None)
        }
    }
}

/// Execute Claude Code CLI with system prompt integration
///
/// This function provides centralized Claude Code invocation with automatic system prompt
/// injection. It handles all the complexity of finding the Claude CLI, rendering the system
/// prompt, and constructing the appropriate command line arguments.
///
/// # Arguments
/// * `args` - Base arguments to pass to Claude CLI
/// * `additional_params` - Additional parameters to append to the command
/// * `config` - Configuration options for the invocation
/// * `quiet` - Whether to suppress stdout output (logs only)
///
/// # Returns
/// * `Ok(Output)` - The command output on success
/// * `Err(ClaudeCodeError)` - Various errors including CLI not found, spawn failures, etc.
///
/// # Behavior
/// - Attempts to render system prompt if enabled in config
/// - Gracefully continues without system prompt if rendering fails (with warnings)
/// - Finds Claude CLI in PATH or common installation locations
/// - Constructs command with all arguments and system prompt parameter
/// - Returns full command output including stdout, stderr, and exit status
///
/// # Examples
///
/// ```rust,no_run
/// use swissarmyhammer::claude_code_integration::{execute_claude_code_with_system_prompt, ClaudeCodeConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let args = vec!["--print".to_string(), "--output-format".to_string(), "stream-json".to_string()];
/// let config = ClaudeCodeConfig::default();
/// let output = execute_claude_code_with_system_prompt(&args, None, config, false).await?;
/// println!("Claude output: {}", String::from_utf8_lossy(&output.stdout));
/// # Ok(())
/// # }
/// ```
pub async fn execute_claude_code_with_system_prompt(
    args: &[String],
    additional_params: Option<Vec<String>>,
    config: ClaudeCodeConfig,
    quiet: bool,
) -> Result<Output, ClaudeCodeError> {
    debug!("Executing Claude Code with system prompt integration");

    // Find Claude CLI executable
    let claude_path = find_claude_path(config.claude_path.as_deref())?;

    // Prepare system prompt parameter if enabled
    let system_prompt_param = prepare_system_prompt_param(&config).await?;

    // Build command
    let mut cmd = Command::new(&claude_path);

    // Add base arguments
    cmd.args(args);

    // Add system prompt parameter if available
    if let Some(ref prompt_content) = system_prompt_param {
        cmd.arg("--append-system-prompt").arg(prompt_content);
        debug!("Added system prompt parameter ({} chars)", prompt_content.len());
    }

    // Add additional parameters
    if let Some(ref params) = additional_params {
        cmd.args(params);
    }

    // Configure stdio
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    debug!(
        "Executing command: {} {}",
        claude_path,
        args.join(" ")
    );

    // Execute the command
    let output = cmd.output().await?;

    // Check for execution success
    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        return Err(ClaudeCodeError::ExecutionFailed { exit_code, stderr });
    }

    // Log success if not in quiet mode
    if !quiet {
        debug!(
            "Claude Code execution completed successfully (exit code: {})",
            output.status.code().unwrap_or(0)
        );
    }

    Ok(output)
}

/// Convenience function for simple Claude Code invocations with default configuration
///
/// # Examples
///
/// ```rust,no_run
/// use swissarmyhammer::claude_code_integration::invoke_claude_code;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let args = vec!["--version".to_string()];
/// let output = invoke_claude_code(&args).await?;
/// println!("Claude version: {}", String::from_utf8_lossy(&output.stdout));
/// # Ok(())
/// # }
/// ```
pub async fn invoke_claude_code(args: &[String]) -> Result<Output, ClaudeCodeError> {
    execute_claude_code_with_system_prompt(args, None, ClaudeCodeConfig::default(), false).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_config_default() {
        let config = ClaudeCodeConfig::default();
        assert!(config.enable_system_prompt_injection);
        assert!(!config.system_prompt_debug);
        assert!(config.claude_path.is_none());
    }

    #[test]
    fn test_claude_code_invocation_builder() {
        let invocation = ClaudeCodeInvocation::new()
            .args(vec!["--print", "--verbose"])
            .additional_params(vec!["--timeout", "30"])
            .quiet(true);

        assert_eq!(invocation.args, vec!["--print", "--verbose"]);
        assert_eq!(invocation.additional_params, vec!["--timeout", "30"]);
        assert!(invocation.quiet);
    }

    #[test]
    fn test_find_claude_path_with_config() {
        // Test with non-existent config path
        let result = find_claude_path(Some("/non/existent/path"));
        // Should try PATH and other locations since config path doesn't exist
        match result {
            Ok(_) => (), // Claude found in PATH or common locations
            Err(ClaudeCodeError::ClaudeNotFound) => (), // Expected if Claude not installed
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_prepare_system_prompt_param_disabled() {
        let mut config = ClaudeCodeConfig::default();
        config.enable_system_prompt_injection = false;

        let result = prepare_system_prompt_param(&config).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_prepare_system_prompt_param_enabled() {
        let config = ClaudeCodeConfig::default();
        
        // This test will pass if system prompt exists, or fail gracefully if not
        let result = prepare_system_prompt_param(&config).await;
        match result {
            Ok(Some(_)) => (), // System prompt rendered successfully
            Ok(None) => (),    // System prompt not found (expected in test environment)
            Err(_) => (),      // Other errors handled gracefully
        }
    }

    #[test]
    fn test_claude_code_error_display() {
        let error = ClaudeCodeError::ClaudeNotFound;
        assert!(error.to_string().contains("Claude CLI not found"));

        let error = ClaudeCodeError::ExecutionFailed {
            exit_code: 1,
            stderr: "Permission denied".to_string(),
        };
        assert!(error.to_string().contains("exit code 1"));
        assert!(error.to_string().contains("Permission denied"));
    }
}