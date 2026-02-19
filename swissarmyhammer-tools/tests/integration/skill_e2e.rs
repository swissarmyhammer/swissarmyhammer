//! End-to-end tests for the skill tool pipeline
//!
//! Tests the full pipeline: skill resolution → MCP tool registration → tool calls
//! Uses in-process HTTP MCP server + RMCP client (same pattern as rmcp_integration.rs)

use rmcp::model::CallToolRequestParam;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server_with_options, McpServerMode},
};

/// Helper to start a server and client with agent_mode setting
async fn setup(
    agent_mode: bool,
) -> (
    swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
    rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
) {
    let server =
        start_mcp_server_with_options(McpServerMode::Http { port: None }, None, None, None, agent_mode)
            .await
            .expect("Failed to start MCP server");
    let client = create_test_client(server.url()).await;
    (server, client)
}

/// Helper to teardown server and client
async fn teardown(
    mut server: swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
    client: rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
) {
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_builtin_skills_discovered_via_list() {
    let (server, client) = setup(true).await;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "skill".into(),
            arguments: serde_json::json!({"op": "list skill"})
                .as_object()
                .cloned(),
        })
        .await
        .expect("list skill should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // Verify builtin skills are present
    assert!(content_text.contains("plan"), "Should list 'plan' skill");
    assert!(
        content_text.contains("commit"),
        "Should list 'commit' skill"
    );
    assert!(content_text.contains("test"), "Should list 'test' skill");

    teardown(server, client).await;
}

#[tokio::test]
async fn test_use_skill_returns_instructions() {
    let (server, client) = setup(true).await;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "skill".into(),
            arguments: serde_json::json!({"op": "use skill", "name": "plan"})
                .as_object()
                .cloned(),
        })
        .await
        .expect("use skill should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // Should return the full skill body with instructions
    assert!(
        content_text.contains("instructions"),
        "Should return instructions field"
    );
    assert!(
        content_text.contains("plan"),
        "Should contain the skill name"
    );

    teardown(server, client).await;
}

#[tokio::test]
async fn test_search_skill_finds_matches() {
    let (server, client) = setup(true).await;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "skill".into(),
            arguments: serde_json::json!({"op": "search skill", "query": "commit"})
                .as_object()
                .cloned(),
        })
        .await
        .expect("search skill should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // Should find the commit skill
    assert!(
        content_text.contains("commit"),
        "Search for 'commit' should find the commit skill"
    );

    teardown(server, client).await;
}

#[tokio::test]
async fn test_search_skill_no_matches() {
    let (server, client) = setup(true).await;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "skill".into(),
            arguments: serde_json::json!({"op": "search skill", "query": "zzz_nonexistent_zzz"})
                .as_object()
                .cloned(),
        })
        .await
        .expect("search skill with no matches should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // Should return an empty array
    assert!(
        content_text.contains("[]"),
        "Search with no matches should return empty array, got: {}",
        content_text
    );

    teardown(server, client).await;
}

#[tokio::test]
async fn test_skill_tool_agent_mode_gating() {
    // With agent_mode=true, skill tool should be present
    let (server_agent, client_agent) = setup(true).await;
    let tools = client_agent
        .list_tools(Default::default())
        .await
        .expect("list tools should succeed");
    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        tool_names.contains(&"skill".to_string()),
        "agent_mode=true should have skill tool"
    );
    teardown(server_agent, client_agent).await;

    // With agent_mode=false, skill tool should be absent
    let (server_no_agent, client_no_agent) = setup(false).await;
    let tools = client_no_agent
        .list_tools(Default::default())
        .await
        .expect("list tools should succeed");
    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        !tool_names.contains(&"skill".to_string()),
        "agent_mode=false should NOT have skill tool"
    );
    teardown(server_no_agent, client_no_agent).await;
}

#[tokio::test]
async fn test_get_verb_backward_compat() {
    let (server, client) = setup(true).await;

    // "get skill" should still work (backward compat, routes to Use)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "skill".into(),
            arguments: serde_json::json!({"op": "get skill", "name": "plan"})
                .as_object()
                .cloned(),
        })
        .await
        .expect("get skill (backward compat) should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    assert!(
        content_text.contains("instructions"),
        "get verb should return instructions (backward compat)"
    );

    teardown(server, client).await;
}
