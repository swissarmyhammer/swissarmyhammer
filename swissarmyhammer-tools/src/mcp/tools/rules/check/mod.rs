//! Rule checking MCP tool that validates code against SwissArmyHammer rules.
//! sah rule ignore test_rule_with_allow
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
use std::time::Instant;
use swissarmyhammer_config::AgentUseCase;
use swissarmyhammer_rules::{
    AgentConfig, RuleCheckRequest as DomainRuleCheckRequest, RuleChecker, RuleViolation, Severity,
};
use swissarmyhammer_todo::TodoId;

// Progress notification milestones
const PROGRESS_START: u32 = 0;
const PROGRESS_INITIALIZED: u32 = 10;
const PROGRESS_CHECKING: u32 = 20;
const PROGRESS_COMPLETE: u32 = 100;

/// Create agent configuration for rule checking
///
/// Creates an AgentConfig from the tool context for rule checking.
/// Note: Rule checking does NOT use MCP tools - it's a simple prompt
/// that checks code against rules and returns PASS/VIOLATION.
/// Therefore, mcp_config is always None for rule checking.
///
/// # Arguments
///
/// * `context` - The tool context containing agent configuration
///
/// # Returns
///
/// * `Result<AgentConfig, McpError>` - The agent configuration
async fn create_agent_config(context: &ToolContext) -> Result<AgentConfig, McpError> {
    let model_config = context.get_agent_for_use_case(AgentUseCase::Rules);

    Ok(AgentConfig {
        model_config: (*model_config).clone(),
        // Rule checking does not need MCP tools - it's a simple prompt/response
        mcp_config: None,
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

    /// Maximum number of concurrent rule checks (default: 4)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<usize>,
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
    // Use the common glob_utils which handles .git and .swissarmyhammer filtering
    use swissarmyhammer_common::glob_utils::{
        expand_glob_patterns as expand_common, GlobExpansionConfig,
    };

    let config = GlobExpansionConfig::default();
    let paths = expand_common(patterns, &config).map_err(|e| {
        McpError::internal_error(format!("Failed to expand glob patterns: {}", e), None)
    })?;

    // Convert Vec<PathBuf> to HashSet<String>
    let files: HashSet<String> = paths
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    Ok(files)
}

/// Extract JSON field value from a tool call result
///
/// Consolidates the common pattern of extracting and parsing JSON from tool call results.
///
/// # Arguments
///
/// * `result` - The tool call result to extract JSON from
/// * `field_path` - Path to the field to extract (e.g., ["todo_item", "id"])
///
/// # Returns
///
/// The extracted JSON value or an error if extraction fails
fn extract_json_field_from_result(
    result: &CallToolResult,
    field_path: &[&str],
) -> Result<serde_json::Value, McpError> {
    let response_text = if let Some(first_content) = result.content.first() {
        match &first_content.raw {
            RawContent::Text(text_content) => &text_content.text,
            _ => {
                return Err(McpError::internal_error(
                    "Unexpected content type from tool result",
                    None,
                ));
            }
        }
    } else {
        return Err(McpError::internal_error(
            "No content returned from tool",
            None,
        ));
    };

    let mut response_json: serde_json::Value =
        serde_json::from_str(response_text).map_err(|e| {
            McpError::internal_error(format!("Failed to parse tool response: {}", e), None)
        })?;

    for field in field_path {
        response_json = response_json[field].clone();
        if response_json.is_null() {
            return Err(McpError::internal_error(
                format!("Field '{}' not found in tool response", field),
                None,
            ));
        }
    }

    Ok(response_json)
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

    // Extract todo_id from the response using the common helper
    let todo_id_value = extract_json_field_from_result(&result, &["todo_item", "id"])?;
    let todo_id_str = todo_id_value.as_str().ok_or_else(|| {
        McpError::internal_error(format!("todo_id is not a string: {}", todo_id_value), None)
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

/// Send a progress notification, handling errors gracefully
///
/// # Arguments
///
/// * `context` - The tool context containing the progress sender
/// * `token` - The progress token for this operation
/// * `progress` - Optional progress percentage (0-100)
/// * `message` - Progress message to display
/// * `metadata` - Additional metadata to include
fn send_progress(
    context: &ToolContext,
    token: &str,
    progress: Option<u32>,
    message: impl Into<String>,
    metadata: serde_json::Value,
) {
    if let Some(sender) = &context.progress_sender {
        if let Err(e) =
            sender.send_progress_with_metadata(token, progress, message.into(), metadata)
        {
            tracing::debug!("Failed to send progress notification: {}", e);
        }
    }
}

/// Accumulates statistics about rule violations during checking
///
/// This struct provides a consolidated way to track violation counts by severity
/// and which files contain violations, eliminating duplication in the stream
/// processing logic.
struct ViolationStatistics {
    /// Count of violations by severity level (e.g., "Error", "Warning")
    counts_by_severity: HashMap<String, usize>,
    /// Set of unique file paths that contain violations
    affected_files: HashSet<std::path::PathBuf>,
}

impl ViolationStatistics {
    /// Creates a new empty statistics tracker
    fn new() -> Self {
        Self {
            counts_by_severity: HashMap::new(),
            affected_files: HashSet::new(),
        }
    }

    /// Records a violation in the statistics
    ///
    /// Updates both severity counts and affected files set.
    ///
    /// # Arguments
    ///
    /// * `violation` - The violation to record
    fn record(&mut self, violation: &RuleViolation) {
        let severity_str = format!("{:?}", violation.severity);
        *self.counts_by_severity.entry(severity_str).or_insert(0) += 1;
        self.affected_files.insert(violation.file_path.clone());
    }

    /// Returns the count of violations by severity
    fn counts_by_severity(&self) -> &HashMap<String, usize> {
        &self.counts_by_severity
    }

    /// Returns the number of affected files
    fn affected_files_count(&self) -> usize {
        self.affected_files.len()
    }
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
pub struct RuleCheckTool;

impl RuleCheckTool {
    /// Creates a new instance of the RuleCheckTool
    pub fn new() -> Self {
        Self
    }

    /// Load rules from the library and filter them based on request parameters
    ///
    /// # Arguments
    ///
    /// * `request` - The rule check request containing filter parameters
    ///
    /// # Returns
    ///
    /// Tuple of (filtered_rules, unfiltered_rules)
    async fn load_and_filter_rules(
        &self,
        request: &RuleCheckRequest,
    ) -> Result<
        (
            Vec<swissarmyhammer_rules::Rule>,
            Vec<swissarmyhammer_rules::Rule>,
        ),
        McpError,
    > {
        let mut rule_library = swissarmyhammer_rules::RuleLibrary::new();
        let mut rule_resolver = swissarmyhammer_rules::RuleResolver::new();
        let mut rules_vec = Vec::new();
        rule_resolver
            .load_all_rules(&mut rules_vec)
            .map_err(|e| McpError::internal_error(format!("Failed to load rules: {}", e), None))?;

        for rule in &rules_vec {
            rule_library.add(rule.clone()).map_err(|e| {
                McpError::internal_error(format!("Failed to add rule to library: {}", e), None)
            })?;
        }

        let all_rules: Vec<swissarmyhammer_rules::Rule> = rule_library
            .list()
            .map_err(|e| McpError::internal_error(format!("Failed to list rules: {}", e), None))?
            .into_iter()
            .filter(|rule| {
                if let Some(ref names) = request.rule_names {
                    if !names.contains(&rule.name) {
                        return false;
                    }
                }
                if let Some(ref severity_filter) = request.severity {
                    if rule.severity != *severity_filter {
                        return false;
                    }
                }
                if let Some(ref category) = request.category {
                    if rule.category.as_deref() != Some(category) {
                        return false;
                    }
                }
                true
            })
            .collect();

        let (filtered_rules, unfiltered_rules): (Vec<_>, Vec<_>) = all_rules
            .into_iter()
            .partition(|rule| rule.has_tool_filter());

        tracing::info!(
            "Rules loaded: {} with tool filtering, {} without",
            filtered_rules.len(),
            unfiltered_rules.len()
        );

        Ok((filtered_rules, unfiltered_rules))
    }

    /// Determine file patterns based on request parameters and changed files
    ///
    /// # Arguments
    ///
    /// * `request` - The rule check request
    /// * `context` - The tool context
    ///
    /// # Returns
    ///
    /// Vector of file patterns to check, or None if no files to check
    async fn determine_file_patterns(
        &self,
        request: &RuleCheckRequest,
        context: &ToolContext,
    ) -> Result<Option<Vec<String>>, McpError> {
        if !request.changed.unwrap_or(false) {
            return Ok(Some(
                request
                    .file_paths
                    .clone()
                    .unwrap_or_else(|| vec!["**/*.*".to_string()]),
            ));
        }

        tracing::info!("Changed files filter enabled");

        let changed_files = get_changed_files(context).await?;

        if changed_files.is_empty() {
            tracing::info!("No changed files found");
            return Ok(None);
        }

        tracing::info!("Found {} changed files", changed_files.len());

        if let Some(ref patterns) = request.file_paths {
            tracing::info!("Intersecting changed files with patterns: {:?}", patterns);
            let matched_files = expand_glob_patterns(patterns).await?;
            let intersection: Vec<String> = changed_files
                .intersection(&matched_files)
                .cloned()
                .collect();

            if intersection.is_empty() {
                tracing::info!("No files match both changed files and patterns");
                return Ok(None);
            }

            tracing::info!("After intersection: {} files to check", intersection.len());
            Ok(Some(intersection))
        } else {
            Ok(Some(changed_files.into_iter().collect()))
        }
    }

    /// Collect violations using ACP agents
    ///
    /// Creates an ACP agent and uses the RuleChecker to check all rules.
    ///
    /// # Arguments
    ///
    /// * `filtered_rules` - Rules with tool filtering (currently treated the same)
    /// * `unfiltered_rules` - Rules without tool filtering
    /// * `patterns` - File patterns to check
    /// * `request` - The rule check request
    /// * `context` - The tool context
    /// * `progress_token` - Token for progress notifications
    ///
    /// # Returns
    ///
    /// Vector of all violations found
    async fn collect_violations(
        &self,
        filtered_rules: &[swissarmyhammer_rules::Rule],
        unfiltered_rules: &[swissarmyhammer_rules::Rule],
        patterns: &[String],
        request: &RuleCheckRequest,
        context: &ToolContext,
        progress_token: &str,
    ) -> Result<Vec<RuleViolation>, McpError> {
        let total_rules = filtered_rules.len() + unfiltered_rules.len();
        let start_time = Instant::now();

        tracing::info!(
            "Processing {} total rules ({} filtered, {} unfiltered) via ACP agents",
            total_rules,
            filtered_rules.len(),
            unfiltered_rules.len()
        );

        // Note: Tool filtering for rules is not yet supported with ACP agents
        // All rules will use the same agent configuration
        if !filtered_rules.is_empty() {
            tracing::warn!(
                "Tool filtering for {} rules is not supported yet with ACP agents - using default agent",
                filtered_rules.len()
            );
        }

        // Combine all rules into a single list
        let mut all_rules = Vec::new();
        all_rules.extend_from_slice(filtered_rules);
        all_rules.extend_from_slice(unfiltered_rules);

        if all_rules.is_empty() {
            tracing::info!("No rules to check");
            return Ok(Vec::new());
        }

        // Create agent config from tool context
        let agent_config = create_agent_config(context).await?;

        // Create checker with agent config
        let checker = RuleChecker::new(agent_config).map_err(|e| {
            McpError::internal_error(format!("Failed to create rule checker: {}", e), None)
        })?;

        // Build request with all rules
        let rule_names: Vec<String> = all_rules.iter().map(|r| r.name.clone()).collect();

        let domain_request = DomainRuleCheckRequest {
            rule_names: Some(rule_names.clone()),
            severity: request.severity,
            category: request.category.clone(),
            patterns: patterns.to_vec(),
            check_mode: swissarmyhammer_rules::CheckMode::CollectAll,
            force: false,
            max_errors: request.max_errors,
            max_concurrency: request.max_concurrency,
        };

        send_progress(
            context,
            progress_token,
            Some(PROGRESS_CHECKING),
            format!("Checking {} rules via ACP agents", all_rules.len()),
            json!({
                "total_rules": total_rules,
                "rule_names": rule_names
            }),
        );

        // Process all rules via streaming
        use futures_util::stream::StreamExt;
        let mut stream = checker
            .check(domain_request)
            .await
            .map_err(|e| McpError::internal_error(format!("Rule check failed: {}", e), None))?;

        let mut violations = Vec::new();
        let mut violation_count = 0;
        let mut last_progress_update = Instant::now();
        let progress_update_interval = std::time::Duration::from_secs(2);

        while let Some(result) = stream.next().await {
            match result {
                Ok(violation) => {
                    violations.push(violation);
                    violation_count += 1;

                    // Send periodic progress updates with timing info
                    if last_progress_update.elapsed() >= progress_update_interval {
                        let elapsed = start_time.elapsed();
                        let elapsed_secs = elapsed.as_secs();

                        let progress_msg = format!(
                            "Checking {} rules - found {} violations ({}s elapsed)",
                            all_rules.len(),
                            violation_count,
                            elapsed_secs
                        );

                        tracing::info!("{}", progress_msg);
                        send_progress(
                            context,
                            progress_token,
                            Some(PROGRESS_CHECKING + 10),
                            progress_msg,
                            json!({
                                "violations_found": violation_count,
                                "elapsed_seconds": elapsed_secs,
                                "total_rules": total_rules
                            }),
                        );

                        last_progress_update = Instant::now();
                    }
                }
                Err(e) => {
                    return Err(McpError::internal_error(
                        format!("Rule check failed: {}", e),
                        None,
                    ));
                }
            }
        }

        let elapsed = start_time.elapsed();
        tracing::info!(
            "ACP agent check completed: found {} violations in {:.2}s",
            violations.len(),
            elapsed.as_secs_f64()
        );

        Ok(violations)
    }

    /// Create todos for violations if requested
    ///
    /// # Arguments
    ///
    /// * `violations` - The violations to create todos for
    /// * `request` - The rule check request
    /// * `context` - The tool context
    /// * `progress_token` - Token for progress notifications
    ///
    /// # Returns
    ///
    /// Vector of created todo metadata
    async fn create_todos_for_violations(
        &self,
        violations: &[RuleViolation],
        request: &RuleCheckRequest,
        context: &ToolContext,
        progress_token: &str,
    ) -> Result<Vec<serde_json::Value>, McpError> {
        let mut todos_created = Vec::new();

        if !request.create_todo.unwrap_or(false) || violations.is_empty() {
            return Ok(todos_created);
        }

        tracing::info!("Creating todos for {} violations", violations.len());

        for violation in violations {
            match create_todo_for_violation(context, violation).await {
                Ok(todo_id) => {
                    todos_created.push(json!({
                        "todo_id": todo_id.to_string(),
                        "rule": violation.rule_name.clone(),
                        "file": violation.file_path.to_string_lossy().to_string()
                    }));

                    send_progress(
                        context,
                        progress_token,
                        None,
                        format!(
                            "Created todo for {} violation in {}",
                            violation.rule_name,
                            violation.file_path.display()
                        ),
                        json!({
                            "todo_id": todo_id.to_string(),
                            "rule": violation.rule_name
                        }),
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create todo for violation in {}: {}",
                        violation.file_path.display(),
                        e
                    );
                }
            }
        }

        tracing::info!("Created {} todos for violations", todos_created.len());
        Ok(todos_created)
    }

    /// Initialize execution and send start notification
    ///
    /// # Arguments
    ///
    /// * `request` - The rule check request
    /// * `context` - The tool context
    ///
    /// # Returns
    ///
    /// Tuple of (start_time, progress_token)
    fn initialize_execution(
        &self,
        request: &RuleCheckRequest,
        context: &ToolContext,
    ) -> (Instant, String) {
        tracing::info!("Executing rule check with request: {:?}", request);
        tracing::info!("Rule names filter: {:?}", request.rule_names);
        tracing::info!("File paths: {:?}", request.file_paths);

        let start_time = Instant::now();
        let progress_token = generate_progress_token();

        tracing::info!("Starting rules check");
        send_progress(
            context,
            &progress_token,
            Some(PROGRESS_START),
            "Starting rules check",
            json!({
                "rule_names": request.rule_names,
                "file_paths": request.file_paths,
                "category": request.category,
                "severity": request.severity
            }),
        );

        (start_time, progress_token)
    }

    /// Send completion notification with metadata
    ///
    /// # Arguments
    ///
    /// * `context` - The tool context
    /// * `progress_token` - Token for progress notifications
    /// * `violations` - The violations found
    /// * `statistics` - Statistics about violations
    /// * `todos_created` - Todos that were created
    /// * `start_time` - When the check started
    fn send_completion_notification(
        &self,
        context: &ToolContext,
        progress_token: &str,
        violations: &[RuleViolation],
        statistics: &ViolationStatistics,
        todos_created: &[serde_json::Value],
        start_time: Instant,
    ) {
        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;
        let mut metadata = json!({
            "violations_found": violations.len(),
            "files_with_violations": statistics.affected_files_count(),
            "violation_count_by_severity": statistics.counts_by_severity(),
            "duration_ms": duration_ms
        });

        if !todos_created.is_empty() {
            metadata["todos_created"] = json!(todos_created.len());
        }

        let message = if !todos_created.is_empty() {
            format!(
                "Rules check complete: {} violations in {} files, {} todos created",
                violations.len(),
                statistics.affected_files_count(),
                todos_created.len()
            )
        } else {
            format!(
                "Rules check complete: {} violations in {} files",
                violations.len(),
                statistics.affected_files_count()
            )
        };

        tracing::info!("{}", message);
        send_progress(
            context,
            progress_token,
            Some(PROGRESS_COMPLETE),
            message,
            metadata,
        );
    }

    /// Compute violation statistics
    ///
    /// # Arguments
    ///
    /// * `violations` - The violations to compute statistics for
    ///
    /// # Returns
    ///
    /// Computed statistics
    fn compute_statistics(violations: &[RuleViolation]) -> ViolationStatistics {
        let mut statistics = ViolationStatistics::new();
        for v in violations {
            statistics.record(v);
        }
        statistics
    }

    /// Format the final response text
    ///
    /// # Arguments
    ///
    /// * `violations` - The violations found
    ///
    /// # Returns
    ///
    /// Formatted response text
    fn format_response(&self, violations: &[RuleViolation]) -> String {
        if violations.is_empty() {
            return "✓ No rule violations found".to_string();
        }

        let violations_text = violations
            .iter()
            .map(|v| {
                format!(
                    "✗ {} [{}] in {}\n   {}",
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
                },
                "max_concurrency": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum number of concurrent rule checks (default: 4)"
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

        let (start_time, progress_token) = self.initialize_execution(&request, context);

        let (filtered_rules, unfiltered_rules) = self.load_and_filter_rules(&request).await?;

        send_progress(
            context,
            &progress_token,
            Some(PROGRESS_INITIALIZED),
            format!(
                "Rule checker initialized ({} filtered, {} unfiltered rules)",
                filtered_rules.len(),
                unfiltered_rules.len()
            )
            .as_str(),
            json!({}),
        );

        let patterns = match self.determine_file_patterns(&request, context).await? {
            Some(patterns) => patterns,
            None => {
                let msg = if request.file_paths.is_some() {
                    "✓ No changed files match the specified patterns"
                } else {
                    "✓ No changed files to check"
                };
                return Ok(BaseToolImpl::create_success_response(msg));
            }
        };

        let violations = self
            .collect_violations(
                &filtered_rules,
                &unfiltered_rules,
                &patterns,
                &request,
                context,
                &progress_token,
            )
            .await?;

        let statistics = Self::compute_statistics(&violations);

        tracing::info!(
            "Total check completed: found {} violations",
            violations.len()
        );

        let todos_created = self
            .create_todos_for_violations(&violations, &request, context, &progress_token)
            .await?;

        self.send_completion_notification(
            context,
            &progress_token,
            &violations,
            &statistics,
            &todos_created,
            start_time,
        );

        let result_text = self.format_response(&violations);
        Ok(BaseToolImpl::create_success_response(&result_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// RAII guard for environment variables that automatically cleans up on drop
    ///
    /// This ensures environment variables are properly cleaned up even if tests panic.
    struct EnvVarGuard {
        key: String,
    }

    impl EnvVarGuard {
        /// Sets an environment variable and returns a guard that will clean it up
        fn set(key: impl Into<String>, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let key = key.into();
            std::env::set_var(&key, value);
            Self { key }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            std::env::remove_var(&self.key);
        }
    }

    /// Macro to generate builder methods with consistent patterns
    ///
    /// Reduces boilerplate in builder method implementations by generating
    /// methods that insert values and return self. Supports any serializable type.
    macro_rules! builder_method {
        ($method_name:ident, $field_name:expr, $type:ty) => {
            fn $method_name(mut self, value: $type) -> Self {
                self.args.insert($field_name.to_string(), json!(value));
                self
            }
        };
    }

    /// Builder for creating test arguments with fluent interface
    ///
    /// Eliminates duplication in test argument construction by providing
    /// a declarative way to build argument maps. Uses macros to reduce
    /// boilerplate in method definitions.
    struct TestArgsBuilder {
        args: serde_json::Map<String, serde_json::Value>,
    }

    impl TestArgsBuilder {
        /// Creates a new builder with empty arguments
        fn new() -> Self {
            Self {
                args: serde_json::Map::new(),
            }
        }

        builder_method!(with_rule_names, "rule_names", Vec<&str>);
        builder_method!(with_file_paths, "file_paths", Vec<String>);
        builder_method!(with_create_todo, "create_todo", bool);
        builder_method!(with_changed, "changed", bool);

        /// Builds the final arguments map
        fn build(self) -> serde_json::Map<String, serde_json::Value> {
            self.args
        }
    }

    /// Builder for git commands that chains operations and handles errors uniformly
    ///
    /// Provides a fluent interface for constructing and executing git commands,
    /// eliminating duplication in git repository setup.
    /// Consolidates git command execution patterns for testing
    ///
    /// This helper eliminates duplication in git repository setup
    /// by providing a unified interface for common git operations using git2.
    struct GitTestHelper;

    impl GitTestHelper {
        /// Configures git user for a repository
        fn configure_user(repo: &git2::Repository) -> std::io::Result<()> {
            let mut config = repo
                .config()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            config
                .set_str("user.email", "test@example.com")
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            config
                .set_str("user.name", "Test User")
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            Ok(())
        }

        /// Adds and commits files to the repository
        fn add_and_commit(repo: &git2::Repository, message: &str) -> std::io::Result<()> {
            let mut index = repo
                .index()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            index
                .write()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            let tree_id = index
                .write_tree()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let tree = repo
                .find_tree(tree_id)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            let signature = git2::Signature::now("Test User", "test@example.com")
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::Other, "Failed to get head target")
                    })?;
                    Some(
                        repo.find_commit(parent_oid)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();

            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            Ok(())
        }

        /// Initializes a git repository with a file
        fn init_repo_with_file(
            repo_path: &Path,
            filename: &str,
            content: &str,
        ) -> std::io::Result<()> {
            let repo = git2::Repository::init(repo_path)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            Self::configure_user(&repo)?;

            let file_path = repo_path.join(filename);
            std::fs::write(&file_path, content)?;

            Self::add_and_commit(&repo, "Initial commit")?;

            Ok(())
        }
    }

    /// Creates a git context from an initialized repository
    ///
    /// Consolidates git setup logic by creating a ToolContext with git operations
    /// configured for the specified repository path.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository
    ///
    /// # Returns
    ///
    /// Configured ToolContext with git operations
    async fn create_git_context(repo_path: &Path) -> ToolContext {
        let git_ops = swissarmyhammer_git::GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);
        context
    }

    /// Unified test harness for rule check testing
    ///
    /// Consolidates all test setup concerns including temp directories,
    /// git initialization, file creation, tool/context setup, and argument building.
    /// Provides a fluent interface for test configuration.
    struct TestHarness {
        temp_dir: Option<TempDir>,
        filename: String,
        content: String,
        rule_names: Vec<String>,
        with_git: bool,
        additional_args: serde_json::Map<String, serde_json::Value>,
    }

    impl TestHarness {
        /// Creates a new test harness with default configuration
        fn new() -> Self {
            Self {
                temp_dir: None,
                filename: "test.rs".to_string(),
                content: String::new(),
                rule_names: Vec::new(),
                with_git: false,
                additional_args: serde_json::Map::new(),
            }
        }

        /// Sets the file to create with the specified content
        fn with_file(mut self, filename: &str, content: &str) -> Self {
            self.filename = filename.to_string();
            self.content = content.to_string();
            self
        }

        /// Enables git repository initialization
        fn with_git(mut self) -> Self {
            self.with_git = true;
            self
        }

        /// Sets the rule names to check
        fn with_rules(mut self, rules: Vec<&str>) -> Self {
            self.rule_names = rules.iter().map(|s| s.to_string()).collect();
            self
        }

        /// Adds additional arguments to the test
        fn with_args(mut self, args: serde_json::Map<String, serde_json::Value>) -> Self {
            self.additional_args.extend(args);
            self
        }

        /// Builds the test setup and returns all necessary components
        ///
        /// # Returns
        ///
        /// A tuple of (tool, context, temp_dir, arguments) ready for testing
        async fn build(
            mut self,
        ) -> (
            RuleCheckTool,
            ToolContext,
            TempDir,
            serde_json::Map<String, serde_json::Value>,
        ) {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join(&self.filename);
            fs::write(&test_file, &self.content).unwrap();

            let tool = RuleCheckTool::new();
            let context = if self.with_git {
                GitTestHelper::init_repo_with_file(temp_dir.path(), &self.filename, &self.content)
                    .unwrap();
                create_git_context(temp_dir.path()).await
            } else {
                crate::test_utils::create_test_context().await
            };

            let rule_names: Vec<&str> = self.rule_names.iter().map(|s| s.as_str()).collect();
            let mut arguments = TestArgsBuilder::new()
                .with_rule_names(rule_names)
                .with_file_paths(vec![test_file.to_string_lossy().to_string()])
                .build();

            arguments.extend(self.additional_args);
            self.temp_dir = Some(temp_dir);

            (tool, context, self.temp_dir.take().unwrap(), arguments)
        }
    }

    /// Helper to execute a closure with a git repository setup
    ///
    /// Creates a temporary git repository, initializes it with a file,
    /// creates a git operations context, and executes the provided closure.
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to create in the repo
    /// * `content` - Content to write to the file
    /// * `f` - Async closure to execute with the test context and repo path
    ///
    /// # Returns
    ///
    /// Result of the closure execution
    async fn with_test_git_repo<F, Fut, T>(filename: &str, content: &str, f: F) -> T
    where
        F: FnOnce(ToolContext, &Path) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        GitTestHelper::init_repo_with_file(repo_path, filename, content).unwrap();
        let context = create_git_context(repo_path).await;

        f(context, repo_path).await
    }

    /// Asserts that a result contains expected patterns
    ///
    /// Helper function for asserting that tool results contain expected content.
    ///
    /// # Arguments
    ///
    /// * `result` - The result to check
    /// * `expected_patterns` - Patterns to assert in the result
    /// * `context_msg` - Context message for assertion failures
    fn assert_result_contains(
        result: Result<CallToolResult, McpError>,
        expected_patterns: &[&str],
        context_msg: &str,
    ) {
        let text = match result {
            Ok(call_result) => format!("{:?}", call_result),
            Err(e) => panic!("Tool execution failed: {}", e),
        };

        println!("Test result: {}", text);

        let found = expected_patterns
            .iter()
            .any(|pattern| text.contains(pattern));
        assert!(
            found,
            "{}\nExpected one of: {:?}\nGot: {}",
            context_msg, expected_patterns, text
        );
    }

    /// Comprehensive test helper that combines setup, execution, and assertion
    ///
    /// This eliminates duplication by providing a single fluent interface that handles
    /// the complete test lifecycle: setup -> execute -> assert patterns.
    /// Supports both regular and git-based tests.
    ///
    /// # Arguments
    ///
    /// * `test_code` - The Rust code to write to the test file
    /// * `rule_names` - List of rule names to check
    /// * `additional_args` - Optional additional arguments to include
    /// * `expected_patterns` - Patterns to assert in the result
    /// * `context_msg` - Context message for assertion failures
    /// * `setup_git` - Whether to initialize a git repository
    async fn setup_execute_and_assert(
        test_code: &str,
        rule_names: Vec<&str>,
        additional_args: Option<serde_json::Map<String, serde_json::Value>>,
        expected_patterns: &[&str],
        context_msg: &str,
        setup_git: bool,
    ) {
        // Build test harness with all configuration
        let mut harness = TestHarness::new()
            .with_file("test.rs", test_code)
            .with_rules(rule_names);

        if setup_git {
            harness = harness.with_git();
        }

        if let Some(args) = additional_args {
            harness = harness.with_args(args);
        }

        // Execute the test
        let (tool, context, _temp_dir, arguments) = harness.build().await;
        let result = tool.execute(arguments, &context).await;

        // Assert expected patterns in result using the shared helper
        assert_result_contains(result, expected_patterns, context_msg);
    }

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

    /// Table-driven test configuration for rule checks
    ///
    /// Eliminates duplication by defining test cases as data rather than separate functions.
    struct RuleCheckTestCase {
        name: &'static str,
        test_code: &'static str,
        rule_names: Vec<&'static str>,
        expected_patterns: Vec<&'static str>,
        context_msg: &'static str,
    }

    /// Executes a table of rule check test cases
    ///
    /// This function provides a declarative way to run multiple test cases,
    /// eliminating the need for manual iteration and logging in each test function.
    ///
    /// # Arguments
    ///
    /// * `test_cases` - Slice of test case configurations to execute
    async fn run_rule_check_test_cases(test_cases: &[RuleCheckTestCase]) {
        for test_case in test_cases {
            println!("Running test case: {}", test_case.name);
            setup_execute_and_assert(
                test_case.test_code,
                test_case.rule_names.clone(),
                None,
                &test_case.expected_patterns,
                test_case.context_msg,
                false,
            )
            .await;
        }
    }

    /// Test basic integration - rule checking completes without errors
    #[tokio::test]
    async fn test_rule_check_tool_execute_integration() {
        let test_cases = vec![
            RuleCheckTestCase {
                name: "no-commented-code rule",
                test_code: r#"
fn example() {
    let x = vec![1, 2, 3];
    let first = x.first().unwrap(); // This should trigger no-unwrap if that rule exists
    println!("first: {}", first);
}
"#,
                rule_names: vec!["code-quality/no-commented-code"],
                expected_patterns: vec!["No rule violations found", "violation"],
                context_msg: "Result should show check completed",
            },
            RuleCheckTestCase {
                name: "cognitive-complexity rule",
                test_code: r#"
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
                rule_names: vec!["code-quality/cognitive-complexity"],
                expected_patterns: vec!["No rule violations found", "violation"],
                context_msg: "Should have checked the cognitive-complexity rule",
            },
        ];

        run_rule_check_test_cases(&test_cases).await;
    }

    /// Test rule checking against an actual repo file
    /// Uses this crate's Cargo.toml which we know exists
    #[tokio::test]
    async fn test_rule_check_with_real_repo_file() {
        let cargo_toml = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert!(
            cargo_toml.exists(),
            "Cargo.toml should exist at {:?}",
            cargo_toml
        );

        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        let arguments = TestArgsBuilder::new()
            .with_rule_names(vec!["code-quality/no-commented-code"])
            .with_file_paths(vec![cargo_toml.to_string_lossy().to_string()])
            .build();

        let result = tool.execute(arguments, &context).await;
        assert_result_contains(
            result,
            &["No rule violations found", "violation"],
            "Should have loaded 'code-quality/no-commented-code' rule and checked Cargo.toml",
        );
    }

    /// Generic test helper for optional fields
    ///
    /// Tests that a field correctly parses to Some(value) when present
    /// and None when absent, eliminating duplication in field testing.
    /// Supports any type that implements PartialEq and Debug.
    ///
    /// # Arguments
    ///
    /// * `field_name` - Name of the field to test
    /// * `test_value` - The test value to set in the field (will be JSON serialized)
    /// * `accessor` - Function to extract the field value from a request
    fn test_optional_field<T, F>(field_name: &str, test_value: serde_json::Value, accessor: F)
    where
        T: PartialEq + std::fmt::Debug,
        F: Fn(&RuleCheckRequest) -> Option<T>,
    {
        // Test with field present
        let args_with = json!({
            "rule_names": ["test-rule"],
            field_name: test_value
        });
        let request_with: RuleCheckRequest = serde_json::from_value(args_with).unwrap();
        assert!(
            accessor(&request_with).is_some(),
            "Field '{}' should parse to Some(_)",
            field_name
        );

        // Test without field (should default to None)
        let args_without = json!({
            "rule_names": ["test-rule"]
        });
        let request_without: RuleCheckRequest = serde_json::from_value(args_without).unwrap();
        assert_eq!(
            accessor(&request_without),
            None,
            "Field '{}' should default to None",
            field_name
        );
    }

    /// Generic test helper for optional boolean fields
    ///
    /// Tests that a boolean field correctly parses to Some(true) when present
    /// and None when absent. This is a convenience wrapper around test_optional_field.
    ///
    /// # Arguments
    ///
    /// * `field_name` - Name of the field to test
    /// * `accessor` - Function to extract the field value from a request
    fn test_optional_bool_field<F>(field_name: &str, accessor: F)
    where
        F: Fn(&RuleCheckRequest) -> Option<bool>,
    {
        test_optional_field(field_name, json!(true), accessor);
    }

    /// Parameterized test cases for optional boolean field validation
    ///
    /// Uses a data-driven approach with a generic test helper to eliminate
    /// repetitive assertion patterns across multiple boolean fields.
    #[tokio::test]
    async fn test_rule_check_request_optional_bool_fields() {
        test_optional_bool_field("changed", |r| r.changed);
        test_optional_bool_field("create_todo", |r| r.create_todo);
    }

    /// Test optional numeric fields using the generic helper
    #[tokio::test]
    async fn test_rule_check_request_optional_numeric_fields() {
        test_optional_field("max_errors", json!(10), |r: &RuleCheckRequest| r.max_errors);
        test_optional_field("max_concurrency", json!(8), |r: &RuleCheckRequest| {
            r.max_concurrency
        });
    }

    /// Test optional string and array fields using the generic helper
    #[tokio::test]
    async fn test_rule_check_request_optional_string_fields() {
        test_optional_field("category", json!("safety"), |r: &RuleCheckRequest| {
            r.category.clone()
        });
        test_optional_field("severity", json!("error"), |r: &RuleCheckRequest| {
            r.severity
        });
    }

    /// Test expand_glob_patterns helper function
    #[tokio::test]
    async fn test_expand_glob_patterns() {
        let temp_dir = TempDir::new().unwrap();

        let rs_file = temp_dir.path().join("test.rs");
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&rs_file, "fn main() {}").unwrap();
        fs::write(&txt_file, "hello").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string()];
        let result = expand_glob_patterns(&patterns).await;

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|f| f.ends_with("test.rs")));
    }

    /// Test that create_todo parameter creates todos for violations
    #[tokio::test]
    async fn test_rule_check_with_create_todo() {
        let todo_dir = TempDir::new().unwrap();
        let _guard = EnvVarGuard::set(
            "SWISSARMYHAMMER_TODO_DIR",
            todo_dir.path().to_string_lossy().to_string(),
        );

        let test_code = r#"
// TODO: This is a todo comment that should trigger a violation
fn example() {
    let x = vec![1, 2, 3];
    println!("x: {:?}", x);
}
"#;

        let additional_args = TestArgsBuilder::new().with_create_todo(true).build();

        setup_execute_and_assert(
            test_code,
            vec!["code-quality/no-todo-comments"],
            Some(additional_args),
            &["violation", "todos created"],
            "Should have found violations and mentioned todo creation",
            false,
        )
        .await;

        let todo_file = todo_dir.path().join("todo.yaml");
        if todo_file.exists() {
            let todo_content = fs::read_to_string(&todo_file).unwrap();
            println!("Todo file content:\n{}", todo_content);

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

    /// Test that changed files integration returns early when no changed files
    #[tokio::test]
    async fn test_rule_check_with_changed_no_files() {
        with_test_git_repo(
            "test.rs",
            "fn main() {}",
            |context, _repo_path| async move {
                let tool = RuleCheckTool::new();

                let arguments = TestArgsBuilder::new().with_changed(true).build();

                let result = tool.execute(arguments, &context).await;
                assert_result_contains(
                    result,
                    &["No changed files"],
                    "Should report no changed files",
                );
            },
        )
        .await;
    }

    /// Test that rule check respects .gitignore files
    #[tokio::test]
    async fn test_rule_check_respects_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        let repo = git2::Repository::init(repo_path).unwrap();
        GitTestHelper::configure_user(&repo).unwrap();

        // Create .gitignore file
        fs::write(repo_path.join(".gitignore"), "ignored.rs\n").unwrap();

        // Create files - one that should be ignored, one that shouldn't
        fs::write(
            repo_path.join("included.rs"),
            "// TODO: This should be found\nfn main() {}",
        )
        .unwrap();
        fs::write(
            repo_path.join("ignored.rs"),
            "// TODO: This should be ignored\nfn test() {}",
        )
        .unwrap();

        // Commit everything so git tracks .gitignore
        GitTestHelper::add_and_commit(&repo, "Initial commit").unwrap();

        // Create context with git operations
        let context = create_git_context(repo_path).await;

        // Run rule check with pattern that matches both files
        let tool = RuleCheckTool::new();
        let arguments = TestArgsBuilder::new()
            .with_rule_names(vec!["code-quality/no-todo-comments"])
            .with_file_paths(vec![format!("{}/**/*.rs", repo_path.display())])
            .build();

        let result = tool.execute(arguments, &context).await;

        // Get the result text
        let result_text = match result {
            Ok(call_result) => format!("{:?}", call_result),
            Err(e) => panic!("Tool execution failed: {}", e),
        };

        println!("Test result: {}", result_text);

        // Should find violations in included.rs but not in ignored.rs
        assert!(
            result_text.contains("included.rs") || result_text.contains("No rule violations"),
            "Should check included.rs"
        );
        assert!(
            !result_text.contains("ignored.rs"),
            "Should NOT check ignored.rs (it's in .gitignore)"
        );
    }
}
