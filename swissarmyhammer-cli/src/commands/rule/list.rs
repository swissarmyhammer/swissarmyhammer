//! List command implementation for rules
//!
//! Lists all available rules with their metadata

use crate::context::CliContext;
use anyhow::Result;
use swissarmyhammer_rules::RuleResolver;

/// Execute the list command to display available rules
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Load all rules from all sources (builtin → user → local)
    let mut resolver = RuleResolver::new();
    let mut rules = Vec::new();
    resolver.load_all_rules(&mut rules)?;

    // Get file sources from resolver for emoji-based display
    let file_sources = &resolver.rule_sources;

    // Filter out partial templates
    let rules: Vec<_> = rules
        .into_iter()
        .filter(|rule| !rule.is_partial())
        .collect();

    // Convert to display objects using emoji-based sources and use context's display_rules method
    let display_rows = super::display::rules_to_display_rows_with_sources(
        rules,
        file_sources,
        cli_context.verbose,
    );

    // Use context's display_rules method
    cli_context.display_rules(display_rows)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commands::rule::{cli, RuleCommand};
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

    fn create_test_rule(name: &str) -> swissarmyhammer_rules::Rule {
        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::json!(format!("{} Title", name)),
        );

        swissarmyhammer_rules::Rule {
            name: name.to_string(),
            description: Some(format!("{} description", name)),
            category: Some("test".to_string()),
            tags: vec!["test".to_string()],
            template: format!("{} template content", name),
            source: Some(std::path::PathBuf::from(format!("/test/path/{}.md", name))),
            metadata,
            severity: swissarmyhammer_rules::Severity::Error,
            auto_fix: false,
        }
    }

    fn create_partial_rule(name: &str) -> swissarmyhammer_rules::Rule {
        swissarmyhammer_rules::Rule {
            name: name.to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: format!("{{% partial %}}\n{} partial content", name),
            source: None,
            metadata: HashMap::new(),
            severity: swissarmyhammer_rules::Severity::Error,
            auto_fix: false,
        }
    }

    fn create_partial_description_rule(name: &str) -> swissarmyhammer_rules::Rule {
        swissarmyhammer_rules::Rule {
            name: name.to_string(),
            description: Some("Partial template for reuse in other rules".to_string()),
            category: None,
            tags: vec![],
            template: format!("{{% partial %}}\n{} content", name),
            source: None,
            metadata: HashMap::new(),
            severity: swissarmyhammer_rules::Severity::Error,
            auto_fix: false,
        }
    }

    #[tokio::test]
    async fn test_list_command_integration() {
        // Test the list command through the main command handler
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, false, false).await;

        // Test that the list command executes without error
        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_mode() {
        // Test the list command in verbose mode
        let context =
            create_test_context(crate::cli::OutputFormat::Table, true, false, false).await;

        // Test that the command executes without error in verbose mode
        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_json_format() {
        // Test the list command with JSON output format
        let context =
            create_test_context(crate::cli::OutputFormat::Json, false, false, false).await;

        // Test that the command executes without error with JSON format
        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_yaml_format() {
        // Test the list command with YAML output format
        let context =
            create_test_context(crate::cli::OutputFormat::Yaml, false, false, false).await;

        // Test that the command executes without error with YAML format
        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_debug_mode() {
        // Test the list command with debug enabled
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, true, false).await;

        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_quiet_mode() {
        // Test the list command with quiet mode enabled
        let context =
            create_test_context(crate::cli::OutputFormat::Table, false, false, true).await;

        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_json() {
        // Test combination of verbose and JSON output
        let context = create_test_context(crate::cli::OutputFormat::Json, true, false, false).await;

        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_verbose_yaml() {
        // Test combination of verbose and YAML output
        let context = create_test_context(crate::cli::OutputFormat::Yaml, true, false, false).await;

        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_command_all_flags() {
        // Test with all flags enabled (verbose, debug, but not quiet as they conflict)
        let context = create_test_context(crate::cli::OutputFormat::Json, true, true, false).await;

        let list_command = RuleCommand::List(cli::ListCommand {});
        let result = super::super::run_rule_command_typed(list_command, &context).await;
        assert!(result.is_ok());
    }

    // Unit tests for filtering functionality that's used by the list command
    #[test]
    fn test_filter_display_rules_removes_partials() {
        let rules = vec![
            create_test_rule("regular1"),
            create_partial_rule("partial1"),
            create_test_rule("regular2"),
            create_partial_description_rule("partial2"),
        ];

        // This tests the logic that would be in execute_list_command for filtering partials
        let filtered: Vec<_> = rules
            .into_iter()
            .filter(|rule| !rule.is_partial())
            .collect();

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "regular1");
        assert_eq!(filtered[1].name, "regular2");
    }

    #[test]
    fn test_filter_display_rules_all_regular() {
        let rules = vec![
            create_test_rule("regular1"),
            create_test_rule("regular2"),
            create_test_rule("regular3"),
        ];

        let filtered: Vec<_> = rules
            .into_iter()
            .filter(|rule| !rule.is_partial())
            .collect();

        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].name, "regular1");
        assert_eq!(filtered[1].name, "regular2");
        assert_eq!(filtered[2].name, "regular3");
    }

    #[test]
    fn test_filter_display_rules_all_partials() {
        let rules = vec![
            create_partial_rule("partial1"),
            create_partial_description_rule("partial2"),
        ];

        let filtered: Vec<_> = rules
            .into_iter()
            .filter(|rule| !rule.is_partial())
            .collect();

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_display_rules_empty() {
        let rules: Vec<swissarmyhammer_rules::Rule> = vec![];

        let filtered: Vec<_> = rules
            .into_iter()
            .filter(|rule| !rule.is_partial())
            .collect();

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_display_rows_conversion_with_filtering() {
        // Test the complete flow: create rules, filter, then convert to display rows
        let rules = vec![
            create_test_rule("regular1"),
            create_partial_rule("partial1"),
            create_test_rule("regular2"),
        ];

        let filtered: Vec<_> = rules
            .into_iter()
            .filter(|rule| !rule.is_partial())
            .collect();

        // Test standard conversion
        let sources = std::collections::HashMap::new();
        let display_rows = super::super::display::rules_to_display_rows_with_sources(
            filtered.clone(),
            &sources,
            false,
        );
        match display_rows {
            super::super::display::DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].name, "regular1");
                assert_eq!(rows[1].name, "regular2");
            }
            _ => panic!("Expected Standard display rows"),
        }

        // Test verbose conversion
        let display_rows =
            super::super::display::rules_to_display_rows_with_sources(filtered, &sources, true);
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
