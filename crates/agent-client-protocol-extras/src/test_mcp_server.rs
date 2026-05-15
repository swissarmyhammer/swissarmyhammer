//! TestMcpServer — predictable MCP server for ACP conformance tests.
//!
//! The conformance suite exercises both the ACP-side notification path and
//! the MCP-side notification path. To produce stable fixtures the suite
//! needs:
//!
//! 1. An **MCP server with deterministic tools** (`list-files`, `create-plan`)
//!    that emits both logging and progress notifications during tool calls.
//! 2. A **proxy in front of that server** that captures the upstream
//!    notifications into a `broadcast::Sender<McpNotification>` so they can
//!    be folded into the recorded ACP fixture.
//!
//! [`TestMcpServerHandler`] (the `rmcp::ServerHandler`) supplies (1).
//! [`TestMcpServer`] (the high-level wrapper returned by
//! [`start_test_mcp_server_with_capture`]) supplies (2): it boots the
//! upstream handler then puts a [`McpProxy`] in front of it, exposing the
//! proxy's URL via [`TestMcpServer::url`] and forwarding
//! [`McpNotificationSource`] subscriptions through to the proxy.
//!
//! ## Lifecycle
//!
//! Both the upstream test server and the proxy are owned by `TestMcpServer`.
//! Dropping the wrapper aborts the upstream serve task; the proxy task is
//! aborted by the proxy's own `Drop` (it owns its own `JoinHandle`). Tests
//! should keep the wrapper alive for as long as they need to call the proxy
//! URL.

use model_context_protocol_extras::{
    start_proxy, McpNotification, McpNotificationSource, McpProxy,
};
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// TestMcpServerHandler — the upstream rmcp::ServerHandler
// ---------------------------------------------------------------------------

/// Predictable MCP `ServerHandler` for ACP conformance tests.
///
/// Exposes two tools:
///
/// - `list-files(path)` — returns a fixed listing of three files. Emits a
///   logging notification at the start of the call and one progress
///   notification per simulated file.
/// - `create-plan(goal)` — returns a fixed four-step plan. Emits a logging
///   notification at the start and one progress notification per step.
///
/// The deterministic shape makes captured fixtures stable across runs.
#[derive(Clone)]
pub struct TestMcpServerHandler {
    name: String,
    version: String,
}

impl TestMcpServerHandler {
    /// Create a fresh handler. The name and version land in the
    /// `initialize` response and the `ServerInfo`.
    pub fn new() -> Self {
        Self {
            name: "test-mcp-server".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    /// Static descriptions of the two tools.
    fn get_tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "list-files",
                "List files in a directory",
                Arc::new({
                    let mut map = Map::new();
                    map.insert("type".to_string(), json!("object"));
                    map.insert(
                        "properties".to_string(),
                        json!({"path": {"type": "string", "description": "Directory path"}}),
                    );
                    map.insert("required".to_string(), json!(["path"]));
                    map
                }),
            ),
            Tool::new(
                "create-plan",
                "Create an execution plan",
                Arc::new({
                    let mut map = Map::new();
                    map.insert("type".to_string(), json!("object"));
                    map.insert(
                        "properties".to_string(),
                        json!({"goal": {"type": "string", "description": "Goal"}}),
                    );
                    map.insert("required".to_string(), json!(["goal"]));
                    map
                }),
            ),
        ]
    }

    /// Deterministic `list-files` body — three named files, fixed order.
    fn execute_list_files(path: &str) -> Value {
        json!({
            "files": ["file1.txt", "file2.txt", "file3.txt"],
            "path": path,
            "count": 3
        })
    }

    /// Deterministic `create-plan` body — four pending steps.
    fn execute_create_plan(goal: &str) -> Value {
        json!({
            "plan": {
                "goal": goal,
                "steps": [
                    {"id": 1, "description": "Analyze requirements", "status": "pending"},
                    {"id": 2, "description": "Design solution", "status": "pending"},
                    {"id": 3, "description": "Implement solution", "status": "pending"},
                    {"id": 4, "description": "Test and validate", "status": "pending"}
                ]
            }
        })
    }
}

impl Default for TestMcpServerHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for TestMcpServerHandler {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::info!(
            "TestMcpServer: Client connecting: {} v{}",
            request.client_info.name,
            request.client_info.version
        );

        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        Ok(InitializeResult::new(caps)
            .with_server_info(Implementation::new(self.name.clone(), self.version.clone()))
            .with_instructions("Test MCP server for ACP conformance testing"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        tracing::debug!("TestMcpServer: list_tools called");
        Ok(ListToolsResult {
            tools: Self::get_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("TestMcpServer: call_tool: {}", request.name);

        let arguments = request.arguments.unwrap_or_default();

        match request.name.as_ref() {
            "list-files" => {
                let path = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                // Send one logging notification at start.
                let _ = context
                    .peer
                    .send_notification(
                        LoggingMessageNotification::new(LoggingMessageNotificationParam {
                            level: LoggingLevel::Info,
                            logger: Some("test-mcp-server".to_string()),
                            data: json!({"message": format!("Listing files in: {}", path)}),
                        })
                        .into(),
                    )
                    .await;

                let token = ProgressToken(NumberOrString::String("list-files-1".into()));

                let result = Self::execute_list_files(path);
                let files = result["files"].as_array().unwrap();

                // Progress notification for each file.
                for (i, _file) in files.iter().enumerate() {
                    let _ = context
                        .peer
                        .send_notification(
                            ProgressNotification::new(ProgressNotificationParam {
                                progress_token: token.clone(),
                                progress: (i + 1) as f64,
                                total: Some(files.len() as f64),
                                message: Some(format!("Processing file {}/{}", i + 1, files.len())),
                            })
                            .into(),
                        )
                        .await;

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            "create-plan" => {
                let goal = arguments
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                let _ = context
                    .peer
                    .send_notification(
                        LoggingMessageNotification::new(LoggingMessageNotificationParam {
                            level: LoggingLevel::Info,
                            logger: Some("test-mcp-server".to_string()),
                            data: json!({"message": format!("Creating plan for: {}", goal)}),
                        })
                        .into(),
                    )
                    .await;

                let token = ProgressToken(NumberOrString::String("create-plan-1".into()));
                let result = Self::execute_create_plan(goal);
                let steps = result["plan"]["steps"].as_array().unwrap();

                for (i, _step) in steps.iter().enumerate() {
                    let _ = context
                        .peer
                        .send_notification(
                            ProgressNotification::new(ProgressNotificationParam {
                                progress_token: token.clone(),
                                progress: (i + 1) as f64,
                                total: Some(steps.len() as f64),
                                message: Some(format!("Creating step {}/{}", i + 1, steps.len())),
                            })
                            .into(),
                        )
                        .await;

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            _ => Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            )),
        }
    }

    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        ServerInfo::new(caps)
            .with_server_info(Implementation::new(self.name.clone(), self.version.clone()))
            .with_instructions("Test MCP server for ACP conformance testing")
    }
}

// ---------------------------------------------------------------------------
// start_test_mcp_server — bare upstream, no proxy.
// ---------------------------------------------------------------------------

/// Start a [`TestMcpServerHandler`] as an in-process HTTP server and return
/// its URL.
///
/// The server runs in a detached task that lives until the process exits.
/// Bare callers don't get a handle to abort it; use
/// [`start_test_mcp_server_with_capture`] when you need a wrapper that owns
/// the server's lifetime and exposes notification capture.
pub async fn start_test_mcp_server() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (url, handle) = start_test_mcp_server_inner().await?;
    // Detach: the bare API has no place to retain the handle. The task
    // terminates when the process exits; this matches the pre-refactor
    // behaviour of `tokio::spawn(...)` without binding the handle.
    drop(handle);
    Ok(url)
}

/// Start a [`TestMcpServerHandler`] and return both its URL and the
/// `JoinHandle` of the serve task. The handle lets callers tie the server's
/// lifetime to their wrapper.
async fn start_test_mcp_server_inner(
) -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn std::error::Error + Send + Sync>> {
    let server = Arc::new(TestMcpServerHandler::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}/mcp", addr);

    tracing::info!("TestMcpServer starting on {}", url);

    let http_service = StreamableHttpService::new(
        move || Ok((*server).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", http_service);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("TestMcpServer error: {}", e);
        }
    });

    // Small delay so callers that immediately connect don't race the bind.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    tracing::info!("TestMcpServer running at {}", url);
    Ok((url, handle))
}

// ---------------------------------------------------------------------------
// TestMcpServer — high-level wrapper with notification capture.
// ---------------------------------------------------------------------------

/// In-process MCP test server fronted by an [`McpProxy`] that captures every
/// notification flowing from the upstream handler.
///
/// Returned by [`start_test_mcp_server_with_capture`]. The wrapper:
///
/// - Owns the upstream serve task — dropping the wrapper aborts it.
/// - Owns the [`McpProxy`] that captures notifications — dropping the
///   wrapper drops the proxy, which aborts the proxy's own serve task.
///
/// The URL exposed via [`Self::url`] points at the **proxy**, not the
/// upstream. Tests configure their agents to hit this URL so the proxy can
/// observe and capture the resulting MCP traffic.
///
/// `TestMcpServer` implements [`McpNotificationSource`] by forwarding to
/// the inner proxy's implementation, so callers can pass `&test_mcp_server`
/// to anything that expects `&dyn McpNotificationSource`.
pub struct TestMcpServer {
    /// Proxy that fronts the upstream test server. Owns its own serve task.
    proxy: McpProxy,
    /// Upstream serve task. Aborted on drop so the test server doesn't
    /// outlive its wrapper.
    upstream_task: tokio::task::JoinHandle<()>,
}

impl TestMcpServer {
    /// URL of the **proxy**. Tests should hand this URL to their agent's
    /// MCP-server config so the proxy can observe the resulting MCP
    /// traffic.
    pub fn url(&self) -> &str {
        self.proxy.url()
    }

    /// Subscribe to the captured MCP notification stream. The receiver
    /// observes every progress / logging notification emitted by the
    /// upstream test server.
    pub fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        McpNotificationSource::subscribe(&self.proxy)
    }
}

impl McpNotificationSource for TestMcpServer {
    fn url(&self) -> &str {
        TestMcpServer::url(self)
    }

    fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        TestMcpServer::subscribe(self)
    }
}

impl Drop for TestMcpServer {
    fn drop(&mut self) {
        self.upstream_task.abort();
    }
}

/// Boot a [`TestMcpServerHandler`] and front it with an [`McpProxy`] so
/// callers can capture the MCP notifications it emits.
///
/// Returns a [`TestMcpServer`] that owns both the upstream server's serve
/// task and the proxy. The `url()` method on the returned wrapper points at
/// the proxy (not the upstream); tests should configure their MCP-using
/// agents with this URL so notifications flow through the capture path.
///
/// # Errors
///
/// Returns the underlying transport error if either the upstream test
/// server or the proxy fails to bind. In practice this only happens when
/// the loopback interface is unavailable, which is rare in test environments.
pub async fn start_test_mcp_server_with_capture(
) -> Result<TestMcpServer, Box<dyn std::error::Error + Send + Sync>> {
    let (upstream_url, upstream_task) = start_test_mcp_server_inner().await?;
    tracing::info!(
        "TestMcpServer: starting capture proxy in front of {}",
        upstream_url
    );
    let proxy = start_proxy(&upstream_url).await?;
    Ok(TestMcpServer {
        proxy,
        upstream_task,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::transport::StreamableHttpClientTransport;
    use rmcp::{ClientHandler, ServiceExt};

    /// Minimal `ClientHandler` that just lets us connect and call tools.
    #[derive(Clone)]
    struct DummyClient;

    impl ClientHandler for DummyClient {
        fn get_info(&self) -> ClientInfo {
            ClientInfo::new(
                ClientCapabilities::default(),
                Implementation::new("test-client", "0").with_title("Test client"),
            )
        }
    }

    #[test]
    fn handler_creation_uses_expected_metadata() {
        let server = TestMcpServerHandler::new();
        assert_eq!(server.name, "test-mcp-server");
        assert_eq!(server.version, "1.0.0");
    }

    #[test]
    fn handler_exposes_two_tools() {
        let tools = TestMcpServerHandler::get_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "list-files"));
        assert!(tools.iter().any(|t| t.name == "create-plan"));
    }

    #[test]
    fn execute_list_files_returns_three_files_at_expected_path() {
        let result = TestMcpServerHandler::execute_list_files("/tmp/x");
        assert_eq!(result["count"], 3);
        assert_eq!(result["path"], "/tmp/x");
        assert_eq!(result["files"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn execute_create_plan_returns_four_pending_steps() {
        let result = TestMcpServerHandler::execute_create_plan("ship feature X");
        assert_eq!(result["plan"]["goal"], "ship feature X");
        assert_eq!(result["plan"]["steps"].as_array().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn start_test_mcp_server_with_capture_exposes_proxy_url() {
        let server = start_test_mcp_server_with_capture()
            .await
            .expect("server should boot");
        assert!(
            server.url().starts_with("http://127.0.0.1:"),
            "url should be loopback: {}",
            server.url()
        );
        assert!(
            server.url().ends_with("/mcp"),
            "url should end with /mcp: {}",
            server.url()
        );

        // Subscribe before any traffic flows — the receiver must be wired
        // up immediately.
        let _rx = server.subscribe();

        // Verify the McpNotificationSource trait impl forwards correctly.
        let as_source: &dyn McpNotificationSource = &server;
        assert_eq!(as_source.url(), server.url());
    }

    #[tokio::test]
    async fn capture_proxy_observes_notifications_from_upstream() {
        let server = start_test_mcp_server_with_capture()
            .await
            .expect("server should boot");
        let url = server.url().to_string();
        let mut rx = server.subscribe();

        // Drive a tool call through the proxy URL and assert that the
        // subscriber sees at least one captured notification (logging +
        // per-file progress arrive during list-files).
        let transport = StreamableHttpClientTransport::from_uri(url);
        let client = DummyClient
            .serve(transport)
            .await
            .expect("client connects to proxy");

        // Initialise + call list-files with a known path. The connect
        // already ran the MCP `initialize` handshake; call_tool drives the
        // upstream tool which fans out the notifications we're trying to
        // capture.
        let result = client
            .peer()
            .call_tool(
                CallToolRequestParams::new("list-files")
                    .with_arguments(json!({"path": "/tmp"}).as_object().unwrap().clone()),
            )
            .await
            .expect("call_tool");
        assert!(!result.content.is_empty());

        // We expect at least one notification (logging at start, plus three
        // progress entries — but timing means we just check for >= 1).
        let mut saw_any = false;
        for _ in 0..8 {
            match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
                Ok(Ok(_)) => {
                    saw_any = true;
                    break;
                }
                Ok(Err(_)) | Err(_) => continue,
            }
        }
        assert!(
            saw_any,
            "capture proxy should observe at least one notification"
        );

        // Cancel before drop so the server task isn't waiting on stdio.
        client.cancel().await.ok();
    }
}
