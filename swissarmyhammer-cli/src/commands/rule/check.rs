//! Check command implementation for rules
//!
//! Checks code files against rules to find violations

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_common::glob_utils::{
    expand_glob_patterns as common_expand_glob_patterns, GlobExpansionConfig,
};
use swissarmyhammer_rules::{RuleChecker, RuleResolver, Severity};
use swissarmyhammer_workflow::{
    AgentExecutionContext, AgentExecutor, AgentExecutorFactory, WorkflowTemplateContext,
};

use super::cli::CheckCommand;

/// Expand glob patterns to file paths with gitignore support
fn expand_glob_patterns(patterns: &[String]) -> CliResult<Vec<PathBuf>> {
    let config = GlobExpansionConfig::default();
    common_expand_glob_patterns(patterns, &config)
        .map_err(|e| CliError::new(format!("Failed to expand glob patterns: {}", e), 1))
}

/// Execute the check command to verify code against rules
///
/// This command goes through multiple phases:
/// 1. Load all available rules from the rules directory
/// 2. Validate all rules to ensure they're well-formed
/// 3. Apply user-specified filters (rule names, severity, category)
/// 4. Expand glob patterns to get target files
/// 5. Create rule checker with LLM agent
/// 6. Run checks with fail-fast behavior on violations
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
    // Phase 1: Load all rules via RuleResolver
    let mut rules = Vec::new();
    let mut resolver = RuleResolver::new();
    resolver
        .load_all_rules(&mut rules)
        .map_err(|e| CliError::new(format!("Failed to load rules: {}", e), 1))?;

    // Filter out partials - they are not standalone rules
    rules.retain(|r| !r.is_partial());

    // Phase 2: Validate all rules first (fail if any invalid)

    for rule in &rules {
        rule.validate().map_err(|e| {
            CliError::new(
                format!("Rule validation failed for '{}': {}", rule.name, e),
                1,
            )
        })?;
    }

    // Phase 3: Apply filters
    if let Some(rule_names) = &cmd.rule {
        rules.retain(|r| rule_names.contains(&r.name));
    }

    if let Some(severity_str) = &cmd.severity {
        let severity: Severity = severity_str
            .parse()
            .map_err(|e: String| CliError::new(e, 1))?;
        rules.retain(|r| r.severity == severity);
    }

    if let Some(category) = &cmd.category {
        rules.retain(|r| r.category.as_ref() == Some(category));
    }

    if rules.is_empty() {
        if !context.quiet {
            println!("No rules matched the filters");
        }
        return Ok(());
    }

    // Phase 4: Get target files from patterns
    let target_files = expand_glob_patterns(&cmd.patterns)?;

    if target_files.is_empty() {
        return Ok(());
    }

    // Phase 5: Create RuleChecker with agent from configuration
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

    // Phase 6: Run check_all with fail-fast behavior
    match checker.check_all(rules, target_files).await {
        Ok(()) => Ok(()),
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
    use std::fs;
    use swissarmyhammer_config::TemplateContext;
    use tempfile::TempDir;

    #[test]
    fn test_expand_glob_patterns_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let patterns = vec![file_path.to_string_lossy().to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], file_path);
    }

    #[test]
    fn test_expand_glob_patterns_directory() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "fn test() {}").unwrap();

        let patterns = vec![temp_dir.path().to_string_lossy().to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_wildcard() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "fn test() {}").unwrap();
        fs::write(temp_dir.path().join("file3.txt"), "text").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.extension().unwrap() == "rs"));
    }

    #[test]
    fn test_expand_glob_patterns_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("src");
        fs::create_dir(&subdir).unwrap();
        fs::write(temp_dir.path().join("root.rs"), "fn main() {}").unwrap();
        fs::write(subdir.join("lib.rs"), "fn test() {}").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["**/*.rs".to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_multiple_patterns() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "text").unwrap();

        // Change to temp directory and use relative patterns
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string(), "*.txt".to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_respects_gitignore() {
        use std::process::Command;

        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo for gitignore to work
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        fs::write(temp_dir.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(temp_dir.path().join("included.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("ignored.rs"), "fn test() {}").unwrap();

        // Use directory pattern which triggers WalkBuilder with gitignore support
        let patterns = vec![temp_dir.path().to_string_lossy().to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        // Check that ignored.rs is not in results and included.rs is
        assert!(result.iter().any(|p| p.ends_with("included.rs")));
        assert!(!result.iter().any(|p| p.ends_with("ignored.rs")));
    }

    #[test]
    fn test_expand_glob_patterns_empty_on_no_match() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "text").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string()];
        let result = expand_glob_patterns(&patterns).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 0);
    }

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
