//! Simplified list command handler
//!
//! Provides a clean, simplified interface for listing all available prompts
//! without source/category filtering complexity.

use crate::commands::prompt::display::prompts_to_display_rows;
use crate::context::CliContext;
use anyhow::Result;

/// Execute the list command - shows all available prompts
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Get prompts from CliContext (reuses existing library/resolver)
    let prompts = cli_context.get_all_prompts()?;

    // Convert to display format based on verbose flag from CliContext
    let display_rows = prompts_to_display_rows(prompts, cli_context.verbose);

    // Use CliContext to handle output formatting
    cli_context.display_prompts(display_rows)?;

    Ok(())
}

/// Public interface for list command - ready for integration
pub async fn handle_list_command(cli_context: &CliContext) -> Result<i32> {
    match execute_list_command(cli_context).await {
        Ok(_) => Ok(crate::exit_codes::EXIT_SUCCESS),
        Err(e) => {
            eprintln!("List command failed: {}", e);
            Ok(crate::exit_codes::EXIT_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_execute_list_command() {
        // Create a test CliContext
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

        // Test that the command executes without error
        let result = execute_list_command(&context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_list_command_success() {
        // Create a test CliContext
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

        // Test that the command returns success exit code
        let result = handle_list_command(&context).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), crate::exit_codes::EXIT_SUCCESS);
    }

    #[tokio::test]
    async fn test_execute_list_command_verbose() {
        // Create a test CliContext with verbose enabled
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();

        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(true) // Enable verbose mode
            .debug(false)
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        // Test that the command executes without error in verbose mode
        let result = execute_list_command(&context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_list_command_json_format() {
        // Create a test CliContext with JSON format
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();

        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Json)
            .format_option(Some(crate::cli::OutputFormat::Json))
            .verbose(false)
            .debug(false)
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        // Test that the command executes without error with JSON format
        let result = execute_list_command(&context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_list_command_yaml_format() {
        // Create a test CliContext with YAML format
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();

        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Yaml)
            .format_option(Some(crate::cli::OutputFormat::Yaml))
            .verbose(false)
            .debug(false)
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        // Test that the command executes without error with YAML format
        let result = execute_list_command(&context).await;
        assert!(result.is_ok());
    }
}
