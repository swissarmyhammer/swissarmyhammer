//! Model-free MCP test for the subscribable diagnostics resource.
//!
//! Boots the real in-process HTTP MCP server, connects an rmcp client that
//! overrides `ClientHandler::on_resource_updated` to capture every
//! `notifications/resources/updated`, subscribes to the diagnostics resource,
//! then pushes a per-uri diagnostics cache update through the server's
//! process-wide diagnostics-resource publish sink.
//!
//! It asserts:
//!
//! 1. The diagnostics resource is advertised by `resources/list`.
//! 2. A pushed cache update emits exactly one
//!    `notifications/resources/updated` carrying the diagnostics resource URI.
//! 3. Reading the resource after the push reflects the pushed diagnostics.
//!
//! This is the courtesy-channel wiring check: a subscribing host receives
//! diagnostics without issuing a tool call. It is model-free — no LSP server
//! and no agent — driving the cache directly through the publish sink the
//! in-process fan-out also feeds.

use rmcp::model::{
    ClientCapabilities, ClientInfo, Implementation, ReadResourceRequestParams,
    ResourceUpdatedNotificationParam, SubscribeRequestParams,
};
use rmcp::service::NotificationContext;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use std::sync::{Arc, Mutex};
use swissarmyhammer_tools::mcp::diagnostics_resource::{
    publish_diagnostics_update, DIAGNOSTICS_RESOURCE_URI,
};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
use tempfile::TempDir;

/// Client handler that captures every `notifications/resources/updated`.
#[derive(Clone)]
struct CapturingClient {
    info: ClientInfo,
    captured: Arc<Mutex<Vec<ResourceUpdatedNotificationParam>>>,
}

impl CapturingClient {
    fn new() -> Self {
        Self {
            info: ClientInfo::new(
                ClientCapabilities::default(),
                Implementation::new("resource-updated-capturing-client", "1.0.0"),
            ),
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn snapshot(&self) -> Vec<ResourceUpdatedNotificationParam> {
        self.captured.lock().unwrap().clone()
    }
}

impl ClientHandler for CapturingClient {
    fn get_info(&self) -> ClientInfo {
        self.info.clone()
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.captured.lock().unwrap().push(params);
    }
}

#[tokio::test]
async fn subscribing_then_pushing_a_cache_update_emits_resources_updated() {
    // 1. Stand up an in-process HTTP MCP server scoped to a tempdir.
    let project = TempDir::new().expect("create temp dir");
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(project.path().to_path_buf()),
    )
    .await
    .expect("start MCP server");

    // 2. Connect a client that captures every resource-updated notification.
    let handler = CapturingClient::new();
    let transport = StreamableHttpClientTransport::with_client(reqwest::Client::default(), {
        let mut config = StreamableHttpClientTransportConfig::default();
        config.uri = server.url().into();
        config
    });
    let client = handler
        .clone()
        .serve(transport)
        .await
        .expect("connect client");

    // 3. The diagnostics resource must be advertised.
    let resources = client.list_all_resources().await.expect("list resources");
    assert!(
        resources.iter().any(|r| r.uri == DIAGNOSTICS_RESOURCE_URI),
        "diagnostics resource must be advertised, got {:?}",
        resources.iter().map(|r| &r.uri).collect::<Vec<_>>()
    );

    // 4. Subscribe to the diagnostics resource.
    client
        .subscribe(SubscribeRequestParams::new(DIAGNOSTICS_RESOURCE_URI))
        .await
        .expect("subscribe to diagnostics resource");

    // 5. Push a per-uri diagnostics cache update through the publish sink the
    //    in-process fan-out feeds. The server must emit
    //    notifications/resources/updated to the subscriber.
    let uri = "file:///workspace/src/main.rs";
    publish_diagnostics_update(uri, vec![diagnostic_at(1, "unused variable `x`")]);

    // Give the notification a moment to traverse the transport to the client.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let captured = handler.snapshot();

    // 6. The pushed update must reflect in the resource read.
    let read = client
        .read_resource(ReadResourceRequestParams::new(DIAGNOSTICS_RESOURCE_URI))
        .await
        .expect("read diagnostics resource");

    client.cancel().await.ok();
    server.shutdown().await.ok();

    // 7. Assertions.
    assert_eq!(
        captured.len(),
        1,
        "exactly one resources/updated expected, got {captured:?}"
    );
    assert_eq!(
        captured[0].uri, DIAGNOSTICS_RESOURCE_URI,
        "resources/updated must carry the diagnostics resource URI"
    );

    let body = match read.contents.first() {
        Some(rmcp::model::ResourceContents::TextResourceContents { text, .. }) => text.clone(),
        other => panic!("expected text resource contents, got {other:?}"),
    };
    assert!(
        body.contains("src/main.rs") && body.contains("unused variable `x`"),
        "resource read must reflect the pushed diagnostics, got {body}"
    );
}

/// Build a single LSP diagnostic at a line with a message.
fn diagnostic_at(line: u32, message: &str) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position { line, character: 0 },
            end: lsp_types::Position { line, character: 1 },
        },
        severity: Some(lsp_types::DiagnosticSeverity::WARNING),
        message: message.to_string(),
        ..Default::default()
    }
}
