//! Flow tool for MCP operations
//!
//! This module provides the FlowTool for executing workflows or listing available workflows
//! through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::flow::types::*;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_workflow::{MemoryWorkflowStorage, WorkflowResolver, WorkflowStorageBackend};

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

        // Convert to metadata format
        let metadata: Vec<WorkflowMetadata> = workflows
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
            .collect();

        let response = WorkflowListResponse {
            workflows: metadata,
        };

        // Format based on request
        let formatted = match request.format.as_deref() {
            Some("yaml") => serde_yaml::to_string(&response).map_err(|e| {
                McpError::internal_error(format!("YAML serialization error: {}", e), None)
            })?,
            Some("table") => format_table(&response),
            _ => serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("JSON serialization error: {}", e), None)
            })?,
        };

        Ok(BaseToolImpl::create_success_response(formatted))
    }

    /// Execute a workflow
    async fn execute_workflow(
        &self,
        request: &FlowToolRequest,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let (storage, _resolver) = self
            .load_workflows()
            .map_err(|e| McpError::internal_error(e, None))?;

        // Get the workflow
        let workflow_name =
            swissarmyhammer_workflow::WorkflowName::new(request.flow_name.clone());
        let workflow = storage.get_workflow(&workflow_name).map_err(|e| {
            McpError::internal_error(
                format!("Failed to load workflow '{}': {}", request.flow_name, e),
                None,
            )
        })?;

        // Create workflow executor
        let mut executor = swissarmyhammer_workflow::WorkflowExecutor::new();

        // Start the workflow
        let mut run = executor.start_workflow(workflow).map_err(|e| {
            McpError::internal_error(format!("Failed to start workflow: {}", e), None)
        })?;

        // Set parameters from request into workflow context
        for (key, value) in &request.parameters {
            run.context.set_workflow_var(key.clone(), value.clone());
        }

        // Execute the workflow (execute_state uses the default MAX_TRANSITIONS internally)
        let result = executor.execute_state(&mut run).await;

        // Handle execution result
        match result {
            Ok(()) => {
                let status = run.status;
                let output = format!(
                    "Workflow '{}' completed with status: {:?}",
                    request.flow_name, status
                );
                Ok(BaseToolImpl::create_success_response(output))
            }
            Err(e) => Err(McpError::internal_error(
                format!("Workflow execution failed: {}", e),
                None,
            )),
        }
    }
}

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
            if workflow.description.len() > 47 {
                format!("{}...", &workflow.description[..47])
            } else {
                workflow.description.clone()
            },
            workflow.source
        ));
    }

    output
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
        // Load available workflows dynamically
        let workflow_names = self
            .load_workflows()
            .ok()
            .and_then(|(storage, _)| storage.list_workflows().ok())
            .map(|workflows| {
                workflows
                    .iter()
                    .map(|w| w.name.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        generate_flow_tool_schema(workflow_names)
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse request
        let request: FlowToolRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Flow tool request: flow_name={}", request.flow_name);

        // Validate the request
        request
            .validate()
            .map_err(|e| McpError::invalid_params(e, None))?;

        // Handle list vs execute
        if request.is_list() {
            self.list_workflows(&request).await
        } else {
            self.execute_workflow(&request, context).await
        }
    }

    fn cli_category(&self) -> Option<&'static str> {
        // Flow is a top-level dynamic command, not categorized
        Some("flow")
    }

    fn cli_name(&self) -> &'static str {
        // This will be the command name within the flow category
        "run"
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

        // Verify schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["flow_name"].is_object());
        assert!(schema["properties"]["parameters"].is_object());
        assert!(schema["required"].is_array());

        // Verify flow_name enum includes "list"
        let flow_name_enum = schema["properties"]["flow_name"]["enum"]
            .as_array()
            .expect("flow_name should have enum");
        assert!(flow_name_enum.iter().any(|v| v.as_str() == Some("list")));
    }

    #[test]
    fn test_flow_tool_cli_integration() {
        let tool = FlowTool::new();
        assert_eq!(tool.cli_category(), Some("flow"));
        assert_eq!(tool.cli_name(), "run");
    }

    #[tokio::test]
    async fn test_list_workflows() {
        let tool = FlowTool::new();
        let request = FlowToolRequest::list();

        let result = tool.list_workflows(&request).await;

        // Should succeed even if no workflows are found
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
            // YAML format should produce valid YAML
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
            // Table format should have headers
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

        // Should not error even if no workflows are found
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

        // Should truncate to 47 chars + "..."
        assert!(table.contains("..."));
        assert!(!table.contains(&long_desc));
    }
}
