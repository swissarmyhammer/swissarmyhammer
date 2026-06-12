//! Model use command implementation
//! sah rule ignore test_rule_with_allow

use crate::context::CliContext;
use colored::Colorize;
use swissarmyhammer_config::model::{
    ModelError as AgentError, ModelManager, ModelPaths, ModelTarget,
};

/// Maximum number of agent name suggestions to display when an agent is not found
const MAX_SUGGESTIONS: usize = 3;

/// Maximum number of available models to display in error messages
const MAX_MODELS_TO_DISPLAY: usize = 5;

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

/// Display success message after setting model
fn display_success_message(agent_name: &str, target: ModelTarget) {
    match target {
        ModelTarget::Default => println!(
            "{} Successfully set model to: {}",
            "✓".green(),
            agent_name.green().bold()
        ),
        ModelTarget::Review => println!(
            "{} Successfully set {} model to: {}",
            "✓".green(),
            "review".cyan().bold(),
            agent_name.green().bold()
        ),
    }

    // Try to get agent info for additional context
    if let Ok(agent_info) = ModelManager::find_agent_by_name(agent_name) {
        println!("   Source: {}", format_agent_source(&agent_info.source));

        if let Some(description) = &agent_info.description {
            println!("   Description: {}", description.dimmed());
        }
    }
}

/// The single source of truth for `--for <purpose>` values.
///
/// Each entry maps a CLI purpose name to the [`ModelTarget`] it writes. Both the
/// clap value-parser (`dynamic_cli::CliBuilder::build_model_command`) and the
/// programmatic [`target_for_purpose`] matcher consume this table, so adding a
/// new purpose (e.g. `commit`) is a one-line edit here rather than two
/// independent edits that can drift.
pub const SUPPORTED_PURPOSES: &[(&str, ModelTarget)] = &[("review", ModelTarget::Review)];

/// The purpose names accepted on `--for`, in declaration order.
///
/// This is the list the clap layer hands to its `PossibleValuesParser`, derived
/// from [`SUPPORTED_PURPOSES`] so the parser and the matcher cannot diverge.
pub fn supported_purpose_names() -> Vec<&'static str> {
    SUPPORTED_PURPOSES.iter().map(|(name, _)| *name).collect()
}

/// Map an optional `--for <purpose>` value to a [`ModelTarget`].
///
/// Absent (`None`) selects the global default. Recognized purposes come from
/// [`SUPPORTED_PURPOSES`]; any other value is rejected with a clear,
/// non-panicking error so an unknown `--for` fails the command rather than
/// silently writing the default.
fn target_for_purpose(
    purpose: Option<&str>,
) -> Result<ModelTarget, Box<dyn std::error::Error + Send + Sync>> {
    let Some(purpose) = purpose else {
        return Ok(ModelTarget::Default);
    };

    SUPPORTED_PURPOSES
        .iter()
        .find(|(name, _)| *name == purpose)
        .map(|(_, target)| *target)
        .ok_or_else(|| {
            format!(
                "Unknown --for purpose '{}'. Supported purposes: {}",
                purpose,
                supported_purpose_names().join(", ")
            )
            .into()
        })
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
    let primary_message = format!("Model '{}' not found", name);

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
    serde_err: serde_yaml_ng::Error,
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

/// Execute the model use command
///
/// `for_purpose` selects which configured model is written: `None` sets the
/// global default (top-level `model:`), `Some("review")` sets the review-tool
/// override (`review.model`). Any other purpose is rejected.
pub async fn execute_use_command(
    name: String,
    for_purpose: Option<String>,
    _context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let target = target_for_purpose(for_purpose.as_deref())?;

    let agent_name = name.trim().to_string();

    if agent_name.is_empty() {
        return Err("Model name cannot be empty".into());
    }

    tracing::info!("Setting {:?} model to: {}", target, agent_name);

    match ModelManager::use_agent_for(&agent_name, target, &ModelPaths::sah()) {
        Ok(()) => {
            display_success_message(&agent_name, target);
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
    use std::fs;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
    use swissarmyhammer_common::SwissarmyhammerDirectory;
    use swissarmyhammer_config::TemplateContext;

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

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_empty_agent_name() {
        // Isolate HOME + CWD — `create_test_context()` calls
        // `CliContextBuilder::build_async()`, which in turn calls
        // `get_swissarmyhammer_dir()` and creates `.sah/` at cwd. Without
        // isolation, even error-path tests leak `.sah/` into the host crate
        // directory.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

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
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_nonexistent_agent() {
        // Isolate HOME + CWD — see `test_execute_use_command_empty_agent_name`
        // for why `create_test_context()` requires this guard.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let context = create_test_context().await;

        let result = execute_use_command("nonexistent-agent-xyz".to_string(), None, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_builtin_agent() {
        // Isolate HOME + CWD — `ModelManager::use_agent(.., &ModelPaths::sah())`
        // writes `.sah/sah.yaml` at cwd and would otherwise leak a `.sah/`
        // skeleton into the host crate directory. Mirrors the pattern in
        // `commands::registry::tests::test_init_runs_without_panic`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

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
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_builtin_agent_explicit_empty_validation() {
        // Isolate HOME + CWD — `ModelManager::use_agent(.., &ModelPaths::sah())`
        // writes `.sah/sah.yaml` at cwd and would otherwise leak a `.sah/`
        // skeleton into the host crate directory. Mirrors the pattern in
        // `commands::registry::tests::test_init_runs_without_panic`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

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
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_with_temp_config() {
        // Isolate HOME + CWD using the canonical pattern. Mirrors the pattern
        // in `commands::registry::tests::test_init_runs_without_panic`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let temp_path = env.temp_dir();
        let _cwd = CurrentDirGuard::new(&temp_path).expect("cwd guard");

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
                // Config file may be at the canonicalized path (macOS /var -> /private/var).
                // Use canonicalize to find the real path.
                let canonical_temp = temp_path.canonicalize().unwrap_or(temp_path.clone());
                let config_path = canonical_temp
                    .join(SwissarmyhammerDirectory::dir_name())
                    .join("sah.yaml");
                assert!(
                    config_path.exists(),
                    "Config file should exist at {:?}",
                    config_path
                );
                let config_content = fs::read_to_string(&config_path).unwrap();
                assert!(
                    config_content.contains("model:"),
                    "Config should contain model key, got: {}",
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
    }

    #[test]
    fn test_supported_purposes_resolve_via_shared_table() {
        // Every purpose name advertised to clap must resolve through
        // `target_for_purpose` to its mapped target — proving both sites
        // consume the same `SUPPORTED_PURPOSES` source of truth.
        assert!(
            !SUPPORTED_PURPOSES.is_empty(),
            "there must be at least one supported purpose"
        );
        for (name, expected_target) in SUPPORTED_PURPOSES {
            let resolved = target_for_purpose(Some(name)).expect("supported purpose must resolve");
            assert_eq!(
                resolved, *expected_target,
                "purpose '{}' must map to its declared target",
                name
            );
        }
    }

    #[test]
    fn test_supported_purpose_names_match_table() {
        // The name list handed to the clap parser must be exactly the keys
        // of the shared table, in order.
        let from_table: Vec<&str> = SUPPORTED_PURPOSES.iter().map(|(name, _)| *name).collect();
        assert_eq!(supported_purpose_names(), from_table);
    }

    #[test]
    fn test_unknown_purpose_error_lists_supported() {
        let err = target_for_purpose(Some("deploy")).unwrap_err().to_string();
        assert!(
            err.contains("deploy"),
            "error names the bad purpose: {}",
            err
        );
        // The supported list in the error is derived from the table.
        for (name, _) in SUPPORTED_PURPOSES {
            assert!(
                err.contains(name),
                "error should list supported purpose '{}': {}",
                name,
                err
            );
        }
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

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_for_review_writes_review_model() {
        // Isolate HOME + CWD so the `.sah/sah.yaml` write lands in the temp dir.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let temp_path = env.temp_dir();
        let _cwd = CurrentDirGuard::new(&temp_path).expect("cwd guard");

        let context = create_test_context().await;

        // Use the first builtin agent so the agent-existence check passes.
        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        let agent_name = builtin_agents[0].name.clone();

        let result =
            execute_use_command(agent_name.clone(), Some("review".to_string()), &context).await;
        assert!(result.is_ok(), "review target should succeed: {:?}", result);

        let canonical_temp = temp_path.canonicalize().unwrap_or(temp_path.clone());
        let config_path = canonical_temp
            .join(SwissarmyhammerDirectory::dir_name())
            .join("sah.yaml");
        let config_content = fs::read_to_string(&config_path).unwrap();

        // The review model must be written under `review.model`, not at top level.
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&config_content).unwrap();
        let review_model = value
            .get("review")
            .and_then(|r| r.get("model"))
            .and_then(|m| m.as_str());
        assert_eq!(
            review_model,
            Some(agent_name.as_str()),
            "review.model should be set, got: {}",
            config_content
        );
        assert!(
            value.get("model").is_none(),
            "bare top-level model: must not be written for --for review, got: {}",
            config_content
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_default_writes_top_level_model() {
        // Isolate HOME + CWD so the `.sah/sah.yaml` write lands in the temp dir.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let temp_path = env.temp_dir();
        let _cwd = CurrentDirGuard::new(&temp_path).expect("cwd guard");

        let context = create_test_context().await;

        let builtin_agents =
            ModelManager::load_builtin_models().expect("Should be able to load builtin agents");
        let agent_name = builtin_agents[0].name.clone();

        let result = execute_use_command(agent_name.clone(), None, &context).await;
        assert!(
            result.is_ok(),
            "default target should succeed: {:?}",
            result
        );

        let canonical_temp = temp_path.canonicalize().unwrap_or(temp_path.clone());
        let config_path = canonical_temp
            .join(SwissarmyhammerDirectory::dir_name())
            .join("sah.yaml");
        let config_content = fs::read_to_string(&config_path).unwrap();

        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&config_content).unwrap();
        assert_eq!(
            value.get("model").and_then(|m| m.as_str()),
            Some(agent_name.as_str()),
            "top-level model: should be set for the default path, got: {}",
            config_content
        );
        assert!(
            value.get("review").is_none(),
            "review: must not be written for the default path, got: {}",
            config_content
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_unknown_purpose_rejected() {
        // Isolate HOME + CWD — see `test_execute_use_command_empty_agent_name`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let context = create_test_context().await;

        let result = execute_use_command(
            "claude-code".to_string(),
            Some("deploy".to_string()),
            &context,
        )
        .await;
        assert!(result.is_err(), "unknown purpose should be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("deploy"),
            "error should name the unknown purpose, got: {}",
            msg
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_use_command_for_review_empty_name_rejected() {
        // Isolate HOME + CWD — see `test_execute_use_command_empty_agent_name`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let context = create_test_context().await;

        let result =
            execute_use_command("   ".to_string(), Some("review".to_string()), &context).await;
        assert!(result.is_err(), "empty name should be rejected");
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    // Integration-style test for error message formatting
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_error_message_format() {
        // Isolate HOME + CWD — see `test_execute_use_command_empty_agent_name`
        // for why `create_test_context()` requires this guard.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let context = create_test_context().await;

        // Test that error messages are properly formatted
        let result =
            execute_use_command("definitely-not-an-agent-12345".to_string(), None, &context).await;
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("not found"));
        assert!(error_msg.contains("definitely-not-an-agent-12345"));
    }
}
