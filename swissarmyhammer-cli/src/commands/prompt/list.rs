//! Simplified list command handler
//!
//! This module previously contained list command handlers that have been
//! replaced by the unified implementation in mod.rs. The tests remain
//! to verify the display functionality.

#[cfg(test)]
mod tests {
    use crate::commands::prompt::{cli, PromptCommand};
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_list_command_integration() {
        // Test the list command through the main command handler
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

        // Test that the list command executes without error
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test] 
    async fn test_list_command_verbose_mode() {
        // Test the list command in verbose mode
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
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_json_format() {
        // Test the list command with JSON output format
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
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_yaml_format() {
        // Test the list command with YAML output format
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
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }
}
