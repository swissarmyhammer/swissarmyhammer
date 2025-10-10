//! Check command implementation for rules
//!
//! Checks code files against rules to find violations and optionally creates
//! issues for ERROR level violations when --create-issues flag is provided

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use swissarmyhammer_agent_executor::AgentExecutor;
use swissarmyhammer_config::agent::{AgentConfig, AgentExecutorConfig};
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_rules::{RuleCheckRequest, RuleChecker, RuleViolation};

use super::cli::CheckCommand;

/// Request structure for check command execution
///
/// Combines the command parameters with optional agent configuration
/// for testability and cleaner function signatures.
struct CheckCommandRequest {
    cmd: CheckCommand,
    agent_config: Option<AgentConfig>,
}

impl CheckCommandRequest {
    /// Create a new request with default agent configuration
    fn new(cmd: CheckCommand) -> Self {
        Self {
            cmd,
            agent_config: None,
        }
    }

    /// Create a new request with explicit agent configuration (for testing)
    #[cfg(test)]
    fn with_config(cmd: CheckCommand, agent_config: AgentConfig) -> Self {
        Self {
            cmd,
            agent_config: Some(agent_config),
        }
    }
}

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
    let request = CheckCommandRequest::new(cmd);
    execute_check_command_impl(request, context).await
}

/// Internal implementation of check command with injectable agent configuration
///
/// This function is identical to `execute_check_command` but accepts a request
/// structure that can include agent configuration for testing purposes. In production
/// use, the configuration is loaded from the environment. In tests, a test
/// configuration can be provided to avoid expensive executor initialization.
///
/// # Arguments
/// * `request` - CheckCommandRequest containing command and optional agent config
/// * `context` - CLI context with output settings
///
/// # Returns
/// * `Ok(())` if all checks pass or no rules/files match filters
/// * `Err(CliError)` if validation fails or violations are found
async fn execute_check_command_impl(
    request: CheckCommandRequest,
    context: &CliContext,
) -> CliResult<()> {
    // Load agent configuration (respects SAH_AGENT_EXECUTOR env var, defaults to ClaudeCode)
    // For tests, use provided config (LlamaAgent), otherwise use default
    let agent_config = request.agent_config.unwrap_or_default();

    // Create and initialize executor based on type
    let executor: Box<dyn AgentExecutor> = match &agent_config.executor {
        AgentExecutorConfig::LlamaAgent(llama_config) => {
            // Start MCP server for LlamaAgent
            use swissarmyhammer_agent_executor::llama::{
                LlamaAgentExecutorWrapper, McpServerHandle,
            };
            use swissarmyhammer_prompts::PromptLibrary;
            use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

            tracing::info!("Starting MCP server for LlamaAgent");
            let tools_mcp_handle = start_mcp_server(
                McpServerMode::Http {
                    port: if llama_config.mcp_server.port == 0 {
                        None
                    } else {
                        Some(llama_config.mcp_server.port)
                    },
                },
                Some(PromptLibrary::default()),
            )
            .await
            .map_err(|e| CliError::new(format!("Failed to start MCP server: {}", e), 1))?;

            tracing::info!(
                "MCP server started on port {:?}",
                tools_mcp_handle.info.port
            );

            // Convert tools McpServerHandle to agent-executor McpServerHandle
            // The two types are structurally identical but from different crates.
            // We create a dummy shutdown channel because the tools MCP handle manages
            // the server lifecycle, and we only need the port/host info for the agent.
            let port = tools_mcp_handle.info.port.unwrap_or(0);
            let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
            let agent_mcp_handle = McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx);

            let mut exec = LlamaAgentExecutorWrapper::new_with_mcp(
                llama_config.clone(),
                Some(agent_mcp_handle),
            );
            exec.initialize().await.map_err(|e| {
                CliError::new(
                    format!("Failed to initialize LlamaAgent executor: {}", e),
                    1,
                )
            })?;
            Box::new(exec)
        }
        AgentExecutorConfig::ClaudeCode(_claude_config) => {
            use swissarmyhammer_agent_executor::ClaudeCodeExecutor;

            tracing::info!("Using ClaudeCode executor for rule checking");
            let mut exec = ClaudeCodeExecutor::new();
            exec.initialize().await.map_err(|e| {
                CliError::new(
                    format!("Failed to initialize ClaudeCode executor: {}", e),
                    1,
                )
            })?;
            Box::new(exec)
        }
    };

    let agent: Arc<dyn AgentExecutor> = Arc::from(executor);
    let checker = RuleChecker::new(agent)
        .map_err(|e| CliError::new(format!("Failed to create rule checker: {}", e), 1))?;

    // Parse severity from string if provided
    let severity = request
        .cmd
        .severity
        .as_ref()
        .map(|s| s.parse().map_err(|e: String| CliError::new(e, 1)))
        .transpose()?;

    // Create rule check request with filters
    let rule_request = RuleCheckRequest {
        rule_names: request.cmd.rule,
        severity,
        category: request.cmd.category,
        patterns: request.cmd.patterns,
    };

    // Choose behavior based on create_issues flag
    if request.cmd.create_issues {
        // Use collection mode to gather all ERROR violations
        match checker.check_with_filters_collect(rule_request).await {
            Ok(result) => {
                // Print results if not quiet
                if !context.quiet {
                    if result.rules_checked == 0 {
                        println!("No rules matched the filters");
                    } else if result.violations.is_empty() {
                        println!("All checks passed - no ERROR violations found");
                    } else {
                        println!(
                            "Found {} ERROR violation(s), creating issues...",
                            result.violations.len()
                        );
                    }
                }

                // Create issues for violations
                if !result.violations.is_empty() {
                    create_issues_for_violations(&result.violations, context).await?;
                }

                Ok(())
            }
            Err(e) => {
                // Other errors need to be logged
                Err(CliError::new(format!("Check failed: {}", e), 1))
            }
        }
    } else {
        // Use fail-fast mode (original behavior)
        match checker.check_with_filters(rule_request).await {
            Ok(result) => {
                // Print results if not quiet
                if !context.quiet && result.rules_checked == 0 {
                    println!("No rules matched the filters");
                }
                Ok(())
            }
            Err(e) => match e {
                swissarmyhammer_common::SwissArmyHammerError::RuleViolation(violation_msg) => {
                    // Violation was already logged by checker, pass through the message
                    Err(CliError::new(
                        format!("Rule violation: {}", violation_msg),
                        1,
                    ))
                }
                _ => {
                    // Other errors need to be logged
                    Err(CliError::new(format!("Check failed: {}", e), 1))
                }
            },
        }
    }
}

/// Number of characters to use from the file path hash in issue names
const ISSUE_HASH_LENGTH: usize = 8;

/// Generate a deterministic hash for a file path
///
/// Uses SHA-256 to hash the file path and returns the first 8 characters
/// of the hex digest for use in issue filenames.
fn generate_file_hash(file_path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.to_string_lossy().as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..ISSUE_HASH_LENGTH].to_string()
}

/// Generate issue name from rule name and file path
///
/// Creates a name in the format: ~{rule_name}_{file_hash}
/// The ~ prefix sorts issues to the end of the list.
/// Replaces slashes in rule names with underscores for filesystem safety.
fn generate_issue_name(rule_name: &str, file_path: &Path) -> String {
    let file_hash = generate_file_hash(file_path);
    let safe_rule_name = rule_name.replace('/', "_");
    format!("~{}_{}", safe_rule_name, file_hash)
}

/// Format issue content from a rule violation
///
/// Creates a markdown-formatted issue body with violation details.
fn format_issue_content(violation: &RuleViolation) -> String {
    format!(
        r#"# Rule Violation: {}

**File**: {}
**Severity**: ERROR

## Violation Message

{}

---
*This issue was automatically created by `sah rule check --create-issues`*
"#,
        violation.rule_name,
        violation.file_path.display(),
        violation.message
    )
}

/// Create issues for all rule violations
///
/// Iterates through violations and creates an issue for each unique rule+file combination.
/// Skips creating issues that already exist.
async fn create_issues_for_violations(
    violations: &[RuleViolation],
    context: &CliContext,
) -> CliResult<()> {
    let storage = FileSystemIssueStorage::new_default()
        .map_err(|e| CliError::new(format!("Failed to initialize issue storage: {}", e), 1))?;

    let mut created_count = 0;
    let mut skipped_count = 0;

    for violation in violations {
        let issue_name = generate_issue_name(&violation.rule_name, &violation.file_path);
        let issue_content = format_issue_content(violation);

        // Check if issue already exists
        match storage.get_issue(&issue_name).await {
            Ok(_existing) => {
                tracing::debug!("Issue '{}' already exists, skipping", issue_name);
                skipped_count += 1;
            }
            Err(_) => {
                // Issue doesn't exist, create it
                match storage
                    .create_issue(issue_name.clone(), issue_content)
                    .await
                {
                    Ok(_) => {
                        if !context.quiet {
                            println!("Created issue: {}", issue_name);
                        }
                        created_count += 1;
                    }
                    Err(e) => {
                        // Log warning but continue processing other violations
                        tracing::warn!("Failed to create issue '{}': {}", issue_name, e);
                    }
                }
            }
        }
    }

    if !context.quiet {
        println!(
            "\nIssue creation summary: {} created, {} skipped (already exist)",
            created_count, skipped_count
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::{LlamaAgentConfig, TemplateContext};

    /// Helper function to create a test CLI context with standard settings
    async fn setup_test_context() -> CliContext {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap()
    }

    /// Helper function to create a test agent configuration
    fn setup_test_agent_config() -> AgentConfig {
        AgentConfig::llama_agent(LlamaAgentConfig::for_testing())
    }

    #[tokio::test]
    async fn test_execute_check_command_no_rules() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed when no rules match filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_no_files() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["/nonexistent/**/*.rs".to_string()],
            rule: None,
            severity: None,
            category: None,
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed when no files match patterns
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_severity() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: Some("error".to_string()),
            category: None,
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only error-level rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_category() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: None,
            severity: None,
            category: Some("security".to_string()),
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only security category rules
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_filter_by_rule_name() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: None,
            category: None,
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - filters to only specified rule
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_combined_filters() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["specific-rule".to_string()]),
            severity: Some("error".to_string()),
            category: Some("security".to_string()),
            create_issues: false,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - applies all filters
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_check_command_with_claude_code() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
            create_issues: false,
        };

        // Request ClaudeCode - it should work now without fallback
        let request = CheckCommandRequest::with_config(cmd, AgentConfig::claude_code());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed - ClaudeCode is fully supported
        assert!(
            result.is_ok(),
            "ClaudeCode should work for rule checking without fallback"
        );
    }

    #[test]
    fn test_generate_file_hash() {
        let path1 = Path::new("src/main.rs");
        let path2 = Path::new("src/lib.rs");
        let path1_again = Path::new("src/main.rs");

        let hash1 = generate_file_hash(path1);
        let hash2 = generate_file_hash(path2);
        let hash1_again = generate_file_hash(path1_again);

        // Hash should be ISSUE_HASH_LENGTH characters
        assert_eq!(hash1.len(), ISSUE_HASH_LENGTH);
        assert_eq!(hash2.len(), ISSUE_HASH_LENGTH);

        // Same path should produce same hash
        assert_eq!(hash1, hash1_again);

        // Different paths should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_generate_issue_name() {
        let rule_name = "no-unwrap";
        let file_path = Path::new("src/main.rs");

        let issue_name = generate_issue_name(rule_name, file_path);

        // Should start with ~
        assert!(issue_name.starts_with('~'));

        // Should contain rule name
        assert!(issue_name.contains("no-unwrap"));

        // Should contain underscore separator
        assert!(issue_name.contains('_'));

        // Should be deterministic
        let issue_name_again = generate_issue_name(rule_name, file_path);
        assert_eq!(issue_name, issue_name_again);
    }

    #[test]
    fn test_generate_issue_name_with_slashes() {
        let rule_name = "security/no-hardcoded-secrets";
        let file_path = Path::new("test_violation.rs");

        let issue_name = generate_issue_name(rule_name, file_path);

        // Should start with ~
        assert!(issue_name.starts_with('~'));

        // Should replace slashes with underscores
        assert!(issue_name.contains("security_no-hardcoded-secrets"));
        assert!(!issue_name.contains('/'));

        // Should contain hash
        assert!(issue_name.contains('_'));

        // Should be deterministic
        let issue_name_again = generate_issue_name(rule_name, file_path);
        assert_eq!(issue_name, issue_name_again);
    }

    #[test]
    fn test_generate_issue_name_with_multiple_slashes() {
        let rule_name = "category/subcategory/rule-name";
        let file_path = Path::new("src/main.rs");

        let issue_name = generate_issue_name(rule_name, file_path);

        // Should replace all slashes with underscores
        assert!(issue_name.contains("category_subcategory_rule-name"));
        assert!(!issue_name.contains('/'));

        // Should start with ~
        assert!(issue_name.starts_with('~'));

        // Should be deterministic
        let issue_name_again = generate_issue_name(rule_name, file_path);
        assert_eq!(issue_name, issue_name_again);
    }

    #[test]
    fn test_generate_issue_name_with_consecutive_slashes() {
        let rule_name = "security//no-secrets";
        let file_path = Path::new("test.rs");

        let issue_name = generate_issue_name(rule_name, file_path);

        // Should replace consecutive slashes with underscores
        assert!(issue_name.contains("security__no-secrets"));
        assert!(!issue_name.contains('/'));

        // Should start with ~
        assert!(issue_name.starts_with('~'));
    }

    #[test]
    fn test_generate_issue_name_with_leading_trailing_slashes() {
        let rule_name = "/security/no-secrets/";
        let file_path = Path::new("test.rs");

        let issue_name = generate_issue_name(rule_name, file_path);

        // Should replace all slashes including leading/trailing
        assert!(issue_name.contains("_security_no-secrets_"));
        assert!(!issue_name.contains('/'));

        // Should start with ~
        assert!(issue_name.starts_with('~'));
    }

    #[test]
    fn test_format_issue_content() {
        use std::path::PathBuf;
        use swissarmyhammer_rules::{RuleViolation, Severity};

        let violation = RuleViolation::new(
            "no-unwrap".to_string(),
            PathBuf::from("src/main.rs"),
            Severity::Error,
            "Found unwrap() call which can panic".to_string(),
        );

        let content = format_issue_content(&violation);

        // Should contain rule name
        assert!(content.contains("no-unwrap"));

        // Should contain file path
        assert!(content.contains("src/main.rs"));

        // Should contain severity
        assert!(content.contains("ERROR"));

        // Should contain violation message
        assert!(content.contains("Found unwrap() call which can panic"));

        // Should be markdown formatted
        assert!(content.contains("# Rule Violation"));

        // Should contain footer
        assert!(content.contains("automatically created"));
    }

    #[test]
    fn test_format_issue_content_with_markdown_chars() {
        use std::path::PathBuf;
        use swissarmyhammer_rules::{RuleViolation, Severity};

        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Found *special* **markdown** chars: # ## ### `code` [link](url)".to_string(),
        );

        let content = format_issue_content(&violation);

        // Should preserve markdown special characters in message
        assert!(content.contains("Found *special* **markdown** chars"));
        assert!(content.contains("`code`"));
        assert!(content.contains("[link](url)"));

        // Should still be properly structured markdown
        assert!(content.contains("# Rule Violation"));
    }

    #[test]
    fn test_format_issue_content_with_newlines() {
        use std::path::PathBuf;
        use swissarmyhammer_rules::{RuleViolation, Severity};

        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Multi-line message:\nLine 1\nLine 2\nLine 3".to_string(),
        );

        let content = format_issue_content(&violation);

        // Should preserve newlines in message
        assert!(content.contains("Multi-line message:\nLine 1\nLine 2\nLine 3"));

        // Should still be properly structured
        assert!(content.contains("# Rule Violation"));
        assert!(content.contains("## Violation Message"));
    }

    #[test]
    fn test_format_issue_content_with_code_block() {
        use std::path::PathBuf;
        use swissarmyhammer_rules::{RuleViolation, Severity};

        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Found issue in code:\n```rust\nfn main() {\n    println!(\"test\");\n}\n```"
                .to_string(),
        );

        let content = format_issue_content(&violation);

        // Should preserve code block formatting
        assert!(content.contains("```rust"));
        assert!(content.contains("fn main()"));

        // Should still be properly structured
        assert!(content.contains("# Rule Violation"));
    }

    #[test]
    fn test_format_issue_content_with_very_long_message() {
        use std::path::PathBuf;
        use swissarmyhammer_rules::{RuleViolation, Severity};

        // Create a very long message
        let long_message = "A".repeat(10000);

        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            long_message.clone(),
        );

        let content = format_issue_content(&violation);

        // Should include the entire message without truncation
        assert!(content.contains(&long_message));

        // Should still be properly structured
        assert!(content.contains("# Rule Violation"));
    }

    #[tokio::test]
    async fn test_execute_check_command_with_create_issues_flag() {
        let context = setup_test_context().await;

        let cmd = super::super::cli::CheckCommand {
            patterns: vec!["test.rs".to_string()],
            rule: Some(vec!["nonexistent-rule".to_string()]),
            severity: None,
            category: None,
            create_issues: true,
        };

        let request = CheckCommandRequest::with_config(cmd, setup_test_agent_config());
        let result = execute_check_command_impl(request, &context).await;
        // Should succeed when no rules match (no violations to create issues for)
        assert!(result.is_ok());
    }
}
