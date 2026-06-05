//! End-to-end test of the AI panel's production path with a real model.
//!
//! This drives the SAME path the webview hits — an in-process ACP agent served
//! over a loopback WebSocket — against the small `qwen-0.6b-test` model, and
//! asserts the user-visible contract that mock-based unit tests cannot:
//!
//! 1. a real `session/prompt` produces non-empty agent text (in the response
//!    `_meta.llama_response`) and reports `tokens_generated > 0` (the "0 tokens"
//!    bug `01KSNJ7CBK9333J0T9G4TCA7DH` was a *successful* call with an empty
//!    result; only a content assertion catches it),
//! 2. a second prompt on the same session succeeds — the single worker is
//!    released after a turn, so there is no "Queue is full" (the queue-lifecycle
//!    half of the same bug), and
//! 3. with the board's MCP server attached via the ACP `mcpServers` list (the
//!    exact path `ai_start_agent` wires `mcpUrl` through), the agent reaches the
//!    board's kanban toolset and the model actually invokes a tool.
//!
//! # Why `#[path]` instead of a normal `use`
//!
//! `kanban-app` is a binary crate with no library target, so a test binary
//! cannot `use kanban_app::…`. The sibling `tests/agent_ws.rs` already
//! establishes the pattern: pull `ai/agent_ws.rs` in directly via `#[path]`. It
//! has no `crate::`-relative references, so it compiles standalone here. We do
//! NOT pull in `ai/models.rs` (it references `crate::state::AppState`); instead
//! the test resolves the model config and starts the board MCP server through
//! the same public APIs `models.rs` / `state.rs` call in production
//! (`ModelManager` + `swissarmyhammer_tools::mcp::unified_server`).

#[allow(dead_code)]
#[path = "../src/ai/agent_ws.rs"]
mod agent_ws;

use std::time::Duration;

use agent_client_protocol::schema::{McpServer, McpServerHttp, NewSessionRequest};
use agent_ws::AgentWebSocketServer;
use futures_util::{SinkExt, StreamExt};
use swissarmyhammer_config::model::{parse_model_config, ModelConfig, ModelManager};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// The small llama chat model used for fast, real-model testing. Not
/// `kanban`-tagged (it must not clutter the picker), but it is a real
/// `llama-agent` chat executor that `ai_start_agent` can drive directly.
const MODEL_ID: &str = "qwen-0.6b-test";

/// Generous hang-guard for a real-model ACP operation. This is NOT a
/// performance assertion (a healthy turn is far faster) — it only stops the
/// test from hanging forever if the agent wedges. The agent build on first
/// connect includes loading the qwen-0.6B weights, so the budget is roomy. It
/// is deliberately well above a healthy turn: on the shared, GPU-backed CI
/// runner the Metal GPU is time-sliced across whatever else is running, so a
/// turn can take several times its uncontended duration without being wedged.
const OP_BUDGET: Duration = Duration::from_secs(240);

/// Resolve `qwen-0.6b-test` to its `ModelConfig` through the exact public APIs
/// `resolve_model_config` uses in production (`ai/models.rs`).
fn resolve_qwen_test_config() -> ModelConfig {
    let info = ModelManager::find_agent_by_name(MODEL_ID)
        .unwrap_or_else(|e| panic!("test model `{MODEL_ID}` must be discoverable: {e}"));
    parse_model_config(&info.content)
        .unwrap_or_else(|e| panic!("test model `{MODEL_ID}` must parse to a ModelConfig: {e}"))
}

/// Start an in-process agent server for `config` and return its loopback
/// `ws://` URL plus the accept-loop task (aborted when the caller drops it).
async fn start_agent(config: ModelConfig) -> (String, tokio::task::JoinHandle<()>) {
    let server = AgentWebSocketServer::bind_with(config)
        .await
        .expect("the in-process agent server must bind a loopback port");
    let url = format!("ws://{}/", server.local_addr());
    let task = tokio::spawn(server.run());
    (url, task)
}

/// A minimal ACP client speaking JSON-RPC 2.0 over the loopback WebSocket —
/// the same transport the webview's ACP client uses.
struct AcpWsClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: i64,
}

/// The result of one `session/prompt` turn: the JSON-RPC `result` object. The
/// agent's full reply and token count live in the result's `_meta`
/// (`llama_response` / `tokens_generated`); this in-process ACP agent does not
/// stream `session/update` chunks over the wire, so the result is the turn's
/// observable output.
struct PromptOutcome {
    result: serde_json::Value,
}

impl AcpWsClient {
    async fn connect(url: &str) -> Self {
        let (ws, _resp) = tokio_tungstenite::connect_async(url)
            .await
            .expect("ACP client must connect to the agent's loopback ws:// URL");
        Self { ws, next_id: 0 }
    }

    /// Send a JSON-RPC request and return its id.
    async fn send(&mut self, method: &str, params: serde_json::Value) -> i64 {
        self.next_id += 1;
        let id = self.next_id;
        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.ws
            .send(Message::text(frame.to_string()))
            .await
            .unwrap_or_else(|e| panic!("sending `{method}` must succeed: {e}"));
        id
    }

    /// Read frames until the JSON-RPC response with `id` arrives, returning it.
    /// `session/update` notifications that stream in the meantime are passed to
    /// `on_notification`. Any error frame for `id` panics with its message.
    async fn await_response(
        &mut self,
        id: i64,
        method: &str,
        mut on_notification: impl FnMut(&serde_json::Value),
    ) -> serde_json::Value {
        let read = async {
            loop {
                match self.ws.next().await {
                    Some(Ok(Message::Text(text))) => {
                        let msg: serde_json::Value = serde_json::from_str(&text)
                            .unwrap_or_else(|e| panic!("agent frame must be valid JSON: {e}"));
                        // A response carries our id; a notification carries none.
                        if msg.get("id").and_then(|v| v.as_i64()) == Some(id) {
                            return msg;
                        }
                        on_notification(&msg);
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        panic!("connection closed before `{method}` (id={id}) response arrived")
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => panic!("WebSocket error awaiting `{method}`: {e}"),
                }
            }
        };
        let msg = tokio::time::timeout(OP_BUDGET, read)
            .await
            .unwrap_or_else(|_| panic!("`{method}` (id={id}) must answer within the hang budget"));

        if let Some(err) = msg.get("error") {
            return serde_json::json!({ "__error": err.clone() });
        }
        msg.get("result")
            .cloned()
            .unwrap_or_else(|| panic!("`{method}` response must carry a result: {msg}"))
    }

    /// ACP `initialize`. Returns the result, or `Err(message)` when the agent
    /// could not be built (e.g. the model could not be loaded) — the server
    /// answers that as a JSON-RPC error rather than a result.
    async fn initialize(&mut self) -> Result<serde_json::Value, String> {
        let id = self
            .send(
                "initialize",
                serde_json::json!({
                    "protocolVersion": 1,
                    "clientCapabilities": {
                        "fs": { "readTextFile": false, "writeTextFile": false },
                        "terminal": false
                    }
                }),
            )
            .await;
        let result = self.await_response(id, "initialize", |_| {}).await;
        if let Some(err) = result.get("__error") {
            return Err(err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string());
        }
        Ok(result)
    }

    /// ACP `session/new`. `mcp_servers` is attached via the typed request so the
    /// `mcpServers` wire shape is exactly what the agent expects. Returns the
    /// new session id string.
    async fn new_session(&mut self, mcp_servers: Vec<McpServer>) -> String {
        let mut req = NewSessionRequest::new(std::path::PathBuf::from("/tmp"));
        if !mcp_servers.is_empty() {
            req = req.mcp_servers(mcp_servers);
        }
        let params = serde_json::to_value(&req).expect("NewSessionRequest must serialize");
        let id = self.send("session/new", params).await;
        let result = self.await_response(id, "session/new", |_| {}).await;
        result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("session/new result must carry a sessionId: {result}"))
            .to_string()
    }

    /// ACP `session/prompt` with a single user text block. Collects the
    /// streamed agent text from `session/update` notifications and returns it
    /// alongside the final result.
    async fn prompt(&mut self, session_id: &str, text: &str) -> PromptOutcome {
        let params = serde_json::json!({
            "sessionId": session_id,
            "prompt": [ { "type": "text", "text": text } ],
        });
        let id = self.send("session/prompt", params).await;
        // This agent answers with a single response frame and no interleaved
        // `session/update` notifications, but tolerate any that arrive.
        let result = self.await_response(id, "session/prompt", |_| {}).await;

        if let Some(err) = result.get("__error") {
            panic!("session/prompt must not return an error: {err}");
        }
        PromptOutcome { result }
    }

    async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

/// The agent's reply text from the response `_meta.llama_response`, or panic.
fn response_text(outcome: &PromptOutcome) -> &str {
    outcome
        .result
        .pointer("/_meta/llama_response")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "prompt response _meta must carry llama_response: {}",
                outcome.result
            )
        })
}

/// `tokens_generated` from a prompt response's `_meta`, or panic. This is the
/// exact field the production agentic loop records and the AI panel relies on.
fn tokens_generated(outcome: &PromptOutcome) -> u64 {
    outcome
        .result
        .pointer("/_meta/tokens_generated")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            panic!(
                "prompt response _meta must report tokens_generated: {}",
                outcome.result
            )
        })
}

/// Skip (return true) when the agent could not be built because the model was
/// unavailable — HF rate-limit or an offline first-run download. This mirrors
/// the `try_init_agent` skip idiom used across the real-model tests; it is NOT
/// an env-var gate, and on the model-cached CI runner the model loads so the
/// test always runs its assertions.
fn is_model_unavailable(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("429")
        || m.contains("too many requests")
        || m.contains("rate limited")
        || m.contains("loadingfailed")
        || m.contains("failed to load")
}

/// Core: a real prompt over the full kanban-app stack streams tokens, and a
/// second prompt on the same session succeeds (queue released).
#[tokio::test]
async fn test_ai_panel_e2e_qwen_generates_tokens_and_second_prompt_succeeds() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let (ws_url, _server) = start_agent(resolve_qwen_test_config()).await;
    let mut client = AcpWsClient::connect(&ws_url).await;

    if let Err(e) = client.initialize().await {
        if is_model_unavailable(&e) {
            eprintln!("skipping: qwen-0.6b-test model unavailable ({e})");
            return;
        }
        panic!("initialize failed for a reason other than model availability: {e}");
    }

    let session = client.new_session(Vec::new()).await;

    // Turn 1 — must produce real, non-empty output. The "0 tokens" bug was a
    // clean call with an empty result, so both the streamed text and the
    // reported token count are asserted.
    let first = client
        .prompt(&session, "Reply with exactly the word: pong")
        .await;
    let first_tokens = tokens_generated(&first);
    assert!(
        first_tokens > 0,
        "first prompt must report tokens_generated > 0 (the 0-token regression); got {first_tokens}"
    );
    assert!(
        !response_text(&first).trim().is_empty(),
        "first prompt must produce non-empty agent text; got: {:?}",
        response_text(&first)
    );

    // Turn 2 on the same session — must not be rejected with "Queue is full".
    // A wedged first turn would leave the single worker occupied; this proves
    // it was released.
    let second = client
        .prompt(&session, "Reply with exactly the word: ping")
        .await;
    let second_tokens = tokens_generated(&second);
    assert!(
        second_tokens > 0,
        "second prompt must also generate tokens — the worker must be free after \
         turn 1 (queue-lifecycle guard); got {second_tokens}"
    );

    client.close().await;
}

/// MCP wiring: with the board's MCP server attached through the ACP
/// `mcpServers` list (the path `ai_start_agent` uses for `mcpUrl`), the model
/// reaches the board's kanban toolset and actually invokes a tool.
///
/// This is the only client-observable proof of MCP wiring: the agent never
/// sends its discovered tool list to the client (no `AvailableCommandsUpdate`
/// is emitted), so "the tools were advertised" is not observable over the wire.
/// The response meta's `tool_calls_executed` is — and it is only > 0 when the
/// board's `mcpUrl` reached the agent, the agent connected and discovered the
/// kanban tools, and the model dispatched one. A few bounded real-model
/// attempts absorb sampling variance (the same proven shape as
/// `llama-agent`'s `acp_multi_turn_dispatches_tool_and_threads_result`); a true
/// wiring break fails every attempt.
#[tokio::test]
async fn test_ai_panel_e2e_mcp_tool_reachable_in_session() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // A real board so the kanban toolset has something to operate on, and the
    // board's MCP server rooted at it — exactly as `start_board_mcp_server`
    // does in production (board_dir is the parent of `.kanban`).
    let board = tempfile::tempdir().expect("tempdir");
    init_board(board.path()).await;
    let mcp = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(board.path().to_path_buf()),
    )
    .await
    .expect("the board MCP server must start");
    let mcp_url = mcp.url().to_string();

    let (ws_url, _server) = start_agent(resolve_qwen_test_config()).await;
    let mut client = AcpWsClient::connect(&ws_url).await;

    if let Err(e) = client.initialize().await {
        if is_model_unavailable(&e) {
            eprintln!("skipping: qwen-0.6b-test model unavailable ({e})");
            return;
        }
        panic!("initialize failed for a reason other than model availability: {e}");
    }

    // `/no_think` disables Qwen3's thinking mode so the small model spends its
    // turn budget on the tool call rather than an unbounded `<think>` block —
    // the technique proven in `acp_multi_turn_dispatches_tool_and_threads_result`.
    let prompt_text = "/no_think Use the kanban tool with op \"list tasks\" to look up the \
                       board's tasks, then tell me how many tasks there are.";

    const MAX_ATTEMPTS: usize = 4;
    let mut last_tool_calls = 0u64;
    for attempt in 1..=MAX_ATTEMPTS {
        // A fresh session per attempt keeps each turn's context clean (matches
        // the sibling multi-turn test); every attempt re-attaches the board MCP.
        let session = client
            .new_session(vec![McpServer::Http(McpServerHttp::new(
                "board-mcp",
                mcp_url.clone(),
            ))])
            .await;
        let outcome = client.prompt(&session, prompt_text).await;
        last_tool_calls = outcome
            .result
            .pointer("/_meta/tool_calls_executed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if last_tool_calls >= 1 {
            client.close().await;
            return;
        }
        eprintln!("attempt {attempt}/{MAX_ATTEMPTS}: model did not call a tool yet");
    }

    panic!(
        "the model never invoked a board kanban tool in {MAX_ATTEMPTS} attempts \
         (tool_calls_executed stayed {last_tool_calls}) — the board mcpUrl wiring into \
         the agent session may be broken"
    );
}

/// Initialize a kanban board at `<dir>/.kanban` so the board MCP server's
/// kanban tools have a real board to operate on.
async fn init_board(dir: &std::path::Path) {
    use swissarmyhammer_kanban::{board::InitBoard, KanbanContext};
    use swissarmyhammer_operations::Execute;

    let ctx = KanbanContext::new(dir.join(".kanban"));
    InitBoard::new("e2e-test-board")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
}
