//! Prompt command implementation
//!
//! Manages and tests prompts with support for listing, validating, and testing

pub mod cli;
pub mod display;
pub mod list;
pub mod test;

use crate::error::{CliError, CliResult};
use crate::exit_codes::EXIT_SUCCESS;

pub use cli::PromptCommand;

/// Handle prompt command using the new CLI module types
///
/// Note: Help text for the prompt command is loaded from description.md
/// and available for reference but no longer used in CLI definitions
pub async fn handle_command_typed(
    command: PromptCommand,
    context: &crate::context::CliContext,
) -> i32 {
    match run_prompt_command_typed(command, context).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Prompt command failed: {}", e);
            e.exit_code
        }
    }
}

/// Main entry point for prompt command using new typed commands
async fn run_prompt_command_typed(
    command: PromptCommand,
    context: &crate::context::CliContext,
) -> CliResult<()> {
    match command {
        PromptCommand::List(_) => list::execute_list_command(context)
            .await
            .map_err(|e| CliError::new(e.to_string(), 1)),
        PromptCommand::Test(test_cmd) => test::execute_test_command(test_cmd, context)
            .await
            .map_err(|e| CliError::new(e.to_string(), 1)),
        PromptCommand::Validate(_) => {
            // For now, just delegate to the root validate command
            // This provides basic validation functionality
            run_validate_command()
                .await
                .map_err(|e| CliError::new(e.to_string(), 1))
        }
    }
}

/// Run the validate command - delegates to root validate functionality
async fn run_validate_command() -> Result<(), anyhow::Error> {
    // For now, delegate to the main validate command functionality
    // This uses the validate command implementation from the main CLI
    let exit_code = crate::validate::run_validate_command_with_dirs(
        false,
        crate::cli::OutputFormat::Table,
        vec![],
        false,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Validation failed: {}", e))?;

    if exit_code == 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Validation failed with exit code {}",
            exit_code
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_run_prompt_command_typed_list() {
        use crate::context::CliContextBuilder;

        // Create a List command using the new typed system
        let command = PromptCommand::List(cli::ListCommand {});

        // Create a mock CliContext for testing
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

        // Run the command - we expect it to succeed
        let result = run_prompt_command_typed(command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_prompt_command_typed_test_with_invalid_prompt() {
        use crate::context::CliContextBuilder;

        // Create a Test command with a non-existent prompt using the new typed system
        let command = PromptCommand::Test(cli::TestCommand {
            prompt_name: Some("non_existent_prompt_12345".to_string()),
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        });

        // Create a mock CliContext for testing
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

        // Run the command - should return an error
        let result = run_prompt_command_typed(command, &context).await;
        assert!(result.is_err());

        // Verify the error has the expected exit code
        if let Err(e) = result {
            assert_eq!(e.exit_code, 1);
        }
    }

    #[test]
    fn test_prompt_is_partial_template() {
        use swissarmyhammer_prompts::Prompt;

        // Test template with partial marker
        let partial_prompt = Prompt {
            name: "test-partial".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "{% partial %}\nThis is a partial template".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(partial_prompt.is_partial_template());

        // Test template with partial description
        let partial_desc_prompt = Prompt {
            name: "test-partial-desc".to_string(),
            description: Some("Partial template for reuse in other prompts".to_string()),
            category: None,
            tags: vec![],
            template: "Regular template content".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(partial_desc_prompt.is_partial_template());

        // Test regular template
        let regular_prompt = Prompt {
            name: "test-regular".to_string(),
            description: Some("A regular prompt".to_string()),
            category: None,
            tags: vec![],
            template: "This is a regular template".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(!regular_prompt.is_partial_template());
    }
}
