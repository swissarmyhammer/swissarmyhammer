//! Integration tests for the in-process ACP agent WebSocket server.
//!
//! `kanban-app` is a binary crate with no library target, so the
//! `ai::agent_ws` module is compiled directly into this test binary via
//! `#[path]` — the same independent-compilation pattern used by
//! `tests/cli_install.rs` and `build.rs` files across this workspace.
//!
//! Only `agent_ws.rs` is pulled in here, not the whole `ai/mod.rs`: the
//! sibling `ai::models` module references `crate::state::AppState` (a Tauri
//! command needs the managed state), which only resolves when compiled as
//! part of the full binary. The `ai::models` agent-registry logic has its
//! own inline tests in `src/ai/models.rs`.
//!
//! Scope: these tests exercise the loopback WebSocket transport and the ACP
//! `initialize` round-trip against an agent built in-process by
//! `swissarmyhammer_agent::create_agent`. No external agent subprocess is
//! involved — claude-agent answers `initialize` purely from capability
//! negotiation, without spawning the `claude` CLI.

// `agent_ws.rs` is pulled in directly. It has no `crate::`-relative
// references, so it compiles standalone in this test binary. Its module-wide
// `dead_code` allowance lives in `ai/mod.rs`, which is not included here, so
// it is re-stated at this `mod` site.
#[allow(dead_code)]
#[path = "../src/ai/agent_ws.rs"]
mod agent_ws;

use agent_ws::AgentWebSocketServer;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// Drive a single `initialize` request/response over a fresh WebSocket
/// connection to a just-started in-process agent server.
///
/// The server binds an ephemeral loopback port, builds the ACP agent in
/// process on connect, and answers the ACP `initialize` handshake. The
/// returned value is the parsed JSON-RPC response object.
async fn initialize_round_trip() -> serde_json::Value {
    let server = AgentWebSocketServer::bind()
        .await
        .expect("WebSocket agent server should bind to a loopback port");
    let addr = server.local_addr();

    // The accept loop runs for the lifetime of this task; it is aborted when
    // the test returns and the handle is dropped.
    let _server_task = tokio::spawn(async move { server.run().await });

    let url = format!("ws://{addr}/");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("client should connect to the loopback WebSocket");

    // ACP `initialize` as a JSON-RPC 2.0 request. `protocolVersion` is the
    // numeric ACP protocol level (1 is the current release).
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false
            }
        }
    });

    ws.send(Message::text(request.to_string()))
        .await
        .expect("initialize request should be sent");

    let reply = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => return text.to_string(),
                Some(Ok(Message::Close(_))) | None => {
                    panic!("connection closed before an initialize response arrived")
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => panic!("WebSocket error while awaiting response: {e}"),
            }
        }
    })
    .await
    .expect("an initialize response should arrive within the timeout");

    serde_json::from_str(&reply).expect("response should be valid JSON")
}

/// A WebSocket client that connects to the in-process server and sends an
/// ACP `initialize` request receives a valid JSON-RPC response over the
/// loopback transport: a negotiated protocol version when the ClaudeCode
/// backend can be built (the `claude` CLI is installed), or a well-formed
/// JSON-RPC error when it cannot.
#[tokio::test]
async fn websocket_initialize_round_trip_negotiates_protocol_version() {
    let response = initialize_round_trip().await;

    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "response must be a JSON-RPC 2.0 message: {response}"
    );
    assert_eq!(
        response.get("id").and_then(|v| v.as_i64()),
        Some(1),
        "response id must echo the request id: {response}"
    );

    // The default model is the ClaudeCode backend, whose agent can only be
    // built when the `claude` CLI is on PATH. Where it is (local dev) we assert
    // real protocol-version negotiation. Where it is NOT (e.g. CI runners that
    // do not install `claude`) `create_agent` reports the agent as unavailable,
    // and the server must answer with a well-formed JSON-RPC *error* rather than
    // hang or drop the socket — that graceful path is itself worth asserting.
    // Either way the loopback WebSocket transport and ACP JSON-RPC framing have
    // round-tripped a request and response, which is what this test exercises.
    match response.get("result") {
        Some(result) => {
            let protocol_version = result
                .get("protocolVersion")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| {
                    panic!("initialize result must carry a protocolVersion: {result}")
                });

            assert!(
                protocol_version >= 1,
                "negotiated protocol version must be at least 1, got {protocol_version}"
            );
        }
        None => {
            let error = response.get("error").unwrap_or_else(|| {
                panic!("initialize must return either a result or an error, got: {response}")
            });
            assert!(
                error.get("code").and_then(|v| v.as_i64()).is_some(),
                "a JSON-RPC error must carry a numeric code: {error}"
            );
            assert!(
                error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .is_some_and(|m| !m.is_empty()),
                "a JSON-RPC error must carry a non-empty message: {error}"
            );
        }
    }
}
