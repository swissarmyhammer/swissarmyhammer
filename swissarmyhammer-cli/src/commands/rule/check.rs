//! Check command implementation for rules
//!
//! Checks code files against rules to find violations

use crate::context::CliContext;
use crate::error::CliResult;

use super::cli::CheckCommand;

/// Execute the check command to verify code against rules
pub async fn execute_check_command(_cmd: CheckCommand, _context: &CliContext) -> CliResult<()> {
    // TODO: Implement rule checking logic
    // This will check code files against specified rules or all applicable rules
    println!("Rule checking not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_execute_check_command() {
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

        let cmd = super::super::cli::CheckCommand {
            rule_name: None,
            files: vec!["test.rs".to_string()],
            fix: false,
        };

        let result = execute_check_command(cmd, &context).await;
        assert!(result.is_ok());
    }
}
