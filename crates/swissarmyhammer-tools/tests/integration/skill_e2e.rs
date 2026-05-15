//! End-to-end tests for the skill tool pipeline
//!
//! Tests the full pipeline: skill resolution → MCP tool registration → tool calls
//! Uses in-process HTTP MCP server + RMCP client (same pattern as rmcp_integration.rs)
//!
//! Verifies the pipeline using real builtin skills (e.g. `plan`). Tests invoke skills
//! by name and verify body-only content (not present in frontmatter) is returned,
//! proving: skill resolution → embedding → MCP tool registration → invocation → instruction delivery.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server_with_options, McpServerMode},
};

/// Helper to start a server and client with agent_mode setting.
///
/// Uses a temp directory as working_dir so that local `.skills/` overrides
/// (which contain pre-rendered templates) do not mask builtin skills that
/// still have raw Liquid templates like `{{arguments}}`.
async fn setup(
    agent_mode: bool,
) -> (
    swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
    rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
    tempfile::TempDir,
) {
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");
    let server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
        agent_mode,
    )
    .await
    .expect("Failed to start MCP server");
    let client = create_test_client(server.url()).await;
    (server, client, temp)
}

/// Helper to teardown server and client
async fn teardown(
    mut server: swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
    client: rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
) {
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Helper to build CallToolRequestParams for the skill tool
fn skill_params(args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams::new("skill")
        .with_arguments(args.as_object().cloned().unwrap_or_default())
}

/// Cap for the preview slice of `content_text` shown in assertion failure messages.
/// Skill responses carry full SKILL.md bodies (multi-KB); truncating to this length
/// keeps test output readable without losing the leading content where assertion
/// failures are almost always visible.
const ASSERT_PREVIEW_LEN: usize = 500;

#[tokio::test]
async fn test_builtin_skills_discovered_via_list() {
    let (server, client, _temp) = setup(true).await;

    let result = client
        .call_tool(skill_params(serde_json::json!({"op": "list skill"})))
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
    let (server, client, _temp) = setup(true).await;

    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "use skill", "name": "plan"}),
        ))
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
    let (server, client, _temp) = setup(true).await;

    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "search skill", "query": "commit"}),
        ))
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
    let (server, client, _temp) = setup(true).await;

    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "search skill", "query": "zzz_nonexistent_zzz"}),
        ))
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
    let (server_agent, client_agent, _temp1) = setup(true).await;
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
    let (server_no_agent, client_no_agent, _temp2) = setup(false).await;
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
    let (server, client, _temp) = setup(true).await;

    // "get skill" should still work (backward compat, routes to Use)
    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "get skill", "name": "plan"}),
        ))
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

// =========================================================================
// Full pipeline verification: invoke builtin skill, verify body content
// =========================================================================

/// A string that appears ONLY in the plan skill's instruction body (not frontmatter).
/// Finding it in a `use skill` response proves the full pipeline delivered instructions.
const PLAN_BODY_MARKER: &str = "kanban";

#[tokio::test]
async fn test_skill_invoke_by_name_returns_body_content() {
    let (server, client, _temp) = setup(true).await;

    // Invoke the plan skill by name
    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "use skill", "name": "plan"}),
        ))
        .await
        .expect("use skill plan should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // DRAFT_PLAN.md is ONLY in the skill body, not in frontmatter.
    // Finding it here proves the full pipeline delivered instructions.
    assert!(
        content_text.contains(PLAN_BODY_MARKER),
        "Invoking plan skill by name should return instructions containing body-only marker '{}', got: {}",
        PLAN_BODY_MARKER,
        content_text
    );

    // Also verify it contains the kanban board reference from the body
    assert!(
        content_text.contains("kanban board"),
        "Instructions should reference kanban board"
    );

    teardown(server, client).await;
}

/// Verify that the test skill returns its body content through the MCP pipeline.
/// The test skill is now a thin dispatcher that delegates to a tester subagent.
#[tokio::test]
async fn test_skill_test_returns_body_content() {
    let (server, client, _temp) = setup(true).await;

    let result = client
        .call_tool(skill_params(
            serde_json::json!({"op": "use skill", "name": "test"}),
        ))
        .await
        .expect("use skill test should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // The test skill is a dispatcher that references the tester subagent
    assert!(
        content_text.contains("tester"),
        "Test skill should reference tester subagent, got: {}",
        &content_text[..content_text.len().min(ASSERT_PREVIEW_LEN)]
    );

    // Verify the skill's own body content is present
    assert!(
        content_text.contains("Zero failures"),
        "Skill instructions should contain own body content"
    );

    // The raw {% include %} tag should NOT be present in rendered output
    assert!(
        !content_text.contains("{% include"),
        "Rendered output should not contain raw Liquid include tags"
    );

    teardown(server, client).await;
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_use_skill_with_arguments_renders_in_output() {
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(temp.path())
        .expect("Failed to change CWD");
    let (server, client, _temp) = setup(true).await;

    // Invoke the task skill with arguments — verifies the MCP pipeline accepts
    // and passes through the "arguments" parameter without error.
    //
    // Note: Whether {{arguments}} appears in the rendered output depends on whether
    // the resolved skill has template tags (builtin) or is a pre-rendered local
    // override. The actual template rendering of arguments is verified by the unit
    // test `test_skill_use_renders_arguments_template` in use_op.rs.
    let result = client
        .call_tool(skill_params(serde_json::json!({
            "op": "use skill",
            "name": "task",
            "arguments": "fix the login bug"
        })))
        .await
        .expect("use skill with arguments should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    // The skill should return valid content with instructions
    assert!(
        content_text.contains("instructions"),
        "Skill response should contain instructions field, got: {}",
        &content_text[..content_text.len().min(ASSERT_PREVIEW_LEN)]
    );

    // The skill name should be present in the response
    assert!(
        content_text.contains("task"),
        "Skill response should contain the skill name 'task', got: {}",
        &content_text[..content_text.len().min(ASSERT_PREVIEW_LEN)]
    );

    // If the skill has template tags (builtin, not local override), arguments
    // should be rendered. If it's a pre-rendered local override, we at least
    // verify the pipeline didn't error.
    if content_text.contains("User Request") {
        assert!(
            content_text.contains("fix the login bug"),
            "When skill has template tags, arguments should be rendered, got: {}",
            &content_text[..content_text.len().min(ASSERT_PREVIEW_LEN)]
        );
    }

    teardown(server, client).await;
}

#[tokio::test]
async fn test_skill_invoke_via_shorthand() {
    let (server, client, _temp) = setup(true).await;

    // Use the shorthand form (just name, no explicit verb)
    let result = client
        .call_tool(skill_params(serde_json::json!({"name": "plan"})))
        .await
        .expect("shorthand use skill should succeed");

    let content_text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or("");

    assert!(
        content_text.contains(PLAN_BODY_MARKER),
        "Shorthand skill invocation should also deliver body content, got: {}",
        content_text
    );

    teardown(server, client).await;
}
