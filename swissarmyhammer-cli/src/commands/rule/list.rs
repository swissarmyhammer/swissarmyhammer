//! List command implementation for rules
//!
//! Lists all available rules with their metadata

use crate::context::CliContext;
use anyhow::Result;
use std::collections::HashMap;

/// Execute the list command to display available rules
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Load rules from the library
    let library = swissarmyhammer_rules::RuleLibrary::new();
    let rules = library.list()?;

    // TODO: Get source information from a RuleResolver when implemented
    let sources = HashMap::new();

    // Convert to display rows using the context's verbose flag
    let display_rows =
        super::display::rules_to_display_rows_with_sources(rules, &sources, cli_context.verbose);

    // Use context's display_rules method
    cli_context.display_rules(display_rows)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_execute_list_command() {
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

        let result = execute_list_command(&context).await;
        assert!(result.is_ok());
    }
}
