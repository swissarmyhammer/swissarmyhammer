//! Multi-turn tool-use round-trip tests with a real model.
//!
//! These tests verify the **agentic loop** inside `AgentServer::generate()`:
//! the model emits a tool call, llama-agent dispatches it through MCP, the
//! tool result is appended to the conversation as a tool-role message, and
//! the model is re-invoked to continue generation in the same `generate(...)`
//! call. The loop terminates cleanly when the model stops emitting tool
//! calls.
//!
//! `tool_call_round_trip.rs` only proves steps 1-3 of that flow — the model
//! emits a tool call, the chat-template engine extracts it. These tests
//! prove steps 4-7: dispatch, result feedback, continued generation, clean
//! termination.
//!
//! ## Why an in-process MCP server
//!
//! The dispatch path runs through `mcp_client.call_tool(...)`. To exercise
//! it end-to-end we need a real MCP server that can answer `read_file`.
//! `read_file_mcp_server.rs` spawns a tiny `StreamableHttpService` on a
//! loopback port — same pattern used by
//! `agent-client-protocol-extras::test_mcp_server` and
//! `swissarmyhammer-tools/src/mcp/unified_server.rs`.
//!
//! ## What the assertions prove
//!
//! Two tests, sharing the same fixture file
//! (`tests/fixtures/multi_turn/example.rs`):
//!
//! 1. **Multi-turn tool-use** — the model is asked to read the file and
//!    describe what function it defines. The final response must reference
//!    `main` (the only function in the fixture). If dispatch or result
//!    feedback is broken, the model has no way to know the file's contents
//!    and the assertion fails.
//!
//! 2. **Validator-shaped multi-turn** — a rule body that tells the model:
//!    "if the file contains `fn main`, emit
//!    `{\"status\":\"failed\",\"message\":\"main is forbidden\"}`; otherwise
//!    emit `{\"status\":\"passed\",\"message\":\"ok\"}`". The fixture
//!    contains `fn main`, so the assertion checks for `\"failed\"`. Proves
//!    the loop produces a verdict that depends on tool-result content.
//!
//! Both tests follow the existing convention in this directory
//! (`agent_cache_integration.rs`, `incremental_processing.rs`,
//! `tool_call_round_trip.rs`): live HuggingFace download, graceful skip on
//! rate-limit failures, `#[serial]` to avoid GPU/RAM contention.

use llama_agent::types::{
    AgentAPI, AgentConfig, GenerationRequest, HttpServerConfig, MCPServerConfig, Message,
    MessageRole, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
    ToolDefinition,
};
use llama_agent::AgentServer;
use serde_json::json;
use serial_test::serial;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{info, warn};

use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

use crate::integration::read_file_mcp_server::start_read_file_mcp_server;

/// Build the agent config used by these tests, pointing at the canonical
/// Qwen3-0.6B HuggingFace model. The MCP server URL is wired in by the
/// caller via `MCPServerConfig::Http` so each test can use a freshly
/// allocated loopback port.
fn create_agent_config(mcp_url: String) -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: TEST_MODEL_REPO.to_string(),
                filename: Some(TEST_MODEL_FILE.to_string()),
                folder: None,
            },
            batch_size: 64,
            use_hf_params: true,
            retry_config: RetryConfig {
                max_retries: 2,
                initial_delay_ms: 100,
                backoff_multiplier: 1.5,
                max_delay_ms: 1000,
            },
            debug: true,
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        },
        mcp_servers: vec![MCPServerConfig::Http(HttpServerConfig {
            name: "read-file-test-server".to_string(),
            url: mcp_url,
            timeout_secs: Some(30),
            sse_keep_alive_secs: None,
            stateful_mode: false,
        })],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
    }
}

/// Build the `read_file` tool descriptor that the session advertises to
/// the model. The schema mirrors the one served by `ReadFileMcpServer`
/// (single required `path` string) so the rendered prompt and the actual
/// MCP server agree on the call shape.
fn read_file_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file from the filesystem and return its contents as text.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                }
            },
            "required": ["path"]
        }),
        server_name: "read-file-test-server".to_string(),
    }
}

/// Resolve the absolute path to the multi-turn fixture file. We use an
/// absolute path so the model has an unambiguous argument to pass to the
/// `read_file` tool — relative paths would depend on the agent process's
/// cwd, which is not guaranteed across test runners.
fn fixture_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("tests")
        .join("fixtures")
        .join("multi_turn")
        .join("example.rs")
}

/// Wrap `AgentServer::initialize` with the same rate-limit-aware skip
/// behavior used by other real-model tests in this directory. Returns
/// `None` when HuggingFace is rate-limiting and the test should skip;
/// returns `Some(agent)` on success; panics on any other initialization
/// failure.
async fn initialize_or_skip(config: AgentConfig) -> Option<AgentServer> {
    match AgentServer::initialize(config).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("429")
                || error_msg.contains("too many requests")
                || error_msg.contains("rate limited")
                || error_msg.contains("loadingfailed")
            {
                warn!(
                    "Skipping test due to HuggingFace rate limiting / model load failure: {}",
                    e
                );
                None
            } else {
                panic!("AgentServer initialization failed: {}", e);
            }
        }
    }
}

/// Multi-turn tool-use round-trip:
///
/// 1. Spin up an in-process `read_file` MCP server.
/// 2. Initialize an `AgentServer` against Qwen3-0.6B with that server
///    configured.
/// 3. Create a session and attach the `read_file` tool definition so the
///    model sees it in the rendered prompt.
/// 4. Ask the model to read `tests/fixtures/multi_turn/example.rs` and
///    describe what function it defines.
/// 5. Single `agent.generate(...)` call — internally the loop should
///    parse a tool call, dispatch it via MCP, append the result, and
///    re-invoke the model.
/// 6. Assert the final response references `main` — only possible if the
///    tool result actually reached the model.
#[tokio::test]
#[serial]
async fn test_multi_turn_tool_use_round_trip_with_real_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    info!("=== MULTI-TURN TOOL-USE ROUND-TRIP TEST ===");

    let mcp_url = match start_read_file_mcp_server().await {
        Ok(url) => url,
        Err(e) => panic!("Failed to start ReadFileMcpServer: {}", e),
    };

    let config = create_agent_config(mcp_url);
    let Some(agent) = initialize_or_skip(config).await else {
        return;
    };

    let mut session = agent
        .create_session()
        .await
        .expect("create_session should succeed once the agent is initialized");

    session.available_tools = vec![read_file_tool_definition()];
    session.updated_at = SystemTime::now();

    agent
        .session_manager()
        .update_session(session.clone())
        .await
        .expect("update_session should accept tool definitions");

    let fixture = fixture_path();
    let fixture_str = fixture.to_string_lossy();
    info!("Multi-turn fixture path: {}", fixture_str);

    let user_prompt = format!(
        "Use the read_file tool to read the file at {}. After you receive the tool \
         result, tell me the name of the function defined in that file in one short sentence.",
        fixture_str
    );

    let user_message = Message {
        role: MessageRole::User,
        content: user_prompt,
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent
        .add_message(&session.id, user_message)
        .await
        .expect("add_message should succeed");

    // Generate. The model needs headroom for thinking, the tool call
    // wrapper, and the post-tool continuation.
    let request = GenerationRequest::new(session.id)
        .with_max_tokens(2048)
        .with_temperature(0.0);

    let response = agent
        .generate(request)
        .await
        .expect("generate should succeed against a healthy model");

    info!(
        "Generation completed: {} tokens, finish_reason: {:?}",
        response.tokens_generated, response.finish_reason
    );
    info!("--- generated text begin ---");
    info!("{}", response.generated_text);
    info!("--- generated text end ---");

    // Re-fetch the session to inspect the message history the loop
    // accumulated during dispatch.
    let final_session = agent
        .session_manager()
        .get_session(&session.id)
        .await
        .expect("session lookup must not error")
        .expect("session must exist after generation");

    let assistant_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .count();
    let tool_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::Tool)
        .count();
    info!(
        "Final session has {} messages ({} assistant, {} tool)",
        final_session.messages.len(),
        assistant_count,
        tool_count
    );

    // The agentic loop must have appended at least one tool-role message
    // (the read_file result) before terminating.
    assert!(
        tool_count >= 1,
        "Expected at least one tool-role message in the final session — the loop should \
         dispatch read_file and append the result. Got {} tool messages. Generated text:\n{}",
        tool_count,
        response.generated_text
    );

    // The crucial assertion: the model's final output must reference
    // `main` — the only function in the fixture. This is only possible
    // if the tool result actually reached the model.
    let lowered = response.generated_text.to_lowercase();
    assert!(
        lowered.contains("main"),
        "Expected the final response to reference `main` (the function defined in the \
         fixture). If this assertion fails, the tool result never reached the model. \
         Generated text:\n{}",
        response.generated_text
    );
}

/// Validator-shaped multi-turn round-trip.
///
/// Mirrors the runtime shape of an avp validator rule: the model is
/// asked to read a file and emit a structured pass/fail JSON verdict
/// whose value depends on the file's contents. Exercises the same code
/// path as the multi-turn test above, but adds the verdict-shape
/// assertion that real validator rules rely on.
#[tokio::test]
#[serial]
async fn test_validator_shaped_multi_turn_with_real_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    info!("=== VALIDATOR-SHAPED MULTI-TURN TEST ===");

    let mcp_url = match start_read_file_mcp_server().await {
        Ok(url) => url,
        Err(e) => panic!("Failed to start ReadFileMcpServer: {}", e),
    };

    let config = create_agent_config(mcp_url);
    let Some(agent) = initialize_or_skip(config).await else {
        return;
    };

    let mut session = agent
        .create_session()
        .await
        .expect("create_session should succeed once the agent is initialized");

    session.available_tools = vec![read_file_tool_definition()];
    session.updated_at = SystemTime::now();

    agent
        .session_manager()
        .update_session(session.clone())
        .await
        .expect("update_session should accept tool definitions");

    let fixture = fixture_path();
    let fixture_str = fixture.to_string_lossy();
    info!("Validator-shaped fixture path: {}", fixture_str);

    // Rule body deliberately mimics the avp validator rule shape: read a
    // listed file, judge it, emit a structured JSON verdict. The fixture
    // contains `fn main`, so a correctly executed pipeline must emit the
    // `failed` branch.
    let user_prompt = format!(
        "You are validating source files. The list of changed_files is: [\"{path}\"].\n\n\
         RULE: Read each file in changed_files using the read_file tool. If any file \
         contains the function `main` (i.e. the substring `fn main`), respond with the \
         JSON object {{\"status\": \"failed\", \"message\": \"main is forbidden\"}}. \
         Otherwise respond with the JSON object {{\"status\": \"passed\", \"message\": \"ok\"}}.\n\n\
         OUTPUT: After tool calls complete, output ONLY the final JSON verdict on its own \
         line. No prose, no explanation, no markdown fences. Just the JSON object.",
        path = fixture_str
    );

    let user_message = Message {
        role: MessageRole::User,
        content: user_prompt,
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent
        .add_message(&session.id, user_message)
        .await
        .expect("add_message should succeed");

    let request = GenerationRequest::new(session.id)
        .with_max_tokens(2048)
        .with_temperature(0.0);

    let response = agent
        .generate(request)
        .await
        .expect("generate should succeed against a healthy model");

    info!(
        "Generation completed: {} tokens, finish_reason: {:?}",
        response.tokens_generated, response.finish_reason
    );
    info!("--- generated text begin ---");
    info!("{}", response.generated_text);
    info!("--- generated text end ---");

    let final_session = agent
        .session_manager()
        .get_session(&session.id)
        .await
        .expect("session lookup must not error")
        .expect("session must exist after generation");

    let tool_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::Tool)
        .count();
    info!(
        "Final session has {} messages ({} tool)",
        final_session.messages.len(),
        tool_count
    );

    assert!(
        tool_count >= 1,
        "Expected at least one tool-role message — the loop should dispatch read_file. \
         Got {} tool messages. Generated text:\n{}",
        tool_count,
        response.generated_text
    );

    // The fixture contains `fn main`, so the verdict shape must include
    // `"failed"`. We use `find` to locate the JSON body rather than
    // requiring the entire output to be a single JSON value — the model
    // may emit `<think>` blocks before the verdict (Qwen3 thinking-mode
    // is the default).
    let text = &response.generated_text;
    let lowered = text.to_lowercase();

    assert!(
        lowered.contains("\"failed\""),
        "Expected the verdict to contain `\"failed\"` (the fixture contains `fn main`, \
         which the rule forbids). If this assertion fails, either the tool result never \
         reached the model or the model didn't apply the rule. Generated text:\n{}",
        text
    );

    // Negative check: we should NOT see the `passed` branch — that
    // branch is only taken when the file does not contain `fn main`,
    // which would mean the tool result did not reach the model.
    assert!(
        !lowered.contains("\"passed\"")
            || lowered.find("\"failed\"").unwrap_or(usize::MAX)
                < lowered.find("\"passed\"").unwrap_or(usize::MAX),
        "Expected the `failed` verdict to dominate. The `passed` branch appearing alone \
         would indicate the tool result didn't influence generation. Generated text:\n{}",
        text
    );
}
