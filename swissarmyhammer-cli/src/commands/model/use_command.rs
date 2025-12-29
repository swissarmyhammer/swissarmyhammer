//! Agent use command implementation
//! sah rule ignore test_rule_with_allow

use crate::context::CliContext;
use colored::Colorize;
use swissarmyhammer_config::model::{ModelError as AgentError, ModelManager};
use swissarmyhammer_config::AgentUseCase;

/// Maximum number of agent name suggestions to display when an agent is not found
const MAX_SUGGESTIONS: usize = 3;

/// Maximum number of available models to display in error messages
const MAX_MODELS_TO_DISPLAY: usize = 5;

/// Parse use command arguments into use case and agent name
fn parse_use_command_args(
    first: String,
    second: Option<String>,
) -> Result<(AgentUseCase, String), String> {
    let (use_case, agent_name) = if let Some(agent) = second {
        // Two arguments: use_case agent_name
        let use_case = first
            .trim()
            .parse::<AgentUseCase>()
            .map_err(|e| format!("Invalid use case: {}", e))?;
        (use_case, agent.trim().to_string())
    } else {
        // One argument: just agent name (root use case)
        (AgentUseCase::Root, first.trim().to_string())
    };

    // Input validation
    if agent_name.is_empty() {
        return Err("Model name cannot be empty".into());
    }

    Ok((use_case, agent_name))
}

/// Format agent source as colored string
fn format_agent_source(
    source: &swissarmyhammer_config::model::ModelConfigSource,
) -> colored::ColoredString {
    match source {
        swissarmyhammer_config::model::ModelConfigSource::Builtin => "builtin".green(),
        swissarmyhammer_config::model::ModelConfigSource::Project => "project".yellow(),
        swissarmyhammer_config::model::ModelConfigSource::GitRoot => "gitroot".cyan(),
        swissarmyhammer_config::model::ModelConfigSource::User => "user".blue(),
    }
}

/// Display success message after setting agent for use case
fn display_success_message(agent_name: &str, use_case: AgentUseCase) {
    println!(
        "{} Successfully set {} use case to model: {}",
        "✓".green(),
        use_case.to_string().cyan(),
        agent_name.green().bold()
    );

    // Try to get agent info for additional context
    if let Ok(agent_info) = ModelManager::find_agent_by_name(agent_name) {
        println!("   Source: {}", format_agent_source(&agent_info.source));

        if let Some(description) = &agent_info.description {
            println!("   Description: {}", description.dimmed());
        }
    }
}

/// Generic error handler that prints formatted error messages
fn handle_error(
    primary_message: String,
    additional_context: Option<String>,
    error_detail: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    eprintln!("{} {}", "✗".red(), primary_message.red());

    if let Some(context) = additional_context {
        eprintln!("{}", context);
    }

    Err(error_detail.into())
}

/// Handle agent not found error with suggestions
fn handle_agent_not_found(name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let primary_message = format!("Agent '{}' not found", name);

    let additional_context = match ModelManager::list_agents() {
        Ok(available_agents) => {
            let mut context_lines = Vec::new();

            // Simple suggestion logic - find agents with similar names
            let suggestions: Vec<_> = available_agents
                .iter()
                .filter(|agent| agent.name.contains(&name) || name.contains(&agent.name))
                .take(MAX_SUGGESTIONS)
                .collect();

            if !suggestions.is_empty() {
                context_lines.push("\nDid you mean:".to_string());
                for suggestion in suggestions {
                    context_lines.push(format!("  • {}", suggestion.name.cyan()));
                }
            } else {
                context_lines.push("\nAvailable models:".to_string());
                for agent in available_agents.iter().take(MAX_MODELS_TO_DISPLAY) {
                    let source = format_agent_source(&agent.source);
                    context_lines.push(format!("  • {} ({})", agent.name.cyan(), source.dimmed()));
                }
                if available_agents.len() > MAX_MODELS_TO_DISPLAY {
                    context_lines.push(format!(
                        "  ... and {} more",
                        available_agents.len() - MAX_MODELS_TO_DISPLAY
                    ));
                }
            }

            Some(context_lines.join("\n"))
        }
        Err(_) => None,
    };

    handle_error(primary_message.clone(), additional_context, primary_message)
}

/// Handle IO error
fn handle_io_error(io_err: std::io::Error) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_error(
        format!("Failed to update configuration: {}", io_err),
        Some("Check that you have write permissions to the config file and directory.".to_string()),
        format!("Configuration update failed: {}", io_err),
    )
}

/// Handle configuration error
fn handle_config_error(config_err: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_error(
        format!("Configuration error: {}", config_err),
        Some("The agent configuration may be invalid or corrupted.".to_string()),
        format!("Configuration error: {}", config_err),
    )
}

/// Handle parse error
fn handle_parse_error(
    serde_err: serde_yaml::Error,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_error(
        format!("Failed to process agent configuration: {}", serde_err),
        None,
        format!("Configuration processing failed: {}", serde_err),
    )
}

/// Handle invalid path error
fn handle_invalid_path_error(
    path: std::path::PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_error(
        format!("Invalid agent path: {}", path.display()),
        Some("The agent configuration file path is invalid or inaccessible.".to_string()),
        format!("Invalid agent path: {}", path.display()),
    )
}

/// Execute the agent use command
pub async fn execute_use_command(
    first: String,
    second: Option<String>,
    _context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (use_case, agent_name) = parse_use_command_args(first, second)?;

    tracing::info!("Setting {} use case to model: {}", use_case, agent_name);

    // Use ModelManager with use case support
    match ModelManager::use_agent_for_use_case(&agent_name, use_case) {
        Ok(()) => {
            display_success_message(&agent_name, use_case);
            Ok(())
        }
        Err(AgentError::NotFound(name)) => handle_agent_not_found(name),
        Err(AgentError::IoError(io_err)) => handle_io_error(io_err),
        Err(AgentError::ConfigError(config_err)) => handle_config_error(config_err),
        Err(AgentError::ParseError(serde_err)) => handle_parse_error(serde_err),
        Err(AgentError::InvalidPath(path)) => handle_invalid_path_error(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use std::env;
    use std::fs;
    use swissarmyhammer_config::TemplateContext;
    use tempfile::TempDir;

    /// Helper to create a test CliContext
    async fn create_test_context() -> CliContext {
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
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap()
    }

    /// Helper to validate that command execution succeeds or fails due to config issues, not logic errors
    async fn assert_use_command_config_only_failure(
        first: &str,
        second: Option<&str>,
        context: &CliContext,
    ) {
        let result =
            execute_use_command(first.to_string(), second.map(|s| s.to_string()), context).await;

        match result {
            Ok(()) => {
                // Success case - command completed successfully
            }
            Err(e) => {
                // Should only fail due to config issues, not parsing or logic errors
                let error_msg = e.to_string();
                assert!(
                    !error_msg.contains("Invalid use case"),
                    "Should not fail parsing valid use case, got: {}",
                    error_msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_execute_use_command_empty_agent_name() {
        let context = create_test_context().await;

        // Test empty string
        let result = execute_use_command("".to_string(), None, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        // Test whitespace-only string
        let result = execute_use_command("   ".to_string(), None, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_execute_use_command_nonexistent_agent() {
        let context = create_test_context().await;

        let result = execute_use_command("nonexistent-agent-xyz".to_string(), None, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_execute_use_command_builtin_agent() {
        let context = create_test_context().await;

        // Get the first available builtin agent dynamically
        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        assert!(
            !builtin_agents.is_empty(),
            "Should have at least one builtin agent"
        );

        let agent_name = &builtin_agents[0].name;

        let result = execute_use_command(agent_name.clone(), None, &context).await;
        // This might fail if no config directory exists, but we test the logic
        match result {
            Ok(()) => {
                // Success case - agent was found and used successfully
            }
            Err(e) => {
                // Should only fail due to config/permission issues, not agent not found
                let error_msg = e.to_string();
                assert!(
                    !error_msg.contains("not found"),
                    "Should not fail with 'not found' for builtin agent, got: {}",
                    error_msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_execute_use_command_builtin_agent_explicit_empty_validation() {
        let context = create_test_context().await;

        // Get the first available builtin agent dynamically
        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        assert!(
            !builtin_agents.is_empty(),
            "Should have at least one builtin agent"
        );

        let agent_name = &builtin_agents[0].name;

        // Test that agent names get properly trimmed
        let result = execute_use_command(format!("  {}  ", agent_name), None, &context).await;

        match result {
            Ok(()) => {
                // Success - agent name was properly trimmed and used
            }
            Err(e) => {
                // Should only fail due to config issues, not validation
                let error_msg = e.to_string();
                assert!(
                    !error_msg.contains("cannot be empty"),
                    "Should not fail validation after trimming, got: {}",
                    error_msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_execute_use_command_with_temp_config() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Change to temp directory so config gets created there
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_path).unwrap();

        let context = create_test_context().await;

        // Get the first available builtin agent dynamically
        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        assert!(
            !builtin_agents.is_empty(),
            "Should have at least one builtin agent"
        );

        let agent_name = &builtin_agents[0].name;

        let result = execute_use_command(agent_name.clone(), None, &context).await;

        // This should succeed or fail only due to permission/config issues
        match result {
            Ok(()) => {
                // Check that config file was created in .swissarmyhammer directory
                let config_path = temp_path.join(".swissarmyhammer").join("sah.yaml");
                assert!(
                    config_path.exists(),
                    "Config file should exist at {:?}",
                    config_path
                );
                let config_content = fs::read_to_string(&config_path).unwrap();
                assert!(
                    config_content.contains("agents:"),
                    "Config should contain agents map structure, got: {}",
                    config_content
                );
            }
            Err(e) => {
                // Should be a config/permission error, not agent not found
                let error_msg = e.to_string().to_lowercase();
                assert!(
                    error_msg.contains("configuration")
                        || error_msg.contains("permission")
                        || error_msg.contains("io")
                        || error_msg.contains("directory"),
                    "Should fail only with config/permission issues, got: {}",
                    e
                );
                assert!(!error_msg.contains("not found"));
            }
        }

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_agent_name_validation_logic() {
        // Test the trimming logic separately
        assert_eq!("".trim(), "");
        assert_eq!("  ".trim(), "");
        assert_eq!("  agent-name  ".trim(), "agent-name");
        assert_eq!("agent-name".trim(), "agent-name");

        // Validate our empty check logic
        assert!("".trim().is_empty());
        assert!("  ".trim().is_empty());
        assert!(!"agent-name".trim().is_empty());
    }

    // Integration-style test for error message formatting
    #[tokio::test]
    async fn test_error_message_format() {
        let context = create_test_context().await;

        // Test that error messages are properly formatted
        let result =
            execute_use_command("definitely-not-an-agent-12345".to_string(), None, &context).await;
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("not found"));
        assert!(error_msg.contains("definitely-not-an-agent-12345"));
    }

    // Test use case parsing with parameterized approach
    #[tokio::test]
    async fn test_execute_use_command_with_valid_use_cases() {
        let context = create_test_context().await;

        // Get the first available builtin agent dynamically
        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        assert!(
            !builtin_agents.is_empty(),
            "Should have at least one builtin agent"
        );

        let agent_name = &builtin_agents[0].name;

        // Test all valid use cases dynamically from the enum
        let valid_use_cases = [
            AgentUseCase::Root,
            AgentUseCase::Rules,
            AgentUseCase::Workflows,
        ];

        for use_case in valid_use_cases {
            assert_use_command_config_only_failure(
                &use_case.to_string(),
                Some(agent_name),
                &context,
            )
            .await;
        }
    }

    #[tokio::test]
    async fn test_execute_use_command_invalid_use_case() {
        let context = create_test_context().await;

        // Test with invalid use case
        let result = execute_use_command(
            "invalid".to_string(),
            Some("claude-code".to_string()),
            &context,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid use case"));
    }

    #[tokio::test]
    async fn test_use_nonexistent_agent_for_use_case() {
        let context = create_test_context().await;

        // Test setting a nonexistent agent for a use case
        let result = execute_use_command(
            AgentUseCase::Rules.to_string(),
            Some("nonexistent".to_string()),
            &context,
        )
        .await;

        assert!(result.is_err(), "Should fail for nonexistent agent");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("not found"),
            "Error should indicate agent was not found"
        );
    }
}
