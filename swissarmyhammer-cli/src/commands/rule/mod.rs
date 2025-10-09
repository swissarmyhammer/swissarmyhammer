//! Rule command implementation
//!
//! Manages and tests rules with support for listing, validating, and checking

pub mod cache;
pub mod check;
pub mod cli;
pub mod display;
pub mod list;
pub mod validate;

use crate::error::CliResult;
use crate::exit_codes::EXIT_SUCCESS;

pub use cli::RuleCommand;

/// Help text for the rule command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle rule command using the new CLI module types
pub async fn handle_command_typed(
    command: RuleCommand,
    context: &crate::context::CliContext,
) -> i32 {
    match run_rule_command_typed(command, context).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("{}", e.full_chain());
            e.exit_code
        }
    }
}

/// Main entry point for rule command using new typed commands
async fn run_rule_command_typed(
    command: RuleCommand,
    context: &crate::context::CliContext,
) -> CliResult<()> {
    match command {
        RuleCommand::List(_) => list::execute_list_command(context)
            .await
            .map_err(|e| crate::error::CliError::new(e.to_string(), 1)),
        RuleCommand::Validate(validate_cmd) => {
            validate::execute_validate_command(validate_cmd, context).await
        }
        RuleCommand::Check(check_cmd) => check::execute_check_command(check_cmd, context).await,
        RuleCommand::Cache(cache_cmd) => cache::execute_cache_command(cache_cmd, context).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_run_rule_command_typed_list() {
        use crate::context::CliContextBuilder;

        let command = RuleCommand::List(cli::ListCommand {});

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
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let result = run_rule_command_typed(command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_rule_command_typed_validate() {
        use crate::context::CliContextBuilder;

        let command = RuleCommand::Validate(cli::ValidateCommand {
            rule_name: Some("test-rule".to_string()),
            file: None,
        });

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
            .quiet(true) // Quiet to suppress error output in tests
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let result = run_rule_command_typed(command, &context).await;
        // Should return error when rule not found
        assert!(result.is_err());
    }
}
