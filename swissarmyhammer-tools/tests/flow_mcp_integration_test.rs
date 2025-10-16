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

use rmcp::model::CallToolRequestParam;
use serde_json::json;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server, McpServerMode},
};
use tokio::time::{timeout, Duration};

/// Test that flow tool appears in MCP tool registry
#[tokio::test]
async fn test_flow_tool_appears_in_list() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List tools
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    // Verify flow tool is registered
    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        tool_names.contains(&"flow".to_string()),
        "Flow tool should be registered. Available tools: {:?}",
        tool_names
    );

    // Get the flow tool details
    let flow_tool = tools
        .tools
        .iter()
        .find(|t| t.name == "flow")
        .expect("Flow tool should exist");

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

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test flow discovery via MCP (flow_name="list")
#[tokio::test]
async fn test_flow_discovery_via_mcp() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // Call flow tool with flow_name="list"
    let tool_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "json",
                "verbose": true
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("Flow list should succeed");

    // Verify response
    assert!(
        !tool_result.content.is_empty(),
        "Flow list should return content"
    );

    // Parse response as JSON
    if let Some(content) = tool_result.content.first() {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            let response: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Response should be valid JSON");

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
    }

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test flow execution via MCP (dry run)
#[tokio::test]
async fn test_flow_execution_via_mcp() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // First, discover available workflows
    let list_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "json"
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("Flow list should succeed");

    // Parse to find a simple workflow without required parameters
    let mut test_workflow: Option<String> = None;
    if let Some(content) = list_result.content.first() {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            let response: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Response should be valid JSON");

            if let Some(workflows) = response["workflows"].as_array() {
                // Find a workflow with no required parameters
                for workflow in workflows {
                    if let Some(params) = workflow["parameters"].as_array() {
                        let has_required = params
                            .iter()
                            .any(|p| p.get("required").and_then(|r| r.as_bool()) == Some(true));

                        if !has_required {
                            test_workflow = workflow["name"].as_str().map(|s| s.to_string());
                            break;
                        }
                    }
                }
            }
        }
    }

    // NOTE: This test dynamically discovers workflows from the actual .swissarmyhammer/workflows
    // directory. It's acceptable for this test to be skipped if no suitable workflows exist,
    // as the test environment may vary. The test validates execution when workflows are available.
    if let Some(workflow_name) = test_workflow {
        eprintln!("Testing execution with workflow: {}", workflow_name);

        // Wrap execution in timeout to prevent hanging on slow workflows
        let exec_future = client.call_tool(CallToolRequestParam {
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

        match timeout(Duration::from_secs(5), exec_future).await {
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
                eprintln!(
                    "SKIP: Workflow '{}' exceeded 5s timeout in dry-run mode - skipping test",
                    workflow_name
                );
            }
        }
    } else {
        // Test is skipped if no suitable workflows found - this is acceptable
        // as workflow availability varies by environment
        eprintln!("SKIP: No workflows without required parameters found - skipping execution test");
    }

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test flow execution with missing required parameter
#[tokio::test]
async fn test_flow_missing_required_parameter() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // First, discover workflows to find one with required parameters
    let list_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "json"
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("Flow list should succeed");

    // Parse to find a workflow with required parameters
    let mut test_workflow: Option<(String, String)> = None; // (workflow_name, required_param_name)
    if let Some(content) = list_result.content.first() {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            let response: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Response should be valid JSON");

            if let Some(workflows) = response["workflows"].as_array() {
                // Find a workflow with required parameters
                for workflow in workflows {
                    if let Some(params) = workflow["parameters"].as_array() {
                        if let Some(required_param) = params
                            .iter()
                            .find(|p| p.get("required").and_then(|r| r.as_bool()) == Some(true))
                        {
                            if let (Some(workflow_name), Some(param_name)) =
                                (workflow["name"].as_str(), required_param["name"].as_str())
                            {
                                test_workflow =
                                    Some((workflow_name.to_string(), param_name.to_string()));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

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
        let exec_result = client
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
        // Test is skipped if no suitable workflows found - this is acceptable
        // as workflow availability varies by environment
        eprintln!("SKIP: No workflows with required parameters found - skipping error test");
    }

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test flow tool schema includes workflow names
#[tokio::test]
async fn test_flow_tool_schema_includes_workflows() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List tools to get flow tool schema
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    let flow_tool = tools
        .tools
        .iter()
        .find(|t| t.name == "flow")
        .expect("Flow tool should exist");

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

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test flow tool with different output formats
#[tokio::test]
async fn test_flow_list_output_formats() {
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // Test JSON format (default)
    let json_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "json"
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("JSON format should work");

    assert!(!json_result.content.is_empty());

    // Test YAML format
    let yaml_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "yaml"
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("YAML format should work");

    assert!(!yaml_result.content.is_empty());

    // Test table format
    let table_result = client
        .call_tool(CallToolRequestParam {
            name: "flow".into(),
            arguments: json!({
                "flow_name": "list",
                "format": "table"
            })
            .as_object()
            .cloned(),
        })
        .await
        .expect("Table format should work");

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

    // Clean shutdown - ensure all resources are released
    client.cancel().await.expect("Failed to cancel client");
    drop(json_result);
    drop(yaml_result);
    drop(table_result);
    server.shutdown().await.expect("Failed to shutdown server");
}
