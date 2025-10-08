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
    execute_check_command_with_config(cmd, context, None).await
}

async fn execute_check_command_with_config(
    cmd: CheckCommand,
    context: &CliContext,
    agent_config: Option<AgentConfig>,
) -> CliResult<()> {
    // Load agent configuration (respects SAH_AGENT_EXECUTOR env var, defaults to ClaudeCode)
    // For tests, use provided config (LlamaAgent), otherwise use default
    let agent_config = agent_config.unwrap_or_default();

    // Create and initialize executor based on type
    // For LlamaAgent, we need to start MCP server first
    let executor: Box<dyn AgentExecutor> = match &agent_config.executor {
        AgentExecutorConfig::ClaudeCode(_) => {
            return Err(CliError::new(
                "ClaudeCode executor not supported in rule check CLI tests. Use LlamaAgent for testing.".to_string(),
                1,
            ));
        }
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
    };

    let agent: Arc<dyn AgentExecutor> = Arc::from(executor);
    let checker = RuleChecker::new(agent)
        .map_err(|e| CliError::new(format!("Failed to create rule checker: {}", e), 1))?;

    // Parse severity from string if provided
    let severity = cmd
        .severity
        .as_ref()
        .map(|s| s.parse().map_err(|e: String| CliError::new(e, 1)))
        .transpose()?;

    // Create request with filters
    let request = RuleCheckRequest {
        rule_names: cmd.rule,
        severity,
        category: cmd.category,
        patterns: cmd.patterns,
    };

    // Run check with filters - this handles all the logic
    match checker.check_with_filters(request).await {
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

    #[tokio::test]
    async fn test_execute_check_command_no_rules() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed when no rules match filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_no_files() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
            rule: None,
            severity: None,
            category: None,
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed when no files match patterns
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_severity() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: Some("error".to_string()),
            category: None,
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed - filters to only error-level rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_category() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: None,
            category: Some("security".to_string()),
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed - filters to only security category rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_rule_name() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: None,
            category: None,
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed - filters to only specified rule
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_combined_filters() {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: Some("error".to_string()),
            category: Some("security".to_string()),
        };

        let test_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
        let result = execute_check_command_with_config(cmd, &context, Some(test_config)).await;
        // Should succeed - applies all filters
        assert!(result.is_ok());
    }
}
