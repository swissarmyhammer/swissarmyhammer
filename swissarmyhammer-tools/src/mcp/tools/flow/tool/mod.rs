//! sah rule ignore test_rule_with_allow
//!
//! Flow tool for MCP operations
//!
//! This module provides the FlowTool for executing workflows or listing available workflows
//! through the MCP protocol.

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::flow::types::*;
use async_trait::async_trait;
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::{generate_monotonic_ulid_string, Pretty};
use swissarmyhammer_config::AgentUseCase;
use swissarmyhammer_workflow::{MemoryWorkflowStorage, WorkflowResolver, WorkflowStorageBackend};

/// Validate that all required parameters are provided
///
/// # Arguments
///
/// * `workflow` - The workflow definition with parameter requirements
/// * `provided_params` - The parameters provided in the request
///
/// # Returns
///
/// Ok if all required parameters are present, Err with description if any are missing
fn validate_required_parameters(
    workflow: &swissarmyhammer_workflow::Workflow,
    provided_params: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    for param in &workflow.parameters {
        if param.required && !provided_params.contains_key(&param.name) {
            return Err(format!(
                "Missing required parameter: '{}'. Description: {}",
                param.name, param.description
            ));
        }
    }
    Ok(())
}

/// Tool for executing workflows or listing available workflows
///
/// The FlowTool provides a unified interface for workflow operations:
/// - When flow_name="list", returns metadata about available workflows
/// - Otherwise, executes the specified workflow with provided parameters
#[derive(Default)]
pub struct FlowTool;

impl FlowTool {
    /// Creates a new instance of the FlowTool
    pub fn new() -> Self {
        Self
    }

    /// Load all workflows using the resolver
    fn load_workflows(&self) -> Result<(MemoryWorkflowStorage, WorkflowResolver), String> {
        let mut storage = MemoryWorkflowStorage::new();
        let mut resolver = WorkflowResolver::new();

        resolver
            .load_all_workflows(&mut storage)
            .map_err(|e| format!("Failed to load workflows: {}", e))?;

        Ok((storage, resolver))
    }

    /// Convert workflows to metadata format
    ///
    /// # Arguments
    ///
    /// * `workflows` - List of workflows to convert
    /// * `resolver` - Workflow resolver containing source information
    ///
    /// # Returns
    ///
    /// Vector of workflow metadata
    fn convert_to_metadata(
        workflows: &[swissarmyhammer_workflow::Workflow],
        resolver: &WorkflowResolver,
    ) -> Vec<WorkflowMetadata> {
        workflows
            .iter()
            .map(|w| {
                let source = resolver
                    .workflow_sources
                    .get(&w.name)
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unknown".to_string());

                let parameters: Vec<WorkflowParameter> = w
                    .parameters
                    .iter()
                    .map(|p| WorkflowParameter {
                        name: p.name.clone(),
                        param_type: format!("{:?}", p.parameter_type).to_lowercase(),
                        description: p.description.clone(),
                        required: p.required,
                    })
                    .collect();

                WorkflowMetadata {
                    name: w.name.to_string(),
                    description: w.description.clone(),
                    source,
                    parameters,
                }
            })
            .collect()
    }

    /// Format workflow list response according to requested format
    ///
    /// # Arguments
    ///
    /// * `response` - Workflow list response to format
    /// * `format` - Optional format specifier (yaml, table, or json)
    ///
    /// # Returns
    ///
    /// Formatted string or error
    fn format_workflow_list(
        response: &WorkflowListResponse,
        format: Option<&str>,
    ) -> Result<String, McpError> {
        match format {
            Some("yaml") => serde_yaml::to_string(&response).map_err(|e| {
                McpError::internal_error(format!("YAML serialization error: {}", e), None)
            }),
            Some("table") => Ok(format_table(response)),
            _ => serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("JSON serialization error: {}", e), None)
            }),
        }
    }

    /// List available workflows
    async fn list_workflows(
        &self,
        request: &FlowToolRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        let (storage, resolver) = self
            .load_workflows()
            .map_err(|e| McpError::internal_error(e, None))?;

        let workflows = storage.list_workflows().map_err(|e| {
            McpError::internal_error(format!("Failed to list workflows: {}", e), None)
        })?;

        let metadata = Self::convert_to_metadata(&workflows, &resolver);
        let response = WorkflowListResponse {
            workflows: metadata,
        };

        let formatted = Self::format_workflow_list(&response, request.format.as_deref())?;

        Ok(BaseToolImpl::create_success_response(formatted))
    }

    /// Load and validate a workflow
    ///
    /// # Arguments
    ///
    /// * `workflow_name` - Name of the workflow to load
    /// * `parameters` - Parameters to validate against workflow requirements
    ///
    /// # Returns
    ///
    /// Ok with the loaded workflow, or Err with MCP error
    fn load_and_validate_workflow(
        &self,
        workflow_name: &str,
        parameters: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<swissarmyhammer_workflow::Workflow, McpError> {
        let (storage, _resolver) = self.load_workflows().map_err(|e| {
            tracing::error!("❌ Failed to load workflows: {}", e);
            McpError::internal_error(e, None)
        })?;

        tracing::info!("📚 Workflows loaded successfully");

        let wf_name = swissarmyhammer_workflow::WorkflowName::new(workflow_name.to_string());
        let workflow = storage.get_workflow(&wf_name).map_err(|e| {
            tracing::error!("❌ Failed to get workflow '{}': {}", workflow_name, e);
            McpError::internal_error(
                format!("Failed to load workflow '{}': {}", workflow_name, e),
                None,
            )
        })?;

        tracing::info!("✅ Workflow '{}' loaded", workflow_name);

        validate_required_parameters(&workflow, parameters)
            .map_err(|e| McpError::invalid_params(e, None))?;

        Ok(workflow)
    }

    /// Setup workflow run context with parameters and configuration
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to setup
    /// * `context` - Tool context with agent and configuration
    /// * `request` - Request containing parameters
    /// * `run_id` - Unique run identifier
    ///
    /// # Returns
    ///
    /// Tuple of (executor, workflow_run) ready for execution
    async fn setup_workflow_run(
        &self,
        workflow: swissarmyhammer_workflow::Workflow,
        context: &ToolContext,
        request: &FlowToolRequest,
        run_id: &str,
    ) -> Result<
        (
            swissarmyhammer_workflow::WorkflowExecutor,
            swissarmyhammer_workflow::WorkflowRun,
        ),
        McpError,
    > {
        let workflows_agent = context.get_agent_for_use_case(AgentUseCase::Workflows);
        tracing::debug!(
            "Using agent for Workflows use case: {:?}",
            workflows_agent.executor_type()
        );

        let working_dir = context
            .working_dir
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("."));

        let mut executor = swissarmyhammer_workflow::WorkflowExecutor::with_working_dir_and_agent(
            working_dir,
            workflows_agent,
        );

        let mut run = executor.start_workflow(workflow).map_err(|e| {
            McpError::internal_error(format!("Failed to start workflow: {}", e), None)
        })?;

        run.context
            .set_workflow_var("__run_id__".to_string(), serde_json::json!(run_id));

        {
            let port_lock = context.mcp_server_port.read().await;
            if let Some(port) = *port_lock {
                run.context.insert(
                    "_mcp_server_port".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(port)),
                );
                tracing::debug!("Set _mcp_server_port={} in workflow context", port);
            }
        }

        // Resolve parameters including defaults with environment variable substitution
        use swissarmyhammer_common::parameters::{DefaultParameterResolver, ParameterResolver};
        let resolver = DefaultParameterResolver::new();
        let cli_args: std::collections::HashMap<String, String> = request
            .parameters
            .iter()
            .map(|(k, v)| {
                // Extract the string value without JSON formatting
                let value_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => String::new(),
                    // For arrays and objects, use JSON representation
                    _ => v.to_string(),
                };
                (k.clone(), value_str)
            })
            .collect();

        match resolver.resolve_parameters(&run.workflow.parameters, &cli_args, false) {
            Ok(resolved_params) => {
                tracing::debug!("Resolved {} workflow parameters", resolved_params.len());
                for (key, value) in resolved_params {
                    tracing::trace!("Setting workflow var: {} = {}", key, Pretty(&value));
                    run.context.set_workflow_var(key, value);
                }
            }
            Err(e) => {
                return Err(McpError::invalid_params(
                    format!("Failed to resolve parameters: {}", e),
                    None,
                ));
            }
        }

        Ok((executor, run))
    }

    /// Send an MCP LoggingMessageNotification via the shared helper
    async fn send_flow_log(
        context: &ToolContext,
        level: LoggingLevel,
        flow_name: &str,
        message: String,
    ) {
        tracing::info!("📨 MCP log [{}]: {}", flow_name, message);
        send_mcp_log(context, level, &format!("flow:{}", flow_name), message).await;
    }

    /// Send flow start notification via MCP peer and internal channel
    async fn send_flow_start_notification(
        context: &ToolContext,
        run_id: &str,
        flow_name: &str,
        parameters: serde_json::Value,
        initial_state: &str,
    ) {
        Self::send_flow_log(
            context,
            LoggingLevel::Info,
            flow_name,
            format!("Starting flow at state '{}'", initial_state),
        )
        .await;
        if let Some(sender) = &context.notification_sender {
            let _ = sender.send_flow_start(run_id, flow_name, parameters, initial_state);
        }
    }

    /// Send flow complete notification via MCP peer and internal channel
    async fn send_flow_complete_notification(
        context: &ToolContext,
        run_id: &str,
        flow_name: &str,
        status: &str,
        final_state: &str,
    ) {
        Self::send_flow_log(
            context,
            LoggingLevel::Info,
            flow_name,
            format!(
                "Completed with status '{}' at state '{}'",
                status, final_state
            ),
        )
        .await;
        if let Some(sender) = &context.notification_sender {
            let _ = sender.send_flow_complete(run_id, flow_name, status, final_state);
        }
    }

    /// Send flow error notification via MCP peer and internal channel
    async fn send_flow_error_notification(
        context: &ToolContext,
        run_id: &str,
        flow_name: &str,
        status: &str,
        error_state: &str,
        error_message: &str,
    ) {
        Self::send_flow_log(
            context,
            LoggingLevel::Error,
            flow_name,
            format!("Error at state '{}': {}", error_state, error_message),
        )
        .await;
        if let Some(sender) = &context.notification_sender {
            let _ = sender.send_flow_error(run_id, flow_name, status, error_state, error_message);
        }
    }

    /// Execute the workflow and handle the result
    ///
    /// # Arguments
    ///
    /// * `executor` - Workflow executor
    /// * `run` - Workflow run to execute
    /// * `run_id` - Unique run identifier
    /// * `flow_name` - Name of the workflow
    /// * `context` - Tool context with notification sender
    ///
    /// # Returns
    ///
    /// Success response with workflow status, or error
    async fn run_and_handle_result(
        &self,
        mut executor: swissarmyhammer_workflow::WorkflowExecutor,
        mut run: swissarmyhammer_workflow::WorkflowRun,
        run_id: &str,
        flow_name: &str,
        context: &ToolContext,
        log_rx: tokio::sync::mpsc::UnboundedReceiver<swissarmyhammer_workflow::LogMessage>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let result = self
            .execute_with_notifications(&mut executor, &mut run, run_id, flow_name, context, log_rx)
            .await;

        match result {
            Ok(()) => {
                Self::send_flow_complete_notification(
                    context,
                    run_id,
                    flow_name,
                    &format!("{:?}", run.status),
                    run.current_state.as_str(),
                )
                .await;

                let output = serde_json::json!({
                    "status": "completed",
                    "workflow": flow_name,
                    "final_status": format!("{:?}", run.status),
                });
                let formatted_output = serde_json::to_string_pretty(&output).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize result: {}", e), None)
                })?;
                Ok(BaseToolImpl::create_success_response(formatted_output))
            }
            Err(e) => {
                Self::send_flow_error_notification(
                    context,
                    run_id,
                    flow_name,
                    &format!("{:?}", run.status),
                    run.current_state.as_str(),
                    &e.to_string(),
                )
                .await;

                Err(McpError::internal_error(
                    format!("Workflow '{}' execution failed: {}", flow_name, e),
                    None,
                ))
            }
        }
    }

    /// Execute a workflow
    ///
    /// # Limitations
    ///
    /// The `interactive`, `dry_run`, and `quiet` flags in the request are not currently
    /// passed to the WorkflowExecutor due to API limitations. These will be implemented
    /// in a future enhancement when the WorkflowExecutor API supports them.
    async fn execute_workflow(
        &self,
        request: &FlowToolRequest,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("🎯 execute_workflow STARTED for '{}'", request.flow_name);

        let workflow = self.load_and_validate_workflow(&request.flow_name, &request.parameters)?;

        let run_id = generate_monotonic_ulid_string();

        Self::send_flow_start_notification(
            context,
            &run_id,
            &request.flow_name,
            serde_json::to_value(&request.parameters).unwrap_or(serde_json::json!({})),
            workflow.initial_state.as_str(),
        )
        .await;

        let (executor, mut run) = self
            .setup_workflow_run(workflow, context, request, &run_id)
            .await?;

        // Set up log channel so LogAction messages get forwarded as MCP notifications
        let (log_tx, log_rx) = tokio::sync::mpsc::unbounded_channel();
        run.context.set_log_sender(log_tx);

        self.run_and_handle_result(executor, run, &run_id, &request.flow_name, context, log_rx)
            .await
    }

    /// Calculate progress percentage based on executed states vs total states
    ///
    /// Progress calculation is approximate - based on executed states vs total states.
    /// May not be accurate for workflows with loops or conditional branches.
    ///
    /// # Arguments
    ///
    /// * `executed_states` - Number of states that have been executed
    /// * `total_states` - Total number of states in the workflow
    ///
    /// # Returns
    ///
    /// Progress percentage from 0-100
    fn calculate_progress(executed_states: usize, total_states: usize) -> u32 {
        if total_states > 0 {
            ((executed_states * 100) / total_states).min(100) as u32
        } else if executed_states > 0 {
            100
        } else {
            0
        }
    }

    /// Send state start notification via MCP peer and internal channel
    async fn send_state_start_notification(
        context: &ToolContext,
        run: &swissarmyhammer_workflow::WorkflowRun,
        run_id: &str,
        flow_name: &str,
        current_state: &swissarmyhammer_workflow::StateId,
        progress: u32,
    ) {
        Self::send_flow_log(
            context,
            LoggingLevel::Info,
            flow_name,
            format!("[{}/100] Entering state '{}'", progress, current_state),
        )
        .await;
        if let Some(sender) = &context.notification_sender {
            if let Some(state) = run.workflow.states.get(current_state) {
                let _ = sender.send_state_start(
                    run_id,
                    flow_name,
                    current_state.as_str(),
                    &state.description,
                    progress,
                );
            }
        }
    }

    /// Send state complete notification via MCP peer and internal channel
    async fn send_state_complete_notification(
        context: &ToolContext,
        run_id: &str,
        flow_name: &str,
        current_state: &swissarmyhammer_workflow::StateId,
        next_state: Option<&str>,
        progress: u32,
    ) {
        let message = match next_state {
            Some(next) => format!(
                "[{}/100] Completed state '{}', transitioning to '{}'",
                progress, current_state, next
            ),
            None => format!(
                "[{}/100] Completed state '{}' (no transition)",
                progress, current_state
            ),
        };
        Self::send_flow_log(context, LoggingLevel::Info, flow_name, message).await;
        if let Some(sender) = &context.notification_sender {
            let _ = sender.send_state_complete(
                run_id,
                flow_name,
                current_state.as_str(),
                next_state,
                progress,
            );
        }
    }

    /// Execute workflow with progress notifications sent at each state transition
    ///
    /// This method wraps the workflow execution loop and sends notifications via the MCP
    /// notification channel at key points: state start, state complete. Progress percentages
    /// are calculated based on executed states vs total states.
    ///
    /// # Arguments
    ///
    /// * `executor` - The workflow executor instance
    /// * `run` - The workflow run being executed
    /// * `run_id` - Unique identifier for this execution
    /// * `flow_name` - Name of the workflow being executed
    /// * `context` - Tool context containing optional notification sender
    ///
    /// # Returns
    ///
    /// Ok if workflow completes successfully, Err if execution fails
    async fn execute_with_notifications(
        &self,
        executor: &mut swissarmyhammer_workflow::WorkflowExecutor,
        run: &mut swissarmyhammer_workflow::WorkflowRun,
        run_id: &str,
        flow_name: &str,
        context: &ToolContext,
        mut log_rx: tokio::sync::mpsc::UnboundedReceiver<swissarmyhammer_workflow::LogMessage>,
    ) -> Result<(), swissarmyhammer_workflow::ExecutorError> {
        let total_states = run.workflow.states.len();
        let mut executed_states = 0;

        loop {
            let current_state = run.current_state.clone();

            let progress = Self::calculate_progress(executed_states, total_states);
            Self::send_state_start_notification(
                context,
                run,
                run_id,
                flow_name,
                &current_state,
                progress,
            )
            .await;

            let transition_performed = executor.execute_single_cycle(run).await?;
            executed_states += 1;

            // Drain log messages emitted during this cycle and forward as MCP logging notifications
            Self::drain_log_messages(context, &mut log_rx, flow_name).await;

            let next_state = if transition_performed {
                Some(run.current_state.as_str())
            } else {
                None
            };

            let progress = Self::calculate_progress(executed_states, total_states);
            Self::send_state_complete_notification(
                context,
                run_id,
                flow_name,
                &current_state,
                next_state,
                progress,
            )
            .await;

            if !transition_performed || executor.is_workflow_finished(run) {
                break;
            }
        }

        // Drain any remaining log messages
        Self::drain_log_messages(context, &mut log_rx, flow_name).await;

        Ok(())
    }

    /// Drain pending log messages from the workflow and send them as MCP logging notifications
    async fn drain_log_messages(
        context: &ToolContext,
        log_rx: &mut tokio::sync::mpsc::UnboundedReceiver<swissarmyhammer_workflow::LogMessage>,
        flow_name: &str,
    ) {
        while let Ok(log_msg) = log_rx.try_recv() {
            let level = match log_msg.level.as_str() {
                "error" => LoggingLevel::Error,
                "warning" => LoggingLevel::Warning,
                _ => LoggingLevel::Info,
            };
            Self::send_flow_log(context, level, flow_name, log_msg.message).await;
        }
    }

    /// Get available workflow names
    ///
    /// # Returns
    ///
    /// Vector of workflow names, or empty vector if loading fails
    fn get_available_workflow_names(&self) -> Vec<String> {
        self.load_workflows()
            .ok()
            .and_then(|(storage, _)| storage.list_workflows().ok())
            .map(|workflows| {
                workflows
                    .iter()
                    .map(|w| w.name.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}

/// Maximum length for description in table format before truncation
const MAX_DESCRIPTION_LENGTH: usize = 47;

/// Format workflow list as a table
fn format_table(response: &WorkflowListResponse) -> String {
    let mut output = String::new();
    output.push_str("Available Workflows:\n\n");
    output.push_str(&format!(
        "{:<20} {:<50} {:<10}\n",
        "Name", "Description", "Source"
    ));
    output.push_str(&"-".repeat(82));
    output.push('\n');

    for workflow in &response.workflows {
        output.push_str(&format!(
            "{:<20} {:<50} {:<10}\n",
            workflow.name,
            if workflow.description.len() > MAX_DESCRIPTION_LENGTH {
                format!("{}...", &workflow.description[..MAX_DESCRIPTION_LENGTH])
            } else {
                workflow.description.clone()
            },
            workflow.source
        ));
    }

    output
}

impl swissarmyhammer_common::health::Doctorable for FlowTool {
    fn name(&self) -> &str {
        "Flow"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use std::collections::HashMap;
        use swissarmyhammer_common::health::HealthCheck;

        let mut checks = Vec::new();
        let resolver = WorkflowResolver::new();
        let cat = self.category();

        // Check workflow directories
        match resolver.get_workflow_directories() {
            Ok(dirs) => {
                for dir in &dirs {
                    if dir.exists() {
                        let count = walkdir::WalkDir::new(dir)
                            .into_iter()
                            .filter_map(|e| e.ok())
                            .filter(|e| e.file_type().is_file())
                            .filter(|e| {
                                e.path().extension().and_then(|s| s.to_str()) == Some("mermaid")
                            })
                            .count();
                        checks.push(HealthCheck::ok(
                            format!("Workflow directory: {}", dir.display()),
                            format!("{} workflows found", count),
                            cat,
                        ));
                    } else {
                        checks.push(HealthCheck::ok(
                            format!("Workflow directory: {}", dir.display()),
                            "Not found (optional, will be created when needed)",
                            cat,
                        ));
                    }
                }
            }
            Err(e) => {
                checks.push(HealthCheck::warning(
                    "Workflow directories",
                    format!("Could not resolve workflow directories: {}", e),
                    None,
                    cat,
                ));
            }
        }

        // Check workflow run storage directory
        if let Some(home) = dirs::home_dir() {
            let run_storage = home
                .join(swissarmyhammer_common::SwissarmyhammerDirectory::dir_name())
                .join("runs");
            if run_storage.exists() {
                checks.push(HealthCheck::ok(
                    "Workflow run storage",
                    format!("Run storage directory exists: {}", run_storage.display()),
                    cat,
                ));
            } else {
                checks.push(HealthCheck::warning(
                    "Workflow run storage",
                    format!("Run storage directory not found: {}", run_storage.display()),
                    Some(format!("Create directory: mkdir -p {:?}", run_storage)),
                    cat,
                ));
            }
        }

        // Check workflow file permissions and parsing
        if let Ok(dirs) = resolver.get_workflow_directories() {
            let mut workflow_names: HashMap<String, Vec<std::path::PathBuf>> = HashMap::new();
            let mut parse_errors = Vec::new();

            for dir in &dirs {
                if !dir.exists() {
                    continue;
                }

                // Check directory permissions
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = std::fs::metadata(dir) {
                        let mode = metadata.permissions().mode();
                        if (mode & 0o700) != 0o700 {
                            checks.push(HealthCheck::warning(
                                format!("Workflow dir permissions: {}", dir.display()),
                                format!("Permissions may be insufficient: {:o}", mode & 0o777),
                                Some(format!("Run: chmod 755 {:?}", dir)),
                                cat,
                            ));
                        }
                    }
                }

                for entry in walkdir::WalkDir::new(dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mermaid"))
                {
                    // Track names for conflict detection
                    if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        workflow_names
                            .entry(stem.to_string())
                            .or_default()
                            .push(entry.path().to_path_buf());
                    }

                    // Check readability
                    match std::fs::read_to_string(entry.path()) {
                        Ok(content) => {
                            if content.trim().is_empty() {
                                parse_errors
                                    .push(format!("{}: file is empty", entry.path().display()));
                            }
                        }
                        Err(e) => {
                            parse_errors.push(format!("{}: {}", entry.path().display(), e));
                        }
                    }
                }
            }

            // Report parsing results
            if parse_errors.is_empty() {
                checks.push(HealthCheck::ok(
                    "Workflow parsing",
                    "All workflow files are readable",
                    cat,
                ));
            } else {
                for error in parse_errors {
                    checks.push(HealthCheck::error(
                        "Workflow parsing",
                        error.clone(),
                        Some(format!("Fix or remove the workflow file: {}", error)),
                        cat,
                    ));
                }
            }

            // Report name conflicts
            let conflicts: Vec<_> = workflow_names
                .iter()
                .filter(|(_, paths)| paths.len() > 1)
                .collect();

            if conflicts.is_empty() {
                checks.push(HealthCheck::ok(
                    "Workflow name conflicts",
                    "No workflow name conflicts detected",
                    cat,
                ));
            } else {
                for (name, paths) in conflicts {
                    let locations = paths
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    checks.push(HealthCheck::warning(
                        format!("Workflow name conflict: {}", name),
                        format!("Exists in multiple locations: {}", locations),
                        Some("Rename or remove duplicate workflows".to_string()),
                        cat,
                    ));
                }
            }
        }

        checks
    }
}

#[async_trait]
impl McpTool for FlowTool {
    fn name(&self) -> &'static str {
        "flow"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let workflow_names = self.get_available_workflow_names();
        generate_flow_tool_schema(workflow_names)
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!(
            "🔧 FlowTool::execute called with arguments: {:?}",
            arguments
        );

        let request: FlowToolRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!(
            "📋 Flow tool request parsed: flow_name={}, parameters={:?}, is_list={}",
            request.flow_name,
            request.parameters,
            request.is_list()
        );

        request
            .validate()
            .map_err(|e| McpError::invalid_params(e, None))?;

        tracing::info!("✅ Request validated successfully");

        if request.is_list() {
            tracing::info!("📜 Calling list_workflows");
            self.list_workflows(&request).await
        } else {
            tracing::info!("🚀 Calling execute_workflow for '{}'", request.flow_name);
            let result = self.execute_workflow(&request, context).await;
            tracing::info!("✨ execute_workflow returned: {}", Pretty(&result.is_ok()));
            result
        }
    }

    fn cli_category(&self) -> Option<&'static str> {
        None
    }

    fn cli_name(&self) -> &'static str {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_tool_name() {
        let tool = FlowTool::new();
        assert_eq!(tool.name(), "flow");
    }

    #[test]
    fn test_flow_tool_description() {
        let tool = FlowTool::new();
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Flow"));
    }

    #[test]
    fn test_flow_tool_schema() {
        let tool = FlowTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["flow_name"].is_object());
        assert!(schema["properties"]["parameters"].is_object());
        assert!(schema["required"].is_array());

        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .expect("flow_name should have enum");
        assert!(flow_name_enum.iter().any(|v| v.as_str() == Some("list")));
    }

    #[test]
    fn test_flow_tool_cli_integration() {
        let tool = FlowTool::new();
        assert_eq!(tool.cli_category(), None);
        assert_eq!(tool.cli_name(), "");
    }

    #[tokio::test]
    async fn test_list_workflows() {
        let tool = FlowTool::new();
        let request = FlowToolRequest::list();

        let result = tool.list_workflows(&request).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_workflows_yaml_format() {
        let tool = FlowTool::new();
        let request = FlowToolRequest::list().with_format("yaml");

        let result = tool.list_workflows(&request).await;

        assert!(result.is_ok());
        if let Ok(call_result) = result {
            let content = call_result.content.first().expect("should have content");
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(text_content.text.contains("workflows"));
            } else {
                panic!("Expected text content");
            }
        }
    }

    #[tokio::test]
    async fn test_list_workflows_table_format() {
        let tool = FlowTool::new();
        let request = FlowToolRequest::list().with_format("table");

        let result = tool.list_workflows(&request).await;

        assert!(result.is_ok());
        if let Ok(call_result) = result {
            let content = call_result.content.first().expect("should have content");
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content.text.contains("Name") || text_content.text.contains("Available")
                );
            } else {
                panic!("Expected text content");
            }
        }
    }

    #[test]
    fn test_load_workflows() {
        let tool = FlowTool::new();

        let result = tool.load_workflows();
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_table() {
        let response = WorkflowListResponse {
            workflows: vec![WorkflowMetadata {
                name: "test".to_string(),
                description: "Test workflow".to_string(),
                source: "builtin".to_string(),
                parameters: vec![],
            }],
        };

        let table = format_table(&response);

        assert!(table.contains("Name"));
        assert!(table.contains("Description"));
        assert!(table.contains("Source"));
        assert!(table.contains("test"));
        assert!(table.contains("Test workflow"));
        assert!(table.contains("builtin"));
    }

    #[test]
    fn test_format_table_truncates_long_descriptions() {
        let long_desc = "a".repeat(100);
        let response = WorkflowListResponse {
            workflows: vec![WorkflowMetadata {
                name: "test".to_string(),
                description: long_desc.clone(),
                source: "builtin".to_string(),
                parameters: vec![],
            }],
        };

        let table = format_table(&response);

        assert!(table.contains("..."));
        assert!(!table.contains(&long_desc));
    }

    #[test]
    fn test_validate_required_parameters_success() {
        use swissarmyhammer_common::{Parameter, ParameterType};
        use swissarmyhammer_workflow::{StateId, Workflow, WorkflowName};

        let workflow = Workflow {
            name: WorkflowName::new("test".to_string()),
            description: "Test workflow".to_string(),
            initial_state: StateId::new("start"),
            parameters: vec![
                Parameter::new(
                    "required_param",
                    "A required parameter",
                    ParameterType::String,
                )
                .required(true),
                Parameter::new(
                    "optional_param",
                    "An optional parameter",
                    ParameterType::String,
                )
                .required(false),
            ],
            states: Default::default(),
            transitions: vec![],
            metadata: Default::default(),
            mode: None,
        };

        let mut provided_params = serde_json::Map::new();
        provided_params.insert("required_param".to_string(), serde_json::json!("value"));

        let result = validate_required_parameters(&workflow, &provided_params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_required_parameters_missing() {
        use swissarmyhammer_common::{Parameter, ParameterType};
        use swissarmyhammer_workflow::{StateId, Workflow, WorkflowName};

        let workflow = Workflow {
            name: WorkflowName::new("test".to_string()),
            description: "Test workflow".to_string(),
            initial_state: StateId::new("start"),
            parameters: vec![Parameter::new(
                "required_param",
                "A required parameter",
                ParameterType::String,
            )
            .required(true)],
            states: Default::default(),
            transitions: vec![],
            metadata: Default::default(),
            mode: None,
        };

        let provided_params = serde_json::Map::new();

        let result = validate_required_parameters(&workflow, &provided_params);
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("required_param"));
        assert!(err_msg.contains("Missing required parameter"));
    }

    #[test]
    fn test_validate_required_parameters_no_required() {
        use swissarmyhammer_workflow::{StateId, Workflow, WorkflowName};

        let workflow = Workflow {
            name: WorkflowName::new("test".to_string()),
            description: "Test workflow".to_string(),
            initial_state: StateId::new("start"),
            parameters: vec![],
            states: Default::default(),
            transitions: vec![],
            metadata: Default::default(),
            mode: None,
        };

        let provided_params = serde_json::Map::new();

        let result = validate_required_parameters(&workflow, &provided_params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_required_parameters_extra_params_allowed() {
        use swissarmyhammer_common::{Parameter, ParameterType};
        use swissarmyhammer_workflow::{StateId, Workflow, WorkflowName};

        let workflow = Workflow {
            name: WorkflowName::new("test".to_string()),
            description: "Test workflow".to_string(),
            initial_state: StateId::new("start"),
            parameters: vec![Parameter::new(
                "required_param",
                "A required parameter",
                ParameterType::String,
            )
            .required(true)],
            states: Default::default(),
            transitions: vec![],
            metadata: Default::default(),
            mode: None,
        };

        let mut provided_params = serde_json::Map::new();
        provided_params.insert("required_param".to_string(), serde_json::json!("value"));
        provided_params.insert("extra_param".to_string(), serde_json::json!("extra"));

        let result = validate_required_parameters(&workflow, &provided_params);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_workflow_nonexistent() {
        let tool = FlowTool::new();
        let request = FlowToolRequest::new("nonexistent_workflow_xyz");

        let context = crate::test_utils::create_test_context().await;

        let result = tool.execute_workflow(&request, &context).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_workflow_missing_required_params() {
        use rmcp::model::ErrorCode;

        let tool = FlowTool::new();

        let (storage, _resolver) = tool.load_workflows().expect("Failed to load workflows");
        let workflows = storage.list_workflows().expect("Failed to list workflows");

        let workflow_with_required = workflows
            .iter()
            .find(|w| w.parameters.iter().any(|p| p.required));

        if let Some(workflow) = workflow_with_required {
            let request = FlowToolRequest::new(workflow.name.to_string());

            let context = crate::test_utils::create_test_context().await;

            let result = tool.execute_workflow(&request, &context).await;

            assert!(result.is_err());
            if let Err(e) = result {
                assert_eq!(e.code, ErrorCode::INVALID_PARAMS);
                assert!(e.message.contains("Missing required parameter"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_workflow_json_output() {
        let tool = FlowTool::new();

        let (storage, _resolver) = tool.load_workflows().expect("Failed to load workflows");
        let workflows = storage.list_workflows().expect("Failed to list workflows");

        if workflows.is_empty() {
            return;
        }

        let simple_workflow = workflows.iter().find(|w| {
            let has_prompt = w.states.values().any(|state| {
                state.description.to_lowercase().contains("prompt")
                    || state.description.to_lowercase().contains("ask")
            });
            !has_prompt && w.parameters.iter().all(|p| !p.required)
        });

        if simple_workflow.is_none() {
            return;
        }

        let workflow = simple_workflow.unwrap();
        let request = FlowToolRequest::new(workflow.name.to_string());

        let context = crate::test_utils::create_test_context().await;

        let result = tool.execute_workflow(&request, &context).await;

        if let Ok(call_result) = result {
            let content = call_result.content.first().expect("should have content");
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text_content.text).expect("Should be valid JSON");
                assert!(parsed.get("status").is_some());
                assert!(parsed.get("workflow").is_some());
                assert!(parsed.get("final_status").is_some());
            } else {
                panic!("Expected text content");
            }
        }
    }

    fn find_simple_test_workflow() -> Option<swissarmyhammer_workflow::Workflow> {
        let tool = FlowTool::new();
        let (storage, _) = tool.load_workflows().ok()?;
        let workflows = storage.list_workflows().ok()?;

        workflows.into_iter().find(|w| {
            let has_prompt = w.states.values().any(|state| {
                state.description.to_lowercase().contains("prompt")
                    || state.description.to_lowercase().contains("ask")
            });
            !has_prompt && w.parameters.iter().all(|p| !p.required)
        })
    }

    async fn create_test_context_with_notifications(
        notification_sender: crate::mcp::notifications::NotificationSender,
    ) -> ToolContext {
        let mut context = crate::test_utils::create_test_context().await;
        context.notification_sender = Some(notification_sender);
        context
    }

    #[tokio::test]
    async fn test_workflow_sends_start_notification() {
        use crate::mcp::notifications::{FlowNotificationMetadata, NotificationSender};

        let workflow = match find_simple_test_workflow() {
            Some(w) => w,
            None => return,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let context = create_test_context_with_notifications(sender).await;

        let tool = FlowTool::new();
        let request = FlowToolRequest::new(workflow.name.to_string());

        let _ = tool.execute_workflow(&request, &context).await;

        let notification = rx.recv().await.expect("Should receive notification");
        match notification.metadata {
            FlowNotificationMetadata::FlowStart { flow_name, .. } => {
                assert_eq!(flow_name, workflow.name.to_string());
            }
            _ => panic!("Expected FlowStart notification, got: {:?}", notification),
        }
    }

    #[tokio::test]
    async fn test_workflow_sends_state_notifications() {
        use crate::mcp::notifications::{FlowNotificationMetadata, NotificationSender};

        let workflow = match find_simple_test_workflow() {
            Some(w) => w,
            None => return,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let context = create_test_context_with_notifications(sender).await;

        let tool = FlowTool::new();
        let request = FlowToolRequest::new(workflow.name.to_string());

        let _ = tool.execute_workflow(&request, &context).await;

        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        let state_start_count = notifications
            .iter()
            .filter(|n| matches!(n.metadata, FlowNotificationMetadata::StateStart { .. }))
            .count();

        let state_complete_count = notifications
            .iter()
            .filter(|n| matches!(n.metadata, FlowNotificationMetadata::StateComplete { .. }))
            .count();

        assert!(
            state_start_count > 0,
            "Expected at least one StateStart notification"
        );
        assert!(
            state_complete_count > 0,
            "Expected at least one StateComplete notification"
        );
    }

    #[tokio::test]
    async fn test_workflow_sends_completion_notification() {
        use crate::mcp::notifications::{FlowNotificationMetadata, NotificationSender};

        let workflow = match find_simple_test_workflow() {
            Some(w) => w,
            None => return,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let context = create_test_context_with_notifications(sender).await;

        let tool = FlowTool::new();
        let request = FlowToolRequest::new(workflow.name.to_string());

        let result = tool.execute_workflow(&request, &context).await;

        if result.is_ok() {
            let mut notifications = Vec::new();
            while let Ok(notif) = rx.try_recv() {
                notifications.push(notif);
            }

            let completion_notif = notifications
                .iter()
                .find(|n| matches!(n.metadata, FlowNotificationMetadata::FlowComplete { .. }));

            assert!(
                completion_notif.is_some(),
                "Expected FlowComplete notification on success"
            );

            if let Some(notif) = completion_notif {
                assert_eq!(notif.progress, Some(100), "Completion should be 100%");
            }
        }
    }

    #[tokio::test]
    async fn test_workflow_sends_error_notification() {
        use crate::mcp::notifications::{FlowNotificationMetadata, NotificationSender};

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let context = create_test_context_with_notifications(sender).await;

        let tool = FlowTool::new();

        let workflow = match find_simple_test_workflow() {
            Some(w) => w,
            None => return,
        };

        let request = FlowToolRequest::new(workflow.name.to_string());

        let result = tool.execute_workflow(&request, &context).await;

        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        let start_notif = notifications
            .iter()
            .find(|n| matches!(n.metadata, FlowNotificationMetadata::FlowStart { .. }));

        assert!(
            start_notif.is_some(),
            "Expected FlowStart notification even if workflow fails"
        );

        if result.is_err() {
            let error_notif = notifications
                .iter()
                .find(|n| matches!(n.metadata, FlowNotificationMetadata::FlowError { .. }));

            assert!(
                error_notif.is_some(),
                "Expected FlowError notification when workflow fails"
            );

            if let Some(notif) = error_notif {
                assert_eq!(
                    notif.progress, None,
                    "FlowError notification should have None for progress"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_progress_calculation() {
        use crate::mcp::notifications::{FlowNotificationMetadata, NotificationSender};

        let workflow = match find_simple_test_workflow() {
            Some(w) => w,
            None => return,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let context = create_test_context_with_notifications(sender).await;

        let tool = FlowTool::new();
        let request = FlowToolRequest::new(workflow.name.to_string());

        let _ = tool.execute_workflow(&request, &context).await;

        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        for notif in &notifications {
            if let Some(progress) = notif.progress {
                assert!(
                    progress <= 100,
                    "Progress should not exceed 100%, got {}",
                    progress
                );
            }
        }

        if let Some(start_notif) = notifications
            .iter()
            .find(|n| matches!(n.metadata, FlowNotificationMetadata::FlowStart { .. }))
        {
            assert_eq!(
                start_notif.progress,
                Some(0),
                "FlowStart should have 0% progress"
            );
        }
    }

    /// Regression test for string parameter quoting bug
    ///
    /// This test ensures that string parameters don't get extra quotes when
    /// converted from JSON values. Previously, using `.to_string()` on a
    /// JSON String value would produce "value" (with quotes), causing template
    /// rendering to fail with nested quotes like: log "Message "value""
    ///
    /// This test verifies the fix in the parameter resolution code that properly
    /// extracts string values without JSON formatting.
    #[tokio::test]
    async fn test_string_parameter_without_json_quotes() {
        use serde_json::json;

        // Simulate the parameter conversion that happens in setup_workflow_run
        let test_value = json!("test.md");

        // Test the conversion logic (same as in the fix)
        let value_str = match &test_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            _ => test_value.to_string(),
        };

        // Should extract the string without quotes
        assert_eq!(
            value_str, "test.md",
            "String value should not include JSON quotes"
        );

        // Verify this doesn't happen with the buggy .to_string() approach
        let buggy_conversion = test_value.to_string();
        assert_eq!(
            buggy_conversion, "\"test.md\"",
            "to_string() includes quotes (this is the bug)"
        );
        assert_ne!(
            value_str, buggy_conversion,
            "Fixed conversion should differ from buggy .to_string()"
        );
    }
}
