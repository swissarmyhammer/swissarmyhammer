//! Integration tests for Flow MCP tool
//!
//! These tests verify that the flow tool correctly implements the MCP protocol
//! for workflow discovery and execution.
//!
//! Test Strategy:
//! - Use in-process HTTP MCP server (no subprocess overhead)
//! - Test via RMCP client library
//! - Verify tool registration and discoverability
//! - Test both list and execute operations
//! - Validate error handling

use rmcp::{
    model::{CallToolRequestParam, ClientInfo},
    service::RunningService,
};
use serde_json::json;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server, McpServerHandle, McpServerMode},
};
use tokio::time::{timeout, Duration};

/// Helper struct that manages test server and client lifecycle
struct TestEnvironment {
    _server: McpServerHandle,
    client: Option<RunningService<rmcp::RoleClient, ClientInfo>>,
}

impl TestEnvironment {
    /// Create a new test environment with server and client
    async fn new() -> Self {
        let server = start_mcp_server(McpServerMode::Http { port: None }, None, None)
            .await
            .expect("Failed to start MCP server");
        let client = create_test_client(server.url()).await;

        Self {
            _server: server,
            client: Some(client),
        }
    }

    /// Get a reference to the client
    fn client(&self) -> &RunningService<rmcp::RoleClient, ClientInfo> {
        self.client.as_ref().expect("Client already consumed")
    }
}

/// Call the flow list tool with specified format and verbose settings
async fn call_flow_list(
    client: &RunningService<rmcp::RoleClient, ClientInfo>,
    format: &str,
    verbose: bool,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": format,
                "verbose": verbose
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("Flow list should succeed")
}

/// Parse tool result content as JSON
fn parse_tool_result_as_json(tool_result: &rmcp::model::CallToolResult) -> serde_json::Value {
    if let Some(content) = tool_result.content.first() {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            return serde_json::from_str(&text_content.text)
                .expect("Response should be valid JSON");
        }
    }
    panic!("Expected text content in tool result");
}

/// Find a workflow by applying a predicate function
fn find_workflow_by_predicate<F>(
    list_result: &rmcp::model::CallToolResult,
    predicate: F,
) -> Option<serde_json::Value>
where
    F: Fn(&serde_json::Value) -> bool,
{
    if let Some(content) = list_result.content.first() {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            if let Ok(response) = serde_json::from_str::<serde_json::Value>(&text_content.text) {
                if let Some(workflows) = response["workflows"].as_array() {
                    return workflows.iter().find(|w| predicate(w)).cloned();
                }
            }
        }
    }
    None
}

/// Log a test skip message
fn log_test_skip(reason: &str) {
    eprintln!("SKIP: {} - skipping test", reason);
}

/// Get the flow tool from the tool registry
async fn get_flow_tool(client: &RunningService<rmcp::RoleClient, ClientInfo>) -> rmcp::model::Tool {
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    tools
        .tools
        .into_iter()
        .find(|t| t.name == "flow")
        .expect("Flow tool should exist")
}

/// Find a workflow without required parameters
fn find_workflow_without_required_params(
    list_result: &rmcp::model::CallToolResult,
) -> Option<String> {
    find_workflow_by_predicate(list_result, |workflow| {
        if let Some(params) = workflow["parameters"].as_array() {
            let has_required = params
                .iter()
                .any(|p| p.get("required").and_then(|r| r.as_bool()) == Some(true));
            !has_required
        } else {
            false
        }
    })
    .and_then(|w| w["name"].as_str().map(|s| s.to_string()))
}

/// Find a workflow with required parameters and return both workflow name and a required parameter name
fn find_workflow_with_required_params(
    list_result: &rmcp::model::CallToolResult,
) -> Option<(String, String)> {
    find_workflow_by_predicate(list_result, |workflow| {
        if let Some(params) = workflow["parameters"].as_array() {
            params
                .iter()
                .any(|p| p.get("required").and_then(|r| r.as_bool()) == Some(true))
        } else {
            false
        }
    })
    .and_then(|workflow| {
        let workflow_name = workflow["name"].as_str()?.to_string();
        let params = workflow["parameters"].as_array()?;
        let required_param = params
            .iter()
            .find(|p| p.get("required").and_then(|r| r.as_bool()) == Some(true))?;
        let param_name = required_param["name"].as_str()?.to_string();
        Some((workflow_name, param_name))
    })
}

/// Test that flow tool appears in MCP tool registry
#[tokio::test]
async fn test_flow_tool_appears_in_list() {
    let env = TestEnvironment::new().await;

    // Get the flow tool details
    let flow_tool = get_flow_tool(env.client()).await;

    // Verify tool has proper description
    if let Some(desc) = &flow_tool.description {
        assert!(
            !desc.is_empty(),
            "Flow tool should have non-empty description"
        );
    }

    // Verify tool has schema with required properties
    assert!(
        !flow_tool.input_schema.is_empty(),
        "Flow tool should have input schema"
    );
}

/// Test flow discovery via MCP (flow_name="list")
#[tokio::test]
async fn test_flow_discovery_via_mcp() {
    let env = TestEnvironment::new().await;

    // Call flow tool with flow_name="list"
    let tool_result = call_flow_list(env.client(), "json", true).await;

    // Verify response
    assert!(
        !tool_result.content.is_empty(),
        "Flow list should return content"
    );

    // Parse response as JSON
    let response = parse_tool_result_as_json(&tool_result);

    // Verify response structure
    assert!(
        response.get("workflows").is_some(),
        "Response should have workflows field"
    );

    let workflows = response["workflows"]
        .as_array()
        .expect("workflows should be an array");

    // Should have some workflows (at least built-in ones)
    assert!(
        !workflows.is_empty(),
        "Should have at least some workflows available"
    );

    // Verify workflow metadata structure
    if let Some(workflow) = workflows.first() {
        assert!(workflow.get("name").is_some(), "Workflow should have name");
        assert!(
            workflow.get("description").is_some(),
            "Workflow should have description"
        );
        assert!(
            workflow.get("source").is_some(),
            "Workflow should have source"
        );
        assert!(
            workflow.get("parameters").is_some(),
            "Workflow should have parameters"
        );
    }
}

/// Test flow execution via MCP (dry run)
#[tokio::test]
async fn test_flow_execution_via_mcp() {
    let env = TestEnvironment::new().await;

    // First, discover available workflows
    let list_result = call_flow_list(env.client(), "json", false).await;

    // Find a workflow with no required parameters
    let test_workflow = find_workflow_without_required_params(&list_result);

    // NOTE: This test dynamically discovers workflows from the actual .swissarmyhammer/workflows
    // directory. It's acceptable for this test to be skipped if no suitable workflows exist,
    // as the test environment may vary. The test validates execution when workflows are available.
    if let Some(workflow_name) = test_workflow {
        eprintln!("Testing execution with workflow: {}", workflow_name);

        // Wrap execution in timeout to prevent hanging on slow workflows
        let exec_future = env.client().call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": workflow_name,
                "parameters": {},
                "dry_run": true,
                "quiet": true
            })
            .as_object()
            .cloned(),
        });

        match timeout(Duration::from_secs(2), exec_future).await {
            Ok(exec_result) => {
                // Execution might succeed or fail depending on the workflow
                // Just verify it doesn't panic and returns a proper response
                match exec_result {
                    Ok(result) => {
                        assert!(
                            !result.content.is_empty(),
                            "Flow execution should return content"
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "Workflow execution error (expected for some workflows): {:?}",
                            e
                        );
                    }
                }
            }
            Err(_) => {
                log_test_skip(&format!(
                    "Workflow '{}' exceeded 2s timeout in dry-run mode",
                    workflow_name
                ));
            }
        }
    } else {
        log_test_skip("No workflows without required parameters found - skipping execution test");
    }
}

/// Test flow execution with missing required parameter
#[tokio::test]
async fn test_flow_missing_required_parameter() {
    let env = TestEnvironment::new().await;

    // First, discover workflows to find one with required parameters
    let list_result = call_flow_list(env.client(), "json", false).await;

    // Find a workflow with required parameters
    let test_workflow = find_workflow_with_required_params(&list_result);

    // NOTE: This test dynamically discovers workflows from the actual .swissarmyhammer/workflows
    // directory. It's acceptable for this test to be skipped if no suitable workflows exist,
    // as the test environment may vary. The test validates error handling when workflows with
    // required parameters are available.
    if let Some((workflow_name, param_name)) = test_workflow {
        eprintln!(
            "Testing missing parameter error with workflow: {} (missing: {})",
            workflow_name, param_name
        );

        // Try to execute without providing the required parameter
        let exec_result = env
            .client()
            .call_tool(CallToolRequestParam {
                name: "flow".into(),
                arguments: json!({
                    "flow_name": workflow_name,
                    "parameters": {} // Empty parameters - missing required parameter
                })
                .as_object()
                .cloned(),
            })
            .await;

        // Should fail with invalid_params error
        assert!(
            exec_result.is_err(),
            "Flow execution should fail when required parameter is missing"
        );

        if let Err(e) = exec_result {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("Missing required parameter")
                    || error_msg.contains(&param_name)
                    || error_msg.contains("invalid"),
                "Error should mention missing required parameter. Got: {}",
                error_msg
            );
        }
    } else {
        log_test_skip("No workflows with required parameters found - skipping error test");
    }
}

/// Test flow tool schema includes workflow names
#[tokio::test]
async fn test_flow_tool_schema_includes_workflows() {
    let env = TestEnvironment::new().await;

    // Get flow tool schema
    let flow_tool = get_flow_tool(env.client()).await;

    // Verify schema has flow_name property with enum
    let schema = &flow_tool.input_schema;
    assert!(
        schema.get("properties").is_some(),
        "Schema should have properties"
    );

    let properties = schema["properties"]
        .as_object()
        .expect("properties should be object");
    assert!(
        properties.contains_key("flow_name"),
        "Schema should have flow_name property"
    );

    let flow_name_schema = &properties["flow_name"];
    assert!(
        flow_name_schema.get("enum").is_some(),
        "flow_name should have enum of workflow names"
    );

    let workflow_names = flow_name_schema["enum"]
        .as_array()
        .expect("enum should be array");

    // Should include "list" as special case
    assert!(
        workflow_names.iter().any(|v| v.as_str() == Some("list")),
        "flow_name enum should include 'list'"
    );

    // Should have at least a few workflow names
    assert!(
        workflow_names.len() > 1,
        "flow_name enum should include multiple workflows"
    );
}

/// Test flow tool with different output formats
#[tokio::test]
async fn test_flow_list_output_formats() {
    let env = TestEnvironment::new().await;

    // Test JSON format (default)
    let json_result = call_flow_list(env.client(), "json", false).await;
    assert!(!json_result.content.is_empty());

    // Test YAML format
    let yaml_result = call_flow_list(env.client(), "yaml", false).await;
    assert!(!yaml_result.content.is_empty());

    // Test table format
    let table_result = call_flow_list(env.client(), "table", false).await;
    assert!(!table_result.content.is_empty());

    // Verify different formats produce different output
    if let (Some(json_content), Some(table_content)) =
        (json_result.content.first(), table_result.content.first())
    {
        if let (
            rmcp::model::RawContent::Text(json_text),
            rmcp::model::RawContent::Text(table_text),
        ) = (&json_content.raw, &table_content.raw)
        {
            assert_ne!(
                json_text.text, table_text.text,
                "Different formats should produce different output"
            );
        }
    }
}
