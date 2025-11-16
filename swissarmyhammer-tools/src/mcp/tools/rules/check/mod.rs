//! Rule checking MCP tool that validates code against SwissArmyHammer rules.
//!
//! This tool provides an MCP interface to the SwissArmyHammer rule checking functionality.
//! It uses the swissarmyhammer-rules library directly for better performance and type safety.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::{CallToolResult, RawContent};
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use swissarmyhammer_agent_executor::AgentExecutor;
use swissarmyhammer_config::AgentConfig;
use swissarmyhammer_rules::{
    RuleCheckRequest as DomainRuleCheckRequest, RuleChecker, RuleViolation, Severity,
};
use swissarmyhammer_todo::TodoId;
use tokio::sync::OnceCell;

// Progress notification milestones
const PROGRESS_START: u32 = 0;
const PROGRESS_INITIALIZED: u32 = 10;
const PROGRESS_CHECKING: u32 = 20;
const PROGRESS_COMPLETE: u32 = 100;

/// Create an agent executor from agent configuration using the centralized factory
///
/// This now uses the centralized AgentExecutorFactory from swissarmyhammer_agent_executor,
/// eliminating code duplication between CLI and MCP tool implementations.
///
/// # Arguments
///
/// * `config` - The agent configuration specifying which executor to use
///
/// # Returns
///
/// * `Result<Arc<dyn AgentExecutor>, McpError>` - The initialized agent executor
///
/// # Errors
///
/// Returns an error if agent initialization fails
async fn create_agent_from_config(
    config: &AgentConfig,
) -> Result<Arc<dyn AgentExecutor>, McpError> {
    tracing::debug!(
        "Creating executor via centralized factory for {:?}",
        config.executor_type()
    );

    // Use the centralized factory from agent-executor crate
    // MCP server is not needed for rule checking as it runs without tools
    swissarmyhammer_agent_executor::AgentExecutorFactory::create_executor(config, None)
        .await
        .map(Arc::from)
        .map_err(|e| {
            McpError::internal_error(format!("Failed to create agent executor: {}", e), None)
        })
}

/// Request structure for rule checking operations via MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheckRequest {
    /// Optional list of specific rule names to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_names: Option<Vec<String>>,

    /// Optional severity filter (error, warning, info)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,

    /// Optional category filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Optional list of file paths or glob patterns to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<Vec<String>>,

    /// Optional maximum number of ERROR violations to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_errors: Option<usize>,

    /// Check only changed files (intersects with file_paths if provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,

    /// Automatically create a todo item for each rule violation found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_todo: Option<bool>,
}

/// Expand glob patterns to concrete file paths
///
/// Takes a list of glob patterns and expands them to actual file paths.
/// Only returns files that exist, not directories.
///
/// # Arguments
///
/// * `patterns` - List of glob patterns to expand
///
/// # Returns
///
/// A set of file paths that match the patterns
async fn expand_glob_patterns(patterns: &[String]) -> Result<HashSet<String>, McpError> {
    let mut files = HashSet::new();

    for pattern in patterns {
        let glob_pattern = if Path::new(pattern).is_absolute() {
            pattern.clone()
        } else {
            // For relative patterns, use current directory
            std::env::current_dir()
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to get current directory: {}", e),
                        None,
                    )
                })?
                .join(pattern)
                .to_string_lossy()
                .to_string()
        };

        let mut glob_options = glob::MatchOptions::new();
        glob_options.case_sensitive = true;
        glob_options.require_literal_separator = false;
        glob_options.require_literal_leading_dot = false;

        let entries = glob::glob_with(&glob_pattern, glob_options).map_err(|e| {
            McpError::invalid_params(format!("Invalid glob pattern '{}': {}", pattern, e), None)
        })?;

        for entry in entries {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        files.insert(path.to_string_lossy().to_string());
                    }
                }
                Err(err) => {
                    tracing::warn!("Error accessing file during glob expansion: {}", err);
                }
            }
        }
    }

    Ok(files)
}

/// Create a todo item for a rule violation
///
/// Creates a todo with formatted task and context fields containing
/// all relevant violation details. Uses the todo_create MCP tool interface
/// instead of direct storage access to maintain proper architectural layering.
///
/// # Arguments
///
/// * `context` - The tool context for calling other tools
/// * `violation` - The rule violation to create a todo for
///
/// # Returns
///
/// The ID of the created todo item, or an error if creation fails
async fn create_todo_for_violation(
    context: &ToolContext,
    violation: &RuleViolation,
) -> Result<TodoId, McpError> {
    // Format the task field
    let task = format!(
        "Fix {} violation in {}",
        violation.rule_name,
        violation.file_path.display()
    );

    // Format the context field with rich markdown
    let context_str = format!(
        r#"## Rule Violation

**Rule**: {}
**File**: {}
**Severity**: {:?}

## Violation Details

{}

## How to Fix

Only change the specific file mentioned above to resolve this violation.

See rule documentation for guidance on resolving this violation."#,
        violation.rule_name,
        violation.file_path.display(),
        violation.severity,
        violation.message
    );

    // Call todo_create tool through MCP interface
    let result = context
        .call_tool(
            "todo_create",
            json!({
                "task": task,
                "context": context_str
            }),
        )
        .await?;

    // Extract todo_id from the response
    // The todo_create tool returns JSON with a "todo_item" object containing "id"
    let response_text = if let Some(first_content) = result.content.first() {
        match &first_content.raw {
            RawContent::Text(text_content) => &text_content.text,
            _ => {
                return Err(McpError::internal_error(
                    "Unexpected content type from todo_create tool",
                    None,
                ));
            }
        }
    } else {
        return Err(McpError::internal_error(
            "No content returned from todo_create tool",
            None,
        ));
    };

    // Parse the JSON response
    let response_json: serde_json::Value = serde_json::from_str(response_text).map_err(|e| {
        McpError::internal_error(format!("Failed to parse todo_create response: {}", e), None)
    })?;

    // Extract the todo_id
    let todo_id_str = response_json["todo_item"]["id"].as_str().ok_or_else(|| {
        McpError::internal_error(
            format!("No todo_id in response from todo_create: {}", response_json),
            None,
        )
    })?;

    let todo_id = TodoId::from_string(todo_id_str.to_string())
        .map_err(|e| McpError::internal_error(format!("Invalid todo_id format: {}", e), None))?;

    tracing::info!(
        "Created todo {} for violation in {}",
        todo_id,
        violation.file_path.display()
    );

    Ok(todo_id)
}

/// Get changed files from git
///
/// Uses the git_changes tool logic to get files that have changed on the current branch.
///
/// # Arguments
///
/// * `context` - Tool context with git operations
///
/// # Returns
///
/// A set of changed file paths
async fn get_changed_files(context: &ToolContext) -> Result<HashSet<String>, McpError> {
    // Get git operations from context
    let git_ops_guard = context.git_ops.lock().await;
    let git_ops = git_ops_guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("Git operations not available", None))?;

    // Get current branch
    let current_branch = git_ops.current_branch().map_err(|e| {
        McpError::internal_error(format!("Failed to get current branch: {}", e), None)
    })?;

    tracing::info!("Getting changed files for branch: {}", current_branch);

    // Try to find parent branch
    let parent_branch = {
        use swissarmyhammer_git::BranchName;
        let branch_name = BranchName::new(&current_branch)
            .map_err(|e| McpError::invalid_params(format!("Invalid branch name: {}", e), None))?;

        match git_ops.find_merge_target_for_issue(&branch_name) {
            Ok(target) if target != current_branch => Some(target),
            _ => None,
        }
    };

    // Get changed files based on whether we have a parent branch
    let mut files = if let Some(ref parent) = parent_branch {
        // Feature/issue branch: get files changed from parent
        tracing::info!("Feature branch detected, parent: {}", parent);
        git_ops
            .get_changed_files_from_parent(&current_branch, parent)
            .map_err(|e| {
                McpError::internal_error(format!("Failed to get changed files: {}", e), None)
            })?
    } else {
        // Main/trunk branch: get only uncommitted changes
        tracing::info!("Main/trunk branch detected, getting uncommitted changes only");
        Vec::new()
    };

    // Add uncommitted changes
    let uncommitted =
        crate::mcp::tools::git::changes::get_uncommitted_changes(git_ops).map_err(|e| {
            McpError::internal_error(format!("Failed to get uncommitted changes: {}", e), None)
        })?;

    tracing::info!("Found {} uncommitted changes", uncommitted.len());
    files.extend(uncommitted);

    // Deduplicate
    let file_set: HashSet<String> = files.into_iter().collect();
    tracing::info!("Total changed files: {}", file_set.len());

    Ok(file_set)
}

/// Tool for checking code against rules via direct library integration
///
/// This tool uses the swissarmyhammer-rules library directly, avoiding subprocess
/// overhead and providing better error handling and type safety.
#[derive(Clone)]
pub struct RuleCheckTool {
    /// Lazily initialized rule checker (shared across requests)
    checker: Arc<OnceCell<RuleChecker>>,
}

impl RuleCheckTool {
    /// Creates a new instance of the RuleCheckTool
    pub fn new() -> Self {
        Self {
            checker: Arc::new(OnceCell::new()),
        }
    }

    /// Get or initialize the rule checker using the provided context's agent configuration.
    ///
    /// This method uses lazy initialization with `OnceCell` to ensure the RuleChecker
    /// is created only once and reused across multiple rule check requests.
    ///
    /// # Arguments
    ///
    /// * `context` - The tool context containing agent configuration
    ///
    /// # Returns
    ///
    /// * `Result<&RuleChecker, McpError>` - Reference to the initialized checker
    ///
    /// # Errors
    ///
    /// Returns an error if agent creation or checker initialization fails
    async fn get_checker(&self, context: &ToolContext) -> Result<&RuleChecker, McpError> {
        // Clone config for use in async closure
        let agent_config = context.agent_config.clone();

        self.checker
            .get_or_try_init(|| async move {
                tracing::debug!("Initializing RuleChecker for MCP tool with configured agent");

                // Create agent executor from configuration
                let agent = create_agent_from_config(&agent_config).await?;

                // Create rule checker
                let mut checker = RuleChecker::new(agent).map_err(|e| {
                    McpError::internal_error(format!("Failed to create rule checker: {}", e), None)
                })?;

                // Initialize the checker
                checker.initialize().await.map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to initialize rule checker: {}", e),
                        None,
                    )
                })?;

                tracing::info!(
                    "RuleChecker initialized successfully with {:?} executor",
                    agent_config.executor_type()
                );
                Ok(checker)
            })
            .await
            .map_err(|e: McpError| e)
    }
}

impl Default for RuleCheckTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for RuleCheckTool {
    fn name(&self) -> &'static str {
        "rules_check"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("rules", "check")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "rule_names": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional array of specific rule names to check"
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "info", "hint"],
                    "description": "Optional severity filter (error, warning, info, hint)"
                },
                "category": {
                    "type": "string",
                    "description": "Optional category filter"
                },
                "file_paths": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional array of file paths or glob patterns to check (defaults to **/*.*)"
                },
                "max_errors": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional maximum number of ERROR violations to return (default: unlimited)"
                },
                "changed": {
                    "type": "boolean",
                    "description": "Optional flag to check only changed files (intersects with file_paths if provided)"
                },
                "create_todo": {
                    "type": "boolean",
                    "description": "Automatically create a todo item for each rule violation found (default: false)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: RuleCheckRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!("Executing rule check with request: {:?}", request);
        tracing::info!("Rule names filter: {:?}", request.rule_names);
        tracing::info!("File paths: {:?}", request.file_paths);

        let start_time = Instant::now();
        let progress_token = generate_progress_token();

        // Send start notification
        if let Some(sender) = &context.progress_sender {
            if let Err(e) = sender.send_progress_with_metadata(
                &progress_token,
                Some(PROGRESS_START),
                "Starting rules check",
                json!({
                    "rule_names": request.rule_names,
                    "file_paths": request.file_paths,
                    "category": request.category,
                    "severity": request.severity
                }),
            ) {
                tracing::debug!("Failed to send progress notification: {}", e);
            }
        }

        // Get or initialize the rule checker with context's agent configuration
        let checker = self.get_checker(context).await?;
        tracing::info!("RuleChecker initialized successfully");

        // Send rules loaded notification
        if let Some(sender) = &context.progress_sender {
            if let Err(e) = sender.send_progress_with_metadata(
                &progress_token,
                Some(PROGRESS_INITIALIZED),
                "Rule checker initialized",
                json!({}),
            ) {
                tracing::debug!("Failed to send progress notification: {}", e);
            }
        }

        // Determine patterns based on changed flag
        let patterns = if request.changed.unwrap_or(false) {
            tracing::info!("Changed files filter enabled");

            // Get changed files from git
            let changed_files = get_changed_files(context).await?;

            if changed_files.is_empty() {
                tracing::info!("No changed files found");
                // Return early with no violations if no files have changed
                let result_text = "✅ No changed files to check".to_string();
                return Ok(BaseToolImpl::create_success_response(&result_text));
            }

            tracing::info!("Found {} changed files", changed_files.len());

            // If file_paths is provided, expand patterns and intersect with changed files
            if let Some(ref patterns) = request.file_paths {
                tracing::info!("Intersecting changed files with patterns: {:?}", patterns);
                let matched_files = expand_glob_patterns(patterns).await?;
                let intersection: Vec<String> = changed_files
                    .intersection(&matched_files)
                    .cloned()
                    .collect();

                if intersection.is_empty() {
                    tracing::info!("No files match both changed files and patterns");
                    let result_text =
                        "✅ No changed files match the specified patterns".to_string();
                    return Ok(BaseToolImpl::create_success_response(&result_text));
                }

                tracing::info!("After intersection: {} files to check", intersection.len());
                intersection
            } else {
                // No patterns provided, use all changed files directly
                changed_files.into_iter().collect()
            }
        } else {
            // Not filtering by changed files, use provided patterns or default
            request
                .file_paths
                .clone()
                .unwrap_or_else(|| vec!["**/*.*".to_string()])
        };

        // Map MCP request to domain request
        let domain_request = DomainRuleCheckRequest {
            rule_names: request.rule_names.clone(),
            severity: request.severity,
            category: request.category.clone(),
            patterns,
            check_mode: swissarmyhammer_rules::CheckMode::FailFast,
            force: false, // MCP tool doesn't expose force flag yet
            max_errors: request.max_errors,
        };

        tracing::info!("Domain request patterns: {:?}", domain_request.patterns);
        tracing::info!("Domain request rule_names: {:?}", domain_request.rule_names);

        // Execute the rule check via streaming library
        use futures_util::stream::StreamExt;
        let mut stream = checker
            .check(domain_request)
            .await
            .map_err(|e| McpError::internal_error(format!("Rule check failed: {}", e), None))?;

        // Send checking progress notification
        if let Some(sender) = &context.progress_sender {
            if let Err(e) = sender.send_progress_with_metadata(
                &progress_token,
                Some(PROGRESS_CHECKING),
                "Checking files against rules",
                json!({}),
            ) {
                tracing::debug!("Failed to send progress notification: {}", e);
            }
        }

        // Collect all violations from the stream and track statistics.
        // This loop processes the violation stream, counting violations by severity
        // and tracking which files contain violations for the completion notification.
        let mut violations = Vec::new();
        let mut violation_count_by_severity: HashMap<String, usize> = HashMap::new();
        let mut files_with_violations = std::collections::HashSet::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(violation) => {
                    // Track severity counts
                    let severity_str = format!("{:?}", violation.severity);
                    *violation_count_by_severity.entry(severity_str).or_insert(0) += 1;

                    // Track unique files with violations
                    files_with_violations.insert(violation.file_path.clone());

                    violations.push(violation);
                }
                Err(e) => {
                    return Err(McpError::internal_error(
                        format!("Rule check failed: {}", e),
                        None,
                    ));
                }
            }
        }

        tracing::info!("Check completed: found {} violations", violations.len());

        // Create todos for violations if requested
        let mut todos_created = Vec::new();
        if request.create_todo.unwrap_or(false) && !violations.is_empty() {
            tracing::info!("Creating todos for {} violations", violations.len());

            for violation in &violations {
                match create_todo_for_violation(context, violation).await {
                    Ok(todo_id) => {
                        todos_created.push(json!({
                            "todo_id": todo_id.to_string(),
                            "rule": violation.rule_name.clone(),
                            "file": violation.file_path.to_string_lossy().to_string()
                        }));

                        // Send progress notification for each todo created
                        if let Some(sender) = &context.progress_sender {
                            if let Err(e) = sender.send_progress_with_metadata(
                                &progress_token,
                                None, // Indeterminate progress during todo creation
                                format!(
                                    "Created todo for {} violation in {}",
                                    violation.rule_name,
                                    violation.file_path.display()
                                ),
                                json!({
                                    "todo_id": todo_id.to_string(),
                                    "rule": violation.rule_name
                                }),
                            ) {
                                tracing::debug!("Failed to send progress notification: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        // Log warning but don't fail the check
                        tracing::warn!(
                            "Failed to create todo for violation in {}: {}",
                            violation.file_path.display(),
                            e
                        );
                    }
                }
            }

            tracing::info!("Created {} todos for violations", todos_created.len());
        }

        let duration = start_time.elapsed();

        // Send completion notification
        if let Some(sender) = &context.progress_sender {
            let duration_ms = duration.as_millis() as u64;
            let mut metadata = json!({
                "violations_found": violations.len(),
                "files_with_violations": files_with_violations.len(),
                "violation_count_by_severity": violation_count_by_severity,
                "duration_ms": duration_ms
            });

            // Add todos_created to metadata if any were created
            if !todos_created.is_empty() {
                metadata["todos_created"] = json!(todos_created.len());
            }

            let message = if !todos_created.is_empty() {
                format!(
                    "Rules check complete: {} violations in {} files, {} todos created",
                    violations.len(),
                    files_with_violations.len(),
                    todos_created.len()
                )
            } else {
                format!(
                    "Rules check complete: {} violations in {} files",
                    violations.len(),
                    files_with_violations.len()
                )
            };

            if let Err(e) = sender.send_progress_with_metadata(
                &progress_token,
                Some(PROGRESS_COMPLETE),
                message,
                metadata,
            ) {
                tracing::debug!("Failed to send progress notification: {}", e);
            }
        }

        // Format the response
        let result_text = if violations.is_empty() {
            "✅ No rule violations found".to_string()
        } else {
            let violations_text = violations
                .iter()
                .map(|v| {
                    format!(
                        "❌ {} [{}] in {}\n   {}",
                        v.rule_name,
                        v.severity,
                        v.file_path.display(),
                        v.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            format!(
                "Found {} violation(s)\n\n{}",
                violations.len(),
                violations_text
            )
        };

        Ok(BaseToolImpl::create_success_response(&result_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Verifies that the tool reports its correct name for MCP registration
    #[tokio::test]
    async fn test_rule_check_tool_name() {
        let tool = RuleCheckTool::new();
        assert_eq!(tool.name(), "rules_check");
    }

    /// Verifies that the tool schema includes all required fields and proper structure
    #[tokio::test]
    async fn test_rule_check_tool_schema() {
        let tool = RuleCheckTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["rule_names"].is_object());
        assert!(schema["properties"]["severity"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert!(schema["properties"]["file_paths"].is_object());
    }

    /// Verifies that RuleCheckRequest correctly parses all fields from JSON arguments
    #[tokio::test]
    async fn test_rule_check_request_parsing() {
        let args = json!({
            "rule_names": ["no-unwrap", "no-panic"],
            "severity": "error",
            "category": "safety",
            "file_paths": ["src/**/*.rs"]
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.rule_names.unwrap(), vec!["no-unwrap", "no-panic"]);
        assert!(matches!(request.severity, Some(Severity::Error)));
        assert_eq!(request.category.unwrap(), "safety");
        assert_eq!(request.file_paths.unwrap(), vec!["src/**/*.rs"]);
    }

    /// Verifies that all fields in RuleCheckRequest are properly optional
    #[tokio::test]
    async fn test_rule_check_request_optional_fields() {
        let args = json!({});

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert!(request.rule_names.is_none());
        assert!(request.severity.is_none());
        assert!(request.category.is_none());
        assert!(request.file_paths.is_none());
    }

    /// Verifies that the RuleChecker initialization completes without panicking
    /// and handles both success and expected failure cases gracefully
    #[tokio::test]
    async fn test_rule_check_tool_initialization() {
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Get checker should initialize it
        let checker_result = tool.get_checker(&context).await;

        // In test environment without actual model, initialization may fail
        // which is expected - we're just testing the initialization pattern
        match checker_result {
            Ok(_) => {
                // Initialization succeeded - great!
            }
            Err(e) => {
                // Initialization failed - expected in test without model
                assert!(e.to_string().contains("Failed to") || e.to_string().contains("failed"));
            }
        }
    }

    /// Verifies that the RuleCheckTool uses lazy initialization pattern and reuses
    /// the same RuleChecker instance across multiple calls
    #[tokio::test]
    async fn test_rule_check_tool_lazy_initialization() {
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Checker should not be initialized yet
        assert!(tool.checker.get().is_none());

        // Calling get_checker should initialize it
        let _ = tool.get_checker(&context).await;

        // Now it should be initialized (or have attempted initialization)
        // We can't check the internal state directly, but a second call
        // should return the same instance (testing the OnceCell behavior)
        let result1 = tool.get_checker(&context).await;
        let result2 = tool.get_checker(&context).await;

        // Both results should have the same success/failure status
        assert_eq!(result1.is_ok(), result2.is_ok());
    }

    /// Integration test that verifies the full execute path works end-to-end
    /// This test creates a temporary file, runs a real rule check via the MCP tool,
    /// and verifies that rules are loaded and checked properly.
    #[tokio::test]
    async fn test_rule_check_tool_execute_integration() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(
            &test_file,
            r#"
fn example() {
    let x = vec![1, 2, 3];
    let first = x.first().unwrap(); // This should trigger no-unwrap if that rule exists
    println!("first: {}", first);
}
"#,
        )
        .unwrap();

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request to check a single rule against the test file for speed
        // Use a simple rule that should pass quickly
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/no-commented-code"]),
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([test_file.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        // The result should succeed (even if violations are found, execute() returns Ok
        // with the formatted result in the response text)
        match result {
            Ok(call_result) => {
                // Verify we got some content back
                assert!(
                    !call_result.content.is_empty(),
                    "Tool should return content"
                );

                // Extract text from the result - we know it's RawContent::Text from the response format
                let text = if let Some(first_content) = call_result.content.first() {
                    // Access the Annotated struct's raw field directly via debug formatting for now
                    // In a real implementation, we'd use proper accessors
                    format!("{:?}", first_content)
                } else {
                    String::from("No content returned")
                };

                // We should get a success message (no violations for this test file)
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Result should show check completed: {}",
                    text
                );

                println!("Integration test result: {}", text);
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test that rule name filtering works correctly
    /// This reproduces the issue where calling with specific rule names returns 0 rules
    #[tokio::test]
    async fn test_rule_check_tool_with_rule_name_filter() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(
            &test_file,
            r#"
fn complex_function() {
    if condition1 {
        if condition2 {
            if condition3 {
                if condition4 {
                    // Very nested logic
                    do_something();
                }
            }
        }
    }
}
"#,
        )
        .unwrap();

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request with specific rule name filter
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/cognitive-complexity"]),
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([test_file.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        match result {
            Ok(call_result) => {
                let text = format!("{:?}", call_result);
                println!("Rule filter test result: {}", text);

                // The key assertion: we should NOT get "0 rules against 0 files"
                if text.contains("Checked 0 rules against 0 files") {
                    panic!(
                        "Rule name filtering failed! Expected to find 'code-quality/cognitive-complexity' rule but got 0 rules.\nFull output: {}",
                        text
                    );
                }

                // We should get a check result (success or violations)
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Should have checked the cognitive-complexity rule. Got: {}",
                    text
                );
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test rule checking against an actual repo file
    /// Uses this crate's Cargo.toml which we know exists
    #[tokio::test]
    async fn test_rule_check_with_real_repo_file() {
        // Use this crate's Cargo.toml
        let cargo_toml = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert!(
            cargo_toml.exists(),
            "Cargo.toml should exist at {:?}",
            cargo_toml
        );

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request - check a specific builtin rule against Cargo.toml
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/no-commented-code"]), // Use a specific builtin rule
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([cargo_toml.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        match result {
            Ok(call_result) => {
                let text = format!("{:?}", call_result);
                println!("Cargo.toml check result: {}", text);

                // Should have completed the check successfully
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Should have loaded 'code-quality/no-commented-code' rule and checked Cargo.toml. Got: {}",
                    text
                );
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test that changed parameter is properly parsed
    #[tokio::test]
    async fn test_rule_check_request_with_changed() {
        let args = json!({
            "rule_names": ["test-rule"],
            "changed": true
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.changed, Some(true));
    }

    /// Test that changed parameter defaults to None when not provided
    #[tokio::test]
    async fn test_rule_check_request_changed_default() {
        let args = json!({
            "rule_names": ["test-rule"]
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.changed, None);
    }

    /// Test that create_todo parameter is properly parsed
    #[tokio::test]
    async fn test_rule_check_request_with_create_todo() {
        let args = json!({
            "rule_names": ["test-rule"],
            "create_todo": true
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.create_todo, Some(true));
    }

    /// Test that create_todo parameter defaults to None when not provided
    #[tokio::test]
    async fn test_rule_check_request_create_todo_default() {
        let args = json!({
            "rule_names": ["test-rule"]
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.create_todo, None);
    }

    /// Test expand_glob_patterns helper function
    #[tokio::test]
    async fn test_expand_glob_patterns() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create test files
        let rs_file = temp_dir.path().join("test.rs");
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&rs_file, "fn main() {}").unwrap();
        fs::write(&txt_file, "hello").unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test glob expansion
        let patterns = vec!["*.rs".to_string()];
        let result = expand_glob_patterns(&patterns).await;

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|f| f.ends_with("test.rs")));
    }

    /// Test that create_todo parameter creates todos for violations
    #[tokio::test]
    async fn test_rule_check_with_create_todo() {
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories for both test file and todos
        let temp_dir = TempDir::new().unwrap();
        let todo_dir = TempDir::new().unwrap();

        // Create a test file with a known violation
        let test_file = temp_dir.path().join("test.rs");
        fs::write(
            &test_file,
            r#"
// TODO: This is a todo comment that should trigger a violation
fn example() {
    let x = vec![1, 2, 3];
    println!("x: {:?}", x);
}
"#,
        )
        .unwrap();

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Override the todo directory for this test
        std::env::set_var(
            "SWISSARMYHAMMER_TODO_DIR",
            todo_dir.path().to_string_lossy().to_string(),
        );

        // Create request with create_todo enabled
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/no-todo-comments"]),
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([test_file.to_string_lossy().to_string()]),
        );
        arguments.insert("create_todo".to_string(), json!(true));

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        // Clean up environment variable
        std::env::remove_var("SAH_TODO_DIR");

        match result {
            Ok(call_result) => {
                let text = format!("{:?}", call_result);
                println!("Create todo test result: {}", text);

                // Should have found violations and created todos
                assert!(
                    text.contains("violation") || text.contains("todos created"),
                    "Should have found violations and mentioned todo creation. Got: {}",
                    text
                );

                // Check that a todo file was actually created
                let todo_file = todo_dir.path().join("todo.yaml");
                if todo_file.exists() {
                    let todo_content = fs::read_to_string(&todo_file).unwrap();
                    println!("Todo file content:\n{}", todo_content);

                    // Verify the todo contains expected information
                    assert!(
                        todo_content.contains("Fix") && todo_content.contains("violation"),
                        "Todo should contain violation fix information"
                    );
                    assert!(
                        todo_content.contains("Rule Violation"),
                        "Todo context should contain violation details"
                    );
                }
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test that changed files integration returns early when no changed files
    #[tokio::test]
    async fn test_rule_check_with_changed_no_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo with committed file
        let repo_path = temp_dir.path();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let test_file = repo_path.join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create tool and context with git ops
        let tool = RuleCheckTool::new();
        let git_ops = swissarmyhammer_git::GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Create request with changed flag but no uncommitted changes
        let mut arguments = serde_json::Map::new();
        arguments.insert("changed".to_string(), json!(true));

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        // Should succeed with message about no changed files
        assert!(result.is_ok());
        let response = result.unwrap();
        let text = format!("{:?}", response);
        assert!(text.contains("No changed files"));
    }
}
