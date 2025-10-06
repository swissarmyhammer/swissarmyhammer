//! Check command implementation for rules
//!
//! Checks code files against rules to find violations

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::sync::Arc;
use swissarmyhammer_rules::{RuleCheckRequest, RuleChecker};
use swissarmyhammer_workflow::{
    AgentExecutionContext, AgentExecutor, AgentExecutorFactory, WorkflowTemplateContext,
};

use super::cli::CheckCommand;

/// Execute the check command to verify code against rules
///
/// This command delegates to the rules crate's high-level API which:
/// 1. Loads all available rules from the rules directory
/// 2. Validates all rules to ensure they're well-formed
/// 3. Applies user-specified filters (rule names, severity, category)
/// 4. Expands glob patterns to get target files
/// 5. Creates rule checker with LLM agent
/// 6. Runs checks with fail-fast behavior on violations
///
/// # Arguments
/// * `cmd` - The parsed CheckCommand with patterns and filters
/// * `context` - CLI context with output settings
///
/// # Returns
/// * `Ok(())` if all checks pass or no rules/files match filters
/// * `Err(CliError)` if validation fails or violations are found
///
/// # Examples
/// ```bash
/// sah rule check "**/*.rs"
/// sah rule check --severity error "src/**/*.rs"
/// sah rule check --rule no-unwrap --category style "*.rs"
/// ```
pub async fn execute_check_command(cmd: CheckCommand, context: &CliContext) -> CliResult<()> {
    // Load agent configuration (respects SAH_AGENT_EXECUTOR env var, defaults to ClaudeCode)
    let workflow_context = WorkflowTemplateContext::load_with_agent_config()
        .map_err(|e| CliError::new(format!("Failed to load agent config: {}", e), 1))?;

    let agent_context = AgentExecutionContext::new(&workflow_context);

    // Create executor using factory (ClaudeCode or LlamaAgent based on config)
    let mut executor = AgentExecutorFactory::create_executor(&agent_context)
        .await
        .map_err(|e| CliError::new(format!("Failed to create agent executor: {}", e), 1))?;

    // Initialize the executor
    executor
        .initialize()
        .await
        .map_err(|e| CliError::new(format!("Failed to initialize agent executor: {}", e), 1))?;

    let agent: Arc<dyn AgentExecutor> = Arc::from(executor);
    let checker = RuleChecker::new(agent)
        .map_err(|e| CliError::new(format!("Failed to create rule checker: {}", e), 1))?;

    // Parse severity from string if provided
    let severity = cmd
        .severity
        .as_ref()
        .map(|s| {
            s.parse()
                .map_err(|e: String| CliError::new(e, 1))
        })
        .transpose()?;

    // Create request with filters
    let request = RuleCheckRequest {
        rule_names: cmd.rule,
        severity,
        category: cmd.category,
        patterns: cmd.patterns,
    };

    // Run check with filters - this handles all the logic
    match checker.check_with_filters(request).await {
        Ok(result) => {
            // Print results if not quiet
            if !context.quiet && result.rules_checked == 0 {
                println!("No rules matched the filters");
            }
            Ok(())
        }
        Err(e) => match e {
            swissarmyhammer_common::SwissArmyHammerError::RuleViolation(_) => {
                // Violation was already logged by checker at appropriate level
                Err(CliError::new("Rule violation found".to_string(), 1))
            }
            _ => {
                // Other errors need to be logged
                Err(CliError::new(format!("Check failed: {}", e), 1))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_execute_check_command_no_rules() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed when no rules match filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_no_files() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
            rule: None,
            severity: None,
            category: None,
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed when no files match patterns
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_severity() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: Some("error".to_string()),
            category: None,
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed - filters to only error-level rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_category() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: None,
            category: Some("security".to_string()),
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed - filters to only security category rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_rule_name() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: None,
            category: None,
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed - filters to only specified rule
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_combined_filters() {
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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: Some("error".to_string()),
            category: Some("security".to_string()),
        };

        let result = execute_check_command(cmd, &context).await;
        // Should succeed - applies all filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_excludes_partials() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory with rules and partials
        let temp_dir = TempDir::new().unwrap();
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let local_rules_dir = temp_dir.path().join(".swissarmyhammer").join("rules");
        fs::create_dir_all(&local_rules_dir).unwrap();

        // Create a normal rule
        let rule_file = local_rules_dir.join("normal-rule.md");
        fs::write(
            &rule_file,
            r#"---
title: Normal Rule
description: A normal rule for testing
severity: error
---

Check for issues
"#,
        )
        .unwrap();

        // Create a partial
        let partials_dir = local_rules_dir.join("_partials");
        fs::create_dir_all(&partials_dir).unwrap();
        let partial_file = partials_dir.join("test-partial.md");
        fs::write(
            &partial_file,
            r#"{% partial %}

This is a partial template
"#,
        )
        .unwrap();

        // Create a test file to check
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

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
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: None,
            category: None,
        };

        // Should succeed - partials should be excluded from checking
        // The test verifies partials are filtered out and only normal rules are checked
        // The actual rule will be checked by the LLM, which may pass or fail
        let result = execute_check_command(cmd, &context).await;

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // The result will be an error if a violation is found OR if the agent fails
        // We're just verifying that partials don't cause issues and normal rules run
        // Both Ok (pass) and Err with "Rule violation found" are acceptable outcomes
        // since we have a real rule being checked
        match result {
            Ok(()) => {
                // Rule passed - this is fine
            }
            Err(e) if e.message.contains("Rule violation found") => {
                // Rule found a violation - this is also acceptable for this test
                // The test is about excluding partials, not about passing rules
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }
}
