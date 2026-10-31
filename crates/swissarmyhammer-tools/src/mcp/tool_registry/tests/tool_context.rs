//! `ToolContext` builders, `call_tool` and MCP log notifications.

use super::fixtures::{new_context, MockTool};
use crate::mcp::tool_registry::*;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Wrap a value in the `Arc<RwLock<_>>` shape the context builders take.
fn shared<T>(value: T) -> Arc<RwLock<T>> {
    Arc::new(RwLock::new(value))
}

/// Build a context whose tool registry is `registry`.
fn new_context_with_registry(registry: ToolRegistry) -> ToolContext {
    new_context().with_tool_registry(shared(registry))
}

/// Build a registry holding one `echo_tool` mock tool.
fn echo_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "echo_tool",
        description: "Echoes input",
    });
    registry
}

// --- ToolContext builder method tests ---

#[test]
fn test_tool_context_with_prompt_library() {
    use swissarmyhammer_templating::TemplateLibrary;
    let ctx = new_context();
    assert!(ctx.prompt_library.is_none());

    let ctx = ctx.with_prompt_library(shared(TemplateLibrary::new()));
    assert!(ctx.prompt_library.is_some());
}

#[test]
fn test_tool_context_with_plan_sender() {
    use crate::mcp::plan_notifications::{PlanNotification, PlanSender};
    use tokio::sync::mpsc;

    let ctx = new_context();
    assert!(ctx.plan_sender.is_none());

    let (tx, _rx) = mpsc::unbounded_channel::<PlanNotification>();
    let sender = PlanSender::new(tx);
    let ctx = ctx.with_plan_sender(sender);
    assert!(ctx.plan_sender.is_some());
}

#[test]
fn test_tool_context_with_tool_registry() {
    let ctx = new_context();
    assert!(ctx.tool_registry.is_none());

    let ctx = ctx.with_tool_registry(shared(ToolRegistry::new()));
    assert!(ctx.tool_registry.is_some());
}

#[test]
fn test_tool_context_with_working_dir() {
    let ctx = new_context();
    assert!(ctx.working_dir.is_none());

    let dir = std::path::PathBuf::from("/tmp/test");
    let ctx = ctx.with_working_dir(dir.clone());
    assert_eq!(ctx.working_dir, Some(dir));
}

#[test]
fn test_session_root_prefers_working_dir() {
    // With a working dir set, session_root returns it verbatim — never the
    // process CWD.
    let board_dir = std::path::PathBuf::from("/tmp/some-board");
    let ctx = new_context().with_working_dir(board_dir.clone());
    assert_eq!(ctx.session_root(), board_dir);
}

#[tokio::test]
async fn test_tool_context_mcp_server_defaults_to_none() {
    let ctx = new_context();

    let guard = ctx.mcp_server.read().await;
    assert!(guard.is_none());
}

#[tokio::test]
async fn test_tool_context_set_mcp_server() {
    use swissarmyhammer_templating::TemplateLibrary;

    let work_dir = tempfile::tempdir().expect("temp dir for the server work dir");
    let server = Arc::new(
        crate::mcp::McpServer::new_with_work_dir(
            TemplateLibrary::default(),
            work_dir.path().to_path_buf(),
        )
        .await
        .expect("server builds"),
    );

    let ctx = new_context();

    ctx.set_mcp_server(server.clone()).await;

    let guard = ctx.mcp_server.read().await;
    let stored = guard
        .as_ref()
        .expect("the server is stored after the set call");
    assert!(Arc::ptr_eq(stored, &server));
}

// --- call_tool tests ---

#[tokio::test]
async fn test_call_tool_no_registry_returns_error() {
    let ctx = new_context();

    // No tool registry set — should return internal error
    let result = ctx.call_tool("anything", serde_json::json!({})).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{:?}", err).contains("tool registry not available"),
        "expected 'tool registry not available' in error, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_call_tool_missing_tool_returns_error() {
    let ctx = new_context_with_registry(ToolRegistry::new());

    let result = ctx
        .call_tool("nonexistent_tool", serde_json::json!({}))
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("not found in registry"),
        "expected 'not found in registry' in error, got: {msg}"
    );
}

#[tokio::test]
async fn test_call_tool_non_object_params_returns_error() {
    let ctx = new_context_with_registry(echo_tool_registry());

    // Pass an array instead of object — should fail
    let result = ctx
        .call_tool("echo_tool", serde_json::json!(["not", "an", "object"]))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_call_tool_success() {
    let ctx = new_context_with_registry(echo_tool_registry());

    let result = ctx.call_tool("echo_tool", serde_json::json!({})).await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
}

// --- send_mcp_log tests ---

#[tokio::test]
async fn test_send_mcp_log_no_peer_is_noop() {
    use rmcp::model::LoggingLevel;

    let ctx = new_context();
    // No peer — send_mcp_log should be a no-op and not panic
    send_mcp_log(&ctx, LoggingLevel::Info, LoggerName("test"), "hello").await;
}

// --- ToolContext session_id tests ---

#[test]
fn test_tool_context_has_unique_session_id() {
    let ctx1 = new_context();
    let ctx2 = new_context();

    // Each context should have a unique session_id
    assert!(!ctx1.session_id.is_empty());
    assert!(!ctx2.session_id.is_empty());
    assert_ne!(ctx1.session_id, ctx2.session_id);
}

// --- ToolContext peer tests ---

/// Minimal server handler: every `ServerHandler` method has a default, so this
/// is enough to bring up a real `Peer<RoleServer>` over an in-memory pipe.
struct NoopServer;

impl rmcp::ServerHandler for NoopServer {}

/// Byte capacity of the in-memory pipe the test peer talks through.
const PEER_PIPE_BYTES: usize = 4096;

#[test]
fn test_tool_context_peer_defaults_to_none() {
    let ctx = new_context();
    assert!(ctx.peer.is_none());
}

#[tokio::test]
async fn test_tool_context_with_peer_sets_peer() {
    use rmcp::ServiceExt;

    // Bring up a real server peer. The `initialize` handshake needs both ends
    // running, so the server end starts in its own task.
    let (client_io, server_io) = tokio::io::duplex(PEER_PIPE_BYTES);
    let server_task = tokio::spawn(rmcp::serve_server(NoopServer, tokio::io::split(server_io)));
    let _client =
        ().serve(tokio::io::split(client_io))
            .await
            .expect("client connects over the in-memory pipe");
    let running_server = server_task
        .await
        .expect("server task joins")
        .expect("server serves over the in-memory pipe");
    let peer = Arc::new(running_server.peer().clone());

    let ctx = new_context().with_peer(peer.clone());

    let stored = ctx
        .peer
        .as_ref()
        .expect("the peer is stored after with_peer");
    assert!(Arc::ptr_eq(stored, &peer));
}

#[test]
fn test_logger_name_displays_its_contents() {
    assert_eq!(LoggerName("kanban").to_string(), "kanban");
}
