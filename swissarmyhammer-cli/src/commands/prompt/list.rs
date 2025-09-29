//! List command handler for prompts
//!
//! Handles listing all available prompts with filtering and display options.
//! Filters out partial templates and provides both standard and verbose output modes.

use crate::context::CliContext;
use anyhow::Result;
use std::collections::HashMap;
use swissarmyhammer::{PromptFilter, PromptLibrary, PromptResolver};

/// Execute the list command - shows all available prompts
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Load all prompts from all sources
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Build a basic filter - no source or category filtering for simplified list command
    let filter = PromptFilter::new();

    // Get file sources from resolver for emoji-based display
    let mut file_sources = HashMap::new();
    let mut prompt_sources = HashMap::new();
    for (name, source) in &resolver.prompt_sources {
        file_sources.insert(name.clone(), source.clone());
        // Convert FileSource to PromptSource for the library API
        let prompt_source: swissarmyhammer_prompts::PromptSource = source.clone().into();
        prompt_sources.insert(name.clone(), prompt_source);
    }
    
    let all_prompts = library.list_filtered(&filter, &prompt_sources)?;

    // Filter out partial templates
    let prompts: Vec<_> = all_prompts
        .into_iter()
        .filter(|prompt| !prompt.is_partial_template())
        .collect();

    // Convert to display objects using emoji-based sources and use context's display_prompts method
    let display_rows = super::display::prompts_to_display_rows_with_sources(
        prompts, 
        &file_sources, 
        cli_context.verbose
    );
    cli_context.display_prompts(display_rows)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commands::prompt::{cli, PromptCommand};
    use crate::context::CliContextBuilder;
    use std::collections::HashMap;
    use swissarmyhammer_config::TemplateContext;

    async fn create_test_context(
        format: crate::cli::OutputFormat,
        verbose: bool,
        debug: bool,
        quiet: bool,
    ) -> crate::context::CliContext {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();

        CliContextBuilder::default()
            .template_context(template_context)
            .format(format)
            .format_option(Some(format))
            .verbose(verbose)
            .debug(debug)
            .quiet(quiet)
            .matches(matches)
            .build_async()
            .await
            .unwrap()
    }

    fn create_test_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::json!(format!("{} Title", name)),
        );

        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: Some(format!("{} description", name)),
            category: Some("test".to_string()),
            tags: vec!["test".to_string()],
            template: format!("{} template content", name),
            parameters: vec![],
            source: Some(std::path::PathBuf::from(format!("/test/path/{}.md", name))),
            metadata,
        }
    }

    fn create_partial_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: format!("{{% partial %}}\n{} partial content", name),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        }
    }

    fn create_partial_description_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: Some("Partial template for reuse in other prompts".to_string()),
            category: None,
            tags: vec![],
            template: format!("{} content", name),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_list_command_integration() {
        // Test the list command through the main command handler
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, false, false).await;

        // Test that the list command executes without error
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_mode() {
        // Test the list command in verbose mode
        let context =
            create_test_context(crate::cli::OutputFormat::Table, true, false, false).await;

        // Test that the command executes without error in verbose mode
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_json_format() {
        // Test the list command with JSON output format
        let context =
            create_test_context(crate::cli::OutputFormat::Json, false, false, false).await;

        // Test that the command executes without error with JSON format
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_yaml_format() {
        // Test the list command with YAML output format
        let context =
            create_test_context(crate::cli::OutputFormat::Yaml, false, false, false).await;

        // Test that the command executes without error with YAML format
        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_debug_mode() {
        // Test the list command with debug enabled
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, true, false).await;

        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_quiet_mode() {
        // Test the list command with quiet mode enabled
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, false, true).await;

        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_json() {
        // Test combination of verbose and JSON output
        let context = create_test_context(crate::cli::OutputFormat::Json, true, false, false).await;

        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_yaml() {
        // Test combination of verbose and YAML output
        let context = create_test_context(crate::cli::OutputFormat::Yaml, true, false, false).await;

        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_all_flags() {
        // Test with all flags enabled (verbose, debug, but not quiet as they conflict)
        let context = create_test_context(crate::cli::OutputFormat::Json, true, true, false).await;

        let list_command = PromptCommand::List(cli::ListCommand {});
        let result = super::super::run_prompt_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    // Unit tests for filtering functionality that's used by the list command
    #[test]
    fn test_filter_display_prompts_removes_partials() {
        let prompts = vec![
            create_test_prompt("regular1"),
            create_partial_prompt("partial1"),
            create_test_prompt("regular2"),
            create_partial_description_prompt("partial2"),
        ];

        // This tests the logic that would be in run_list_command for filtering partials
        let filtered: Vec<_> = prompts
            .into_iter()
            .filter(|prompt| !prompt.is_partial_template())
            .collect();

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "regular1");
        assert_eq!(filtered[1].name, "regular2");
    }

    #[test]
    fn test_filter_display_prompts_all_regular() {
        let prompts = vec![
            create_test_prompt("regular1"),
            create_test_prompt("regular2"),
            create_test_prompt("regular3"),
        ];

        let filtered: Vec<_> = prompts
            .into_iter()
            .filter(|prompt| !prompt.is_partial_template())
            .collect();

        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].name, "regular1");
        assert_eq!(filtered[1].name, "regular2");
        assert_eq!(filtered[2].name, "regular3");
    }

    #[test]
    fn test_filter_display_prompts_all_partials() {
        let prompts = vec![
            create_partial_prompt("partial1"),
            create_partial_description_prompt("partial2"),
        ];

        let filtered: Vec<_> = prompts
            .into_iter()
            .filter(|prompt| !prompt.is_partial_template())
            .collect();

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_display_prompts_empty() {
        let prompts: Vec<swissarmyhammer_prompts::Prompt> = vec![];

        let filtered: Vec<_> = prompts
            .into_iter()
            .filter(|prompt| !prompt.is_partial_template())
            .collect();

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_display_rows_conversion_with_filtering() {
        // Test the complete flow: create prompts, filter, then convert to display rows
        let prompts = vec![
            create_test_prompt("regular1"),
            create_partial_prompt("partial1"),
            create_test_prompt("regular2"),
        ];

        let filtered: Vec<_> = prompts
            .into_iter()
            .filter(|prompt| !prompt.is_partial_template())
            .collect();

        // Test standard conversion
        let sources = std::collections::HashMap::new();
        let display_rows = super::super::display::prompts_to_display_rows_with_sources(filtered.clone(), &sources, false);
        match display_rows {
            super::super::display::DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].name, "regular1");
                assert_eq!(rows[1].name, "regular2");
            }
            _ => panic!("Expected Standard display rows"),
        }

        // Test verbose conversion
        let display_rows = super::super::display::prompts_to_display_rows_with_sources(filtered, &sources, true);
        match display_rows {
            super::super::display::DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].name, "regular1");
                assert_eq!(rows[1].name, "regular2");
                assert_eq!(rows[0].description, "regular1 description");
                assert_eq!(rows[1].description, "regular2 description");
            }
            _ => panic!("Expected Verbose display rows"),
        }
    }

    // Test the create_test_context helper itself
    #[tokio::test]
    async fn test_create_test_context_variations() {
        // Test different combinations to ensure the helper works correctly
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, false, false).await;
        assert!(!context.verbose);
        assert!(!context.debug);
        assert!(!context.quiet);

        let context = create_test_context(crate::cli::OutputFormat::Json, true, true, true).await;
        assert!(context.verbose);
        assert!(context.debug);
        assert!(context.quiet);
    }
}
