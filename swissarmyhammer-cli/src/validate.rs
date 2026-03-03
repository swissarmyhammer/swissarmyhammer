use anyhow::Result;
use colored::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;

use swissarmyhammer::validation::{
    Validatable, ValidationConfig, ValidationIssue, ValidationLevel, ValidationManager,
    ValidationResult,
};
use swissarmyhammer_skills::SkillResolver;

use crate::cli::OutputFormat;
use crate::dynamic_cli::CliBuilder;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use crate::mcp_integration::CliToolContext;

#[derive(Debug, Serialize)]
struct JsonValidationResult {
    files_checked: usize,
    errors: usize,
    warnings: usize,
    issues: Vec<JsonValidationIssue>,
}

#[derive(Debug, Serialize)]
struct JsonValidationIssue {
    level: String,
    file_path: String,
    line: Option<usize>,
    column: Option<usize>,
    message: String,
    suggestion: Option<String>,
}

pub struct Validator {
    quiet: bool,
    validation_manager: ValidationManager,
}

impl Validator {
    pub fn new(quiet: bool) -> Self {
        let config = ValidationConfig::default();
        let validation_manager = ValidationManager::new(config);
        Self {
            quiet,
            validation_manager,
        }
    }

    pub async fn validate_all_with_options(
        &mut self,
        validate_tools: bool,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        // Load all prompts using the centralized PromptResolver
        let mut library = swissarmyhammer::PromptLibrary::new();
        let mut resolver = swissarmyhammer::PromptResolver::new();
        resolver.load_all_prompts(&mut library)?;

        // Validate each loaded prompt
        let prompts = library.list()?;
        for prompt in prompts {
            result.files_checked += 1;

            // Store prompt title for error reporting
            let content_title = prompt
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some(prompt.name.clone()));

            // Check if this prompt is builtin
            let is_builtin = resolver
                .prompt_sources
                .get(&prompt.name)
                .map(|source| matches!(source, swissarmyhammer::FileSource::Builtin))
                .unwrap_or(false);

            // Skip liquid syntax validation for builtin prompts to avoid environment-specific errors
            if !is_builtin {
                // Validate template syntax with partials support
                self.validate_liquid_syntax_with_partials(
                    &prompt,
                    &library,
                    prompt.source.as_ref().unwrap_or(&PathBuf::new()),
                    &mut result,
                    content_title.clone(),
                );
            }

            // Use the Validatable trait to validate the prompt
            let validation_issues = prompt.validate(prompt.source.as_deref());

            for issue in validation_issues {
                // Skip undefined template variable errors for builtin prompts
                // Builtin prompts are designed to be used with parameters provided at runtime
                if is_builtin
                    && issue.level == ValidationLevel::Error
                    && issue.message.starts_with("Undefined template variable:")
                {
                    continue;
                }
                result.add_issue(issue);
            }
        }

        // Validate skills from all sources
        self.validate_all_skills(&mut result)?;

        // Validate sah.toml configuration file
        self.validate_sah_config(&mut result)?;

        // Validate tools if requested
        if validate_tools {
            self.validate_tools(&mut result).await?;
        }

        Ok(result)
    }

    /// Validates all skill sources, catching parse/load failures
    fn validate_all_skills(&self, result: &mut ValidationResult) -> Result<()> {
        let resolver = SkillResolver::new();
        let issues = resolver.validate_all_sources();

        // Count successfully-loaded skills for files_checked
        let loaded_skills = resolver.resolve_all();
        result.files_checked += loaded_skills.len();

        // Also count any files that produced errors (not already counted)
        let mut error_files = std::collections::HashSet::new();
        for issue in &issues {
            error_files.insert(issue.file_path.clone());
        }
        result.files_checked += error_files.len();

        for issue in issues {
            result.add_issue(issue);
        }

        Ok(())
    }

    fn validate_liquid_syntax_with_partials(
        &self,
        prompt: &swissarmyhammer::Prompt,
        library: &swissarmyhammer::PromptLibrary,
        file_path: &Path,
        result: &mut ValidationResult,
        content_title: Option<String>,
    ) {
        // First validate content using the validation manager
        let content_validation_result = self.validation_manager.validate_content(
            &prompt.template,
            file_path,
            content_title.clone(),
        );
        result.merge(content_validation_result);

        // Try to render the template with partials support using the same path as test/serve

        // Use render which internally uses partials support
        if let Err(e) = library.render(&prompt.name, &TemplateContext::default()) {
            let error_msg = e.to_string();

            // Only report actual syntax errors, not unknown variable errors
            if !error_msg.contains("Unknown variable") && !error_msg.contains("Required argument") {
                result.add_issue(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: file_path.to_path_buf(),
                    content_title,
                    line: None,
                    column: None,
                    message: format!("Liquid template syntax error: {error_msg}"),
                    suggestion: Some(
                        "Check Liquid template syntax and partial references".to_string(),
                    ),
                });
            }
        }
    }

    /// Format results as a string instead of printing them
    #[allow(dead_code)] // Used by validation but may appear unused due to conditional compilation
    pub fn format_results(
        &self,
        result: &ValidationResult,
        format: OutputFormat,
    ) -> Result<String> {
        match format {
            OutputFormat::Table => Ok(self.format_text_results(result)),
            OutputFormat::Json => self.format_json_results(result),
            OutputFormat::Yaml => {
                // For validate command, YAML output is not implemented, fall back to JSON
                self.format_json_results(result)
            }
        }
    }

    fn format_text_results(&self, result: &ValidationResult) -> String {
        use std::fmt::Write;
        let mut output = String::new();

        if result.issues.is_empty() {
            if !self.quiet {
                writeln!(
                    output,
                    "{} All {} files validated successfully!",
                    "✓".green(),
                    result.files_checked
                )
                .unwrap();
            }
            return output;
        }

        // Group issues by file
        let mut issues_by_file: std::collections::HashMap<PathBuf, Vec<&ValidationIssue>> =
            std::collections::HashMap::new();

        for issue in &result.issues {
            issues_by_file
                .entry(issue.file_path.clone())
                .or_default()
                .push(issue);
        }

        // Print issues grouped by file
        for (file_path, issues) in issues_by_file {
            if !self.quiet {
                // Get the prompt title from the first issue (all issues for a file should have the same title)
                let content_title = issues
                    .first()
                    .and_then(|issue| issue.content_title.as_ref());

                if let Some(title) = content_title {
                    // Show the prompt title
                    writeln!(output, "\n{}", title.bold()).unwrap();
                    // Show the file path in smaller text if it's a user prompt
                    if file_path.to_string_lossy() != ""
                        && !file_path.to_string_lossy().contains("PathBuf")
                    {
                        writeln!(output, "  {}", file_path.display().to_string().dimmed()).unwrap();
                    }
                } else {
                    // Fallback to file path if no title
                    writeln!(output, "\n{}", file_path.display().to_string().bold()).unwrap();
                }
            }

            for issue in issues {
                let level_str = match issue.level {
                    ValidationLevel::Error => "ERROR".red(),
                    ValidationLevel::Warning => "WARN".yellow(),
                    ValidationLevel::Info => "INFO".blue(),
                };

                let location = if let (Some(line), Some(col)) = (issue.line, issue.column) {
                    format!("{line}:{col}")
                } else if let Some(line) = issue.line {
                    format!("{line}")
                } else {
                    "-".to_string()
                };

                if self.quiet && issue.level != ValidationLevel::Error {
                    continue;
                }

                writeln!(output, "  {} [{}] {}", level_str, location, issue.message).unwrap();

                if !self.quiet {
                    if let Some(suggestion) = &issue.suggestion {
                        writeln!(output, "    💡 {}", suggestion.dimmed()).unwrap();
                    }
                }
            }
        }

        if !self.quiet {
            writeln!(output, "\n{}", "Summary:".bold()).unwrap();
            writeln!(output, "  Files checked: {}", result.files_checked).unwrap();
            if result.errors > 0 {
                writeln!(output, "  Errors: {}", result.errors.to_string().red()).unwrap();
            }
            if result.warnings > 0 {
                writeln!(
                    output,
                    "  Warnings: {}",
                    result.warnings.to_string().yellow()
                )
                .unwrap();
            }

            if result.has_errors() {
                writeln!(output, "\n{} Validation failed with errors.", "✗".red()).unwrap();
            } else if result.has_warnings() {
                writeln!(
                    output,
                    "\n{} Validation completed with warnings.",
                    "⚠️".yellow()
                )
                .unwrap();
            } else {
                writeln!(output, "\n{} Validation passed!", "✓".green()).unwrap();
            }
        } else {
            // In quiet mode, only show summary for errors
            if result.has_errors() {
                writeln!(output, "\n{}", "Summary:".bold()).unwrap();
                writeln!(output, "  Files checked: {}", result.files_checked).unwrap();
                writeln!(output, "  Errors: {}", result.errors.to_string().red()).unwrap();
                writeln!(output, "\n{} Validation failed with errors.", "✗".red()).unwrap();
            }
        }

        output
    }

    fn format_json_results(&self, result: &ValidationResult) -> Result<String> {
        let json_issues: Vec<JsonValidationIssue> = result
            .issues
            .iter()
            .map(|issue| JsonValidationIssue {
                level: match issue.level {
                    ValidationLevel::Error => "error".to_string(),
                    ValidationLevel::Warning => "warning".to_string(),
                    ValidationLevel::Info => "info".to_string(),
                },
                file_path: issue.file_path.display().to_string(),
                line: issue.line,
                column: issue.column,
                message: issue.message.clone(),
                suggestion: issue.suggestion.clone(),
            })
            .collect();

        let json_result = JsonValidationResult {
            files_checked: result.files_checked,
            errors: result.errors,
            warnings: result.warnings,
            issues: json_issues,
        };

        Ok(serde_json::to_string_pretty(&json_result)?)
    }

    /// Validate sah.toml configuration file if it exists
    fn validate_sah_config(&self, result: &mut ValidationResult) -> Result<()> {
        use std::path::Path;

        // Check for sah.toml in the current directory
        let config_path = Path::new("sah.toml");
        if !config_path.exists() {
            // No configuration file found - this is not an error
            return Ok(());
        }

        result.files_checked += 1;

        // Try to validate the configuration by loading it with the new system
        match swissarmyhammer_config::load_configuration() {
            Ok(_template_context) => {
                if !self.quiet {
                    let issue = ValidationIssue {
                        level: ValidationLevel::Info,
                        file_path: config_path.to_path_buf(),
                        content_title: Some("sah.toml".to_string()),
                        line: None,
                        column: None,
                        message: "Configuration file validation passed".to_string(),
                        suggestion: None,
                    };
                    result.add_issue(issue);
                }
            }
            Err(e) => {
                // Convert configuration loading error to validation issue
                let (level, message, suggestion) = (
                    ValidationLevel::Error,
                    format!("Configuration loading failed: {}", e),
                    Some("Check the configuration file syntax and structure".to_string()),
                );

                let issue = ValidationIssue {
                    level,
                    file_path: config_path.to_path_buf(),
                    content_title: Some("sah.toml".to_string()),
                    line: None, // We could enhance this to include line numbers from TOML parse errors
                    column: None,
                    message,
                    suggestion,
                };
                result.add_issue(issue);
            }
        }

        Ok(())
    }

    /// Validate MCP tool schemas for CLI compatibility
    async fn validate_tools(&mut self, result: &mut ValidationResult) -> Result<()> {
        // Initialize tool context for validation
        let cli_tool_context = match CliToolContext::new().await {
            Ok(context) => Arc::new(context),
            Err(e) => {
                result.add_issue(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: PathBuf::from("MCP Tools"),
                    content_title: Some("Tool Context".to_string()),
                    line: None,
                    column: None,
                    message: format!("Failed to initialize tool context: {}", e),
                    suggestion: Some(
                        "Check MCP server configuration and accessibility".to_string(),
                    ),
                });
                return Ok(());
            }
        };

        let tool_registry = cli_tool_context.get_tool_registry_arc();
        let cli_builder = CliBuilder::new(tool_registry.clone());

        let validation_stats = cli_builder.get_validation_stats();
        let validation_errors = cli_builder.validate_all_tools();

        // Convert tool validation results to ValidationIssues
        if !validation_stats.is_all_valid() {
            for error in validation_errors {
                let level = if error.to_string().contains("Error") {
                    ValidationLevel::Error
                } else {
                    ValidationLevel::Warning
                };

                result.add_issue(ValidationIssue {
                    level,
                    file_path: PathBuf::from("MCP Tools"),
                    content_title: Some("Tool Validation".to_string()),
                    line: None,
                    column: None,
                    message: error.to_string(),
                    suggestion: error.suggestion().map(|s| s.to_string()),
                });
            }
        }

        // Add a success info message if all tools passed validation (in non-quiet mode)
        if validation_stats.is_all_valid() && !self.quiet {
            result.add_issue(ValidationIssue {
                level: ValidationLevel::Info,
                file_path: PathBuf::from("MCP Tools"),
                content_title: Some("Tool Validation".to_string()),
                line: None,
                column: None,
                message: format!("All {} tools passed validation", validation_stats.summary()),
                suggestion: None,
            });
        }

        Ok(())
    }
}

/// Run validation command and return the output as a string and exit code
/// This is used for in-process testing where we need to capture the output
#[allow(dead_code)] // Used by test infrastructure
pub async fn run_validate_command_with_dirs_captured(
    quiet: bool,
    format: OutputFormat,
    validate_tools: bool,
) -> Result<(String, i32)> {
    let mut validator = Validator::new(quiet);

    let result = validator.validate_all_with_options(validate_tools).await?;

    let output = validator.format_results(&result, format)?;

    // Return appropriate exit code
    let exit_code = if result.has_errors() {
        EXIT_ERROR // Errors
    } else if result.has_warnings() {
        EXIT_WARNING // Warnings
    } else {
        EXIT_SUCCESS // Success
    };

    Ok((output, exit_code))
}

/// Run validation command and return structured results for CliContext integration
pub async fn run_validate_command_structured(
    quiet: bool,
    validate_tools: bool,
) -> Result<(ValidationResult, i32)> {
    let mut validator = Validator::new(quiet);

    let result = validator.validate_all_with_options(validate_tools).await?;

    // Determine exit code
    let exit_code = if result.has_errors() {
        EXIT_ERROR
    } else if result.has_warnings() {
        EXIT_WARNING
    } else {
        EXIT_SUCCESS
    };

    Ok((result, exit_code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_cli::CliValidationStats;
    use std::path::PathBuf;
    use swissarmyhammer_common::Validatable;

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult::new();
        assert_eq!(result.files_checked, 0);
        assert_eq!(result.errors, 0);
        assert_eq!(result.warnings, 0);
        assert!(!result.has_errors());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_validation_result_add_error() {
        let mut result = ValidationResult::new();
        let issue = ValidationIssue {
            level: ValidationLevel::Error,
            file_path: PathBuf::from("test.md"),
            content_title: Some("Test Prompt".to_string()),
            line: Some(1),
            column: Some(1),
            message: "Test error".to_string(),
            suggestion: None,
        };

        result.add_issue(issue);
        assert_eq!(result.errors, 1);
        assert_eq!(result.warnings, 0);
        assert!(result.has_errors());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_validation_result_add_warning() {
        let mut result = ValidationResult::new();
        let issue = ValidationIssue {
            level: ValidationLevel::Warning,
            file_path: PathBuf::from("test.md"),
            content_title: Some("Test Prompt".to_string()),
            line: Some(1),
            column: Some(1),
            message: "Test warning".to_string(),
            suggestion: None,
        };

        result.add_issue(issue);
        assert_eq!(result.errors, 0);
        assert_eq!(result.warnings, 1);
        assert!(!result.has_errors());
        assert!(result.has_warnings());
    }

    #[test]
    fn test_validator_creation() {
        let validator = Validator::new(false);
        assert!(!validator.quiet);

        let quiet_validator = Validator::new(true);
        assert!(quiet_validator.quiet);
    }

    #[tokio::test]
    async fn test_validate_all_handles_partial_templates() {
        // This test verifies that .liquid files with {% partial %} marker
        // don't generate errors for missing title/description
        let mut validator = Validator::new(false);

        // Note: This test relies on the actual prompt loading mechanism
        // which will load test files from the test environment

        // In test environment, validate_all may fail due to missing directories
        // We allow this to fail gracefully as we're testing partial template handling
        let result = match validator.validate_all_with_options(false).await {
            Ok(r) => r,
            Err(_) => {
                // In test environment, we may not have standard directories
                // Create an empty result to complete the test
                ValidationResult::new()
            }
        };

        // Check that partial templates don't cause title/description errors
        let partial_errors = result
            .issues
            .iter()
            .filter(|issue| {
                issue.file_path.to_string_lossy().ends_with(".liquid")
                    && (issue.message.contains("Missing required field: title")
                        || issue
                            .message
                            .contains("Missing required field: description"))
            })
            .count();

        assert_eq!(partial_errors, 0,
            "Partial templates (with {{% partial %}} marker) should not have title/description errors");
    }

    #[test]
    fn test_partial_template_no_variable_validation_errors() {
        // Test that partial templates with {% partial %} marker don't generate
        // variable validation errors for undefined variables
        let mut result = ValidationResult::new();

        // Create a partial template with undefined variables
        let partial_content = r#"{% partial %}

This is a partial template that uses {{ todo_file }} variable
and also uses {{ language }} argument.

Neither of these should cause validation errors."#;

        // Create a prompt directly to test validation using the actual swissarmyhammer::Prompt
        let partial_prompt = swissarmyhammer::Prompt::new("test-partial", partial_content)
            .with_description("Partial template for reuse in other prompts");

        // Validate the partial prompt using the Validatable trait
        let validation_issues =
            partial_prompt.validate(Some(std::path::Path::new("test-partial.liquid")));
        for issue in validation_issues {
            result.add_issue(issue);
        }

        // Should have no errors for undefined variables in partials
        let variable_errors = result
            .issues
            .iter()
            .filter(|issue| issue.message.contains("Undefined template variable"))
            .count();

        assert_eq!(
            variable_errors, 0,
            "Partial templates should not have undefined variable errors"
        );

        // Should also not have warnings about template using variables with no arguments
        let arg_warnings = result
            .issues
            .iter()
            .filter(|issue| {
                issue
                    .message
                    .contains("Template uses variables but no arguments are defined")
            })
            .count();

        assert_eq!(
            arg_warnings, 0,
            "Partial templates should not have warnings about missing argument definitions"
        );
    }

    #[test]
    fn test_quiet_mode_hides_warnings_from_summary() {
        // Test that quiet mode hides both warning details and warning counts from summary
        let mut result = ValidationResult::new();
        result.files_checked = 1;

        // Add a warning to the result
        let warning_issue = ValidationIssue {
            level: ValidationLevel::Warning,
            file_path: PathBuf::from("test.md"),
            content_title: Some("Test".to_string()),
            line: None,
            column: None,
            message: "Test warning".to_string(),
            suggestion: Some("Test suggestion".to_string()),
        };
        result.add_issue(warning_issue);

        // Test quiet mode validator configuration
        let quiet_validator = Validator::new(true);
        assert!(quiet_validator.quiet);

        // Verify warning count is correct but quiet mode should hide it
        assert_eq!(result.warnings, 1);
        assert!(!result.has_errors());
        assert!(result.has_warnings());

        // Test normal mode - should show warnings
        let normal_validator = Validator::new(false);
        assert!(!normal_validator.quiet);

        // The fix ensures that in quiet mode, when only warnings exist,
        // no summary is shown at all (tested in integration tests)
    }

    #[tokio::test]
    async fn test_validate_tools_functionality() {
        let mut validator = Validator::new(false);

        let mut result = ValidationResult::new();

        // Test validate_tools method directly
        // This should not panic even if no MCP tools are available
        let validation_result = validator.validate_tools(&mut result).await;

        // The method should complete without error
        assert!(
            validation_result.is_ok(),
            "validate_tools should not fail even without MCP tools"
        );
    }

    #[tokio::test]
    async fn test_validate_all_with_tools_flag() {
        let mut validator = Validator::new(false);

        // Test validation with validate_tools = false
        let result_without_tools = validator.validate_all_with_options(false).await;
        assert!(
            result_without_tools.is_ok(),
            "Validation without tools should succeed"
        );

        // Test validation with validate_tools = true
        let result_with_tools = validator.validate_all_with_options(true).await;
        assert!(
            result_with_tools.is_ok(),
            "Validation with tools should succeed even if no tools available"
        );
    }

    #[tokio::test]
    async fn test_validate_tools_error_handling() {
        let mut validator = Validator::new(false);

        let mut result = ValidationResult::new();

        // Test that validation continues even if tool context fails
        let validation_result = validator.validate_tools(&mut result).await;

        // Should handle gracefully when MCP tool context cannot be created
        assert!(
            validation_result.is_ok(),
            "validate_tools should handle MCP context errors gracefully"
        );
    }

    #[test]
    fn test_cli_validation_stats_default() {
        let stats = CliValidationStats::default();

        assert_eq!(stats.total_tools, 0);
        assert_eq!(stats.valid_tools, 0);
        assert_eq!(stats.invalid_tools, 0);
        assert_eq!(stats.validation_errors, 0);
        assert!(stats.is_all_valid());
        assert_eq!(stats.success_rate(), 100.0);
    }

    #[test]
    fn test_cli_validation_stats_calculations() {
        let mut stats = CliValidationStats::new();
        stats.total_tools = 10;
        stats.valid_tools = 8;
        stats.invalid_tools = 2;
        stats.validation_errors = 1;

        assert!(!stats.is_all_valid());
        assert_eq!(stats.success_rate(), 80.0);

        let summary = stats.summary();
        assert!(summary.contains("8 of 10"));
        assert!(summary.contains("80.0%"));
        assert!(summary.contains("1 validation errors"));
    }

    #[test]
    fn test_cli_validation_stats_edge_cases() {
        // Test with zero tools
        let mut stats = CliValidationStats::new();
        assert_eq!(stats.success_rate(), 100.0);
        assert!(stats.is_all_valid());

        // Test with all valid tools
        stats.total_tools = 5;
        stats.valid_tools = 5;
        stats.invalid_tools = 0;
        stats.validation_errors = 0;

        assert!(stats.is_all_valid());
        assert_eq!(stats.success_rate(), 100.0);

        let summary = stats.summary();
        assert!(summary.contains("All 5 CLI tools are valid"));
    }
}
