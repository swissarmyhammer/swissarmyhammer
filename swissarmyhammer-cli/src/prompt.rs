//! Prompt command implementation for managing prompts

use crate::cli::PromptSubcommand;
use crate::error::{CliError, CliResult};
use crate::{list, search, test};

/// Main entry point for prompt command
pub async fn run_prompt_command(subcommand: PromptSubcommand) -> CliResult<()> {
    match subcommand {
        PromptSubcommand::List {
            format,
            verbose,
            source,
            category,
            search,
        } => list::run_list_command(format, verbose, source, category, search)
            .map(|_| ())
            .map_err(|e| CliError::new(e.to_string(), 1)),
        PromptSubcommand::Test {
            prompt_name,
            file,
            vars,
            raw,
            copy,
            save,
            debug,
        } => {
            let all_vars = vars;

            let mut runner = test::TestRunner::new();
            let config = test::TestConfig {
                prompt_name,
                file,
                arguments: all_vars,
                raw,
                copy,
                save,
                debug,
            };
            runner.run(config).await.map(|_| ()).map_err(|e| {
                // Check if the underlying error is a SwissArmyHammerError with abort
                if let Some(swissarmyhammer::SwissArmyHammerError::ExecutorError(
                    swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
                )) = e.downcast_ref::<swissarmyhammer::SwissArmyHammerError>()
                {
                    return CliError::new(
                        format!("Prompt execution aborted: {abort_reason}"),
                        crate::exit_codes::EXIT_ERROR,
                    );
                }
                CliError::new(e.to_string(), 1)
            })
        }
        PromptSubcommand::Search {
            query,
            r#in,
            regex,
            fuzzy,
            case_sensitive,
            source,
            has_arg,
            no_args,
            full,
            format,
            highlight,
            limit,
        } => search::run_search_command(
            query,
            r#in,
            regex,
            fuzzy,
            case_sensitive,
            source,
            has_arg,
            no_args,
            full,
            format,
            highlight,
            limit,
        )
        .map(|_| ())
        .map_err(|e| CliError::new(e.to_string(), 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PromptSubcommand;

    #[tokio::test]
    async fn test_run_prompt_command_list() {
        // Create a List subcommand with minimal arguments
        let subcommand = PromptSubcommand::List {
            format: crate::cli::OutputFormat::Table,
            verbose: false,
            source: None,
            category: None,
            search: None,
        };

        // Run the command - we expect it to succeed
        let result = run_prompt_command(subcommand).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_prompt_command_search() {
        // Create a Search subcommand with a simple query
        let subcommand = PromptSubcommand::Search {
            query: "test".to_string(),
            r#in: None,
            regex: false,
            fuzzy: false,
            case_sensitive: false,
            source: None,
            has_arg: None,
            no_args: false,
            full: false,
            format: crate::cli::OutputFormat::Table,
            highlight: true,
            limit: None,
        };

        // Run the command
        let result = run_prompt_command(subcommand).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_prompt_command_test_with_invalid_prompt() {
        // Create a Test subcommand with a non-existent prompt
        let subcommand = PromptSubcommand::Test {
            prompt_name: Some("non_existent_prompt_12345".to_string()),
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        // Run the command - should return an error
        let result = run_prompt_command(subcommand).await;
        assert!(result.is_err());

        // Verify the error has the expected exit code
        if let Err(e) = result {
            assert_eq!(e.exit_code, 1);
        }
    }
}
