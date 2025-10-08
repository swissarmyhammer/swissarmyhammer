//! Check command implementation for rules
//!
//! Checks code files against rules to find violations

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::sync::Arc;
use swissarmyhammer_agent_executor::AgentExecutor;
use swissarmyhammer_config::agent::{AgentConfig, AgentExecutorConfig};
use swissarmyhammer_rules::{RuleCheckRequest, RuleChecker};

use super::cli::CheckCommand;

/// Request structure for check command execution
///
/// Combines the command parameters with optional agent configuration
/// for testability and cleaner function signatures.
struct CheckCommandRequest {
    cmd: CheckCommand,
    agent_config: Option<AgentConfig>,
}

impl CheckCommandRequest {
    /// Create a new request with default agent configuration
    fn new(cmd: CheckCommand) -> Self {
        Self {
            cmd,
            agent_config: None,
        }
    }

    /// Create a new request with explicit agent configuration (for testing)
    #[cfg(test)]
    fn with_config(cmd: CheckCommand, agent_config: AgentConfig) -> Self {
        Self {
            cmd,
            agent_config: Some(agent_config),
        }
    }
}

/// Execute the check command to verify code against rules
///
/// This command delegates to the rules crate's high-level API which:
/// 1. Loads all available rules from the rules directory
/// 2. Validates all rules to ensure they're well-formed
/// 3. Applies user-specified filters (rule names, severity, category)
/// 4. Expands glob patterns to get target files
/// 5. Creates rule checker with LLM agent
/// 6. Runs checks with fail-fast behavior on violations
///
/// # Arguments
/// * `cmd` - The parsed CheckCommand with patterns and filters
/// * `context` - CLI context with output settings
///
/// # Returns
/// * `Ok(())` if all checks pass or no rules/files match filters
/// * `Err(CliError)` if validation fails or violations are found
///
/// # Examples
/// ```bash
/// sah rule check "**/*.rs"
/// sah rule check --severity error "src/**/*.rs"
/// sah rule check --rule no-unwrap --category style "*.rs"
/// ```
pub async fn execute_check_command(cmd: CheckCommand, context: &CliContext) -> CliResult<()> {
    let request = CheckCommandRequest::new(cmd);
    execute_check_command_impl(request, context).await
}

/// Internal implementation of check command with injectable agent configuration
///
/// This function is identical to `execute_check_command` but accepts a request
/// structure that can include agent configuration for testing purposes. In production
/// use, the configuration is loaded from the environment. In tests, a test
/// configuration can be provided to avoid expensive executor initialization.
///
/// # Arguments
/// * `request` - CheckCommandRequest containing command and optional agent config
/// * `context` - CLI context with output settings
///
/// # Returns
/// * `Ok(())` if all checks pass or no rules/files match filters
/// * `Err(CliError)` if validation fails or violations are found
async fn execute_check_command_impl(
    request: CheckCommandRequest,
    context: &CliContext,
) -> CliResult<()> {
    // Load agent configuration (respects SAH_AGENT_EXECUTOR env var, defaults to ClaudeCode)
    // For tests, use provided config (LlamaAgent), otherwise use default
    let agent_config = request.agent_config.unwrap_or_default();

    // Create and initialize executor based on type
    let executor: Box<dyn AgentExecutor> = match &agent_config.executor {
        AgentExecutorConfig::LlamaAgent(llama_config) => {
            // Start MCP server for LlamaAgent
            use swissarmyhammer_agent_executor::llama::{
                LlamaAgentExecutorWrapper, McpServerHandle,
            };
            use swissarmyhammer_prompts::PromptLibrary;
            use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

            tracing::info!("Starting MCP server for LlamaAgent");
            let tools_mcp_handle = start_mcp_server(
                McpServerMode::Http {
                    port: if llama_config.mcp_server.port == 0 {
                        None
                    } else {
                        Some(llama_config.mcp_server.port)
                    },
                },
                Some(PromptLibrary::default()),
            )
            .await
            .map_err(|e| CliError::new(format!("Failed to start MCP server: {}", e), 1))?;

            tracing::info!(
                "MCP server started on port {:?}",
                tools_mcp_handle.info.port
            );

            // Convert tools McpServerHandle to agent-executor McpServerHandle
            // The two types are structurally identical but from different crates.
            // We create a dummy shutdown channel because the tools MCP handle manages
            // the server lifecycle, and we only need the port/host info for the agent.
            let port = tools_mcp_handle.info.port.unwrap_or(0);
            let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
            let agent_mcp_handle = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

            let mut exec = LlamaAgentExecutorWrapper::new_with_mcp(
                llama_config.clone(),
                Some(agent_mcp_handle),
            );
            exec.initialize().await.map_err(|e| {
                CliError::new(
                    format!("Failed to initialize LlamaAgent executor: {}", e),
                    1,
                )
            })?;
            Box::new(exec)
        }
        AgentExecutorConfig::ClaudeCode(_claude_config) => {
            use swissarmyhammer_agent_executor::ClaudeCodeExecutor;

            tracing::info!("Using ClaudeCode executor for rule checking");
            let mut exec = ClaudeCodeExecutor::new();
            exec.initialize().await.map_err(|e| {
                CliError::new(
                    format!("Failed to initialize ClaudeCode executor: {}", e),
                    1,
                )
            })?;
            Box::new(exec)
        }
    };

    let agent: Arc<dyn AgentExecutor> = Arc::from(executor);
    let checker = RuleChecker::new(agent)
        .map_err(|e| CliError::new(format!("Failed to create rule checker: {}", e), 1))?;

    // Parse severity from string if provided
    let severity = request
        .cmd
        .severity
        .as_ref()
        .map(|s| s.parse().map_err(|e: String| CliError::new(e, 1)))
        .transpose()?;

    // Create rule check request with filters
    let rule_request = RuleCheckRequest {
        rule_names: request.cmd.rule,
        severity,
        category: request.cmd.category,
        patterns: request.cmd.patterns,
    };

    // Run check with filters - this handles all the logic
    match checker.check_with_filters(rule_request).await {
        Ok(result) => {
            // Print results if not quiet
            if !context.quiet && result.rules_checked == 0 {
                println!("No rules matched the filters");
            }
            Ok(())
        }
        Err(e) => match e {
            swissarmyhammer_common::SwissArmyHammerError::RuleViolation(_) => {
                // Violation was already logged by checker at appropriate level
                Err(CliError::new("Rule violation found".to_string(), 1))
            }
            _ => {
                // Other errors need to be logged
                Err(CliError::new(format!("Check failed: {}", e), 1))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::{LlamaAgentConfig, TemplateContext};

    /// Helper function to create a test CLI context with standard settings
    async fn setup_test_context() -> CliContext {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap()
    }

    /// Helper function to create a test agent configuration
    fn setup_test_agent_config() -> AgentConfig {
        AgentConfig::llama_agent(LlamaAgentConfig::for_testing())
    }

    #[tokio::test]
    async fn test_execute_check_command_no_rules() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed when no rules match filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_no_files() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
            rule: None,
            severity: None,
            category: None,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed when no files match patterns
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_severity() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: Some("error".to_string()),
            category: None,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only error-level rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_category() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: None,
            category: Some("security".to_string()),
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only security category rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_rule_name() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: None,
            category: None,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only specified rule
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_combined_filters() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: Some("error".to_string()),
            category: Some("security".to_string()),
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - applies all filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_with_claude_code() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
        };

        // Request ClaudeCode - it should work now without fallback
        let request = CheckCommandRequest::with_config(cmd, AgentConfig::claude_code());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - ClaudeCode is fully supported
        assert!(
            result.is_ok(),
            "ClaudeCode should work for rule checking without fallback"
        );
    }
}
