//! Validate command implementation for rules
//!
//! Validates rule files for correct syntax and structure

use crate::context::CliContext;
use crate::error::CliResult;

use super::cli::ValidateCommand;

/// Execute the validate command to check rule syntax
pub async fn execute_validate_command(cmd: ValidateCommand, context: &CliContext) -> CliResult<()> {
    use swissarmyhammer_rules::{Rule, RuleResolver};

    // Load all rules from all sources
    let mut resolver = RuleResolver::new();
    let mut all_rules = Vec::new();
    resolver.load_all_rules(&mut all_rules)?;

    // Filter out partials - they are not standalone rules
    all_rules.retain(|r| !r.is_partial());

    // Filter rules if specific rule name or file requested
    let rules_to_validate: Vec<&Rule> = if let Some(ref rule_name) = cmd.rule_name {
        all_rules.iter().filter(|r| r.name == *rule_name).collect()
    } else if let Some(ref file_path) = cmd.file {
        all_rules
            .iter()
            .filter(|r| {
                r.source
                    .as_ref()
                    .map(|p| p.to_string_lossy().contains(file_path))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        all_rules.iter().collect()
    };

    if rules_to_validate.is_empty() {
        return if let Some(ref rule_name) = cmd.rule_name {
            Err(crate::error::CliError::new(
                format!("Rule '{}' not found", rule_name),
                1,
            ))
        } else if let Some(ref file_path) = cmd.file {
            Err(crate::error::CliError::new(
                format!("No rules found in file '{}'", file_path),
                1,
            ))
        } else {
            // No filters and empty list - this shouldn't happen but handle gracefully
            Ok(())
        };
    }

    // Validate each rule and collect errors
    let mut valid_count = 0;
    let mut invalid_rules = Vec::new();

    for rule in rules_to_validate {
        match rule.validate() {
            Ok(()) => {
                valid_count += 1;
            }
            Err(e) => {
                let source = resolver
                    .rule_sources
                    .get(&rule.name)
                    .map(|s| s.display_emoji())
                    .unwrap_or("Unknown");

                invalid_rules.push((rule.name.clone(), source, rule.source.clone(), e));
            }
        }
    }

    // Display results
    if !context.quiet {
        if valid_count > 0 && invalid_rules.is_empty() {
            println!("✓ All rules valid");
        }

        if !invalid_rules.is_empty() {
            println!("✗ {} invalid rule(s):\n", invalid_rules.len());

            for (name, source, file_path, error) in &invalid_rules {
                println!("  Rule: {}", name);
                println!("  Source: {}", source);
                if let Some(path) = file_path {
                    println!("  File: {}", path.display());
                }
                println!("  Error: {}", error);
                println!();
            }
        }
    }

    // Return error if any invalid rules
    if !invalid_rules.is_empty() {
        return Err(crate::error::CliError::new(
            format!("{} invalid rule(s) found", invalid_rules.len()),
            1,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    #[tokio::test]
    async fn test_validate_all_rules() {
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
            .quiet(true) // Quiet mode to avoid output in tests
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let cmd = ValidateCommand {
            rule_name: None,
            file: None,
        };

        // Should succeed if all builtin rules are valid
        let result = execute_validate_command(cmd, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_specific_nonexistent_rule() {
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

        let cmd = ValidateCommand {
            rule_name: Some("nonexistent-rule-xyz".to_string()),
            file: None,
        };

        // Should return an error when rule not found
        let result = execute_validate_command(cmd, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_with_file_filter() {
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

        let cmd = ValidateCommand {
            rule_name: None,
            file: Some("nonexistent-file.md".to_string()),
        };

        // Should return error when no rules match the file filter
        let result = execute_validate_command(cmd, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_specific_valid_rule() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory with a test rule
        let temp_dir = TempDir::new().unwrap();
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let local_rules_dir = temp_dir.path().join(".swissarmyhammer").join("rules");
        fs::create_dir_all(&local_rules_dir).unwrap();

        let rule_file = local_rules_dir.join("valid-test-rule.md");
        fs::write(
            &rule_file,
            r#"---
title: Valid Test Rule
description: A valid rule for testing validation
severity: error
---

Check for test issues
"#,
        )
        .unwrap();

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

        let cmd = ValidateCommand {
            rule_name: Some("valid-test-rule".to_string()),
            file: None,
        };

        // Should succeed when validating a specific valid rule
        let result = execute_validate_command(cmd, &context).await;

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_excludes_partials() {
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

        let cmd = ValidateCommand {
            rule_name: None,
            file: None,
        };

        // Should succeed and only validate the normal rule, not the partial
        let result = execute_validate_command(cmd, &context).await;

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
    }
}
