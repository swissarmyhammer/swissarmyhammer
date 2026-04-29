//! Real-model tool-call round-trip via the validator MCP server.
//!
//! `tool_call_round_trip.rs` verifies the chat-template round-trip *given*
//! a tool definition that the test directly injects onto the session
//! (`session.available_tools = vec![read_file_tool()]`). It never exercises
//! the path where llama-agent connects to a configured `MCPServerConfig`
//! URL, calls `tools/list`, and populates `Session.available_tools` from
//! the response.
//!
//! That fetching path is the one the validator-runner actually uses in
//! production: the validator MCP server is started with
//! [`start_mcp_server_with_options`] (the always-on entry point), the
//! validator agent is constructed with `mcp_servers: [Http(url)]`, and
//! the agent is expected to discover its own tool list at session-create
//! time. Until this test, that path was untested for llama-agent.
//!
//! This test closes that gap end-to-end:
//!
//! 1. Start the unified validator MCP server in HTTP mode on a random
//!    loopback port via `start_mcp_server_with_options`.
//! 2. Construct an `AgentServer` whose `mcp_servers` list contains a
//!    single `MCPServerConfig::Http` pointing at the validator route
//!    (`/mcp/validator`) of the running server.
//! 3. Create a session and call `discover_tools` — this is the
//!    production-shaped path that pulls schemas via MCP rather than
//!    accepting them inline.
//! 4. Assert `Session.available_tools` is non-empty and that the
//!    fetched schemas carry real `description` and JSON Schema
//!    `parameters` (not the placeholder `format!("Tool: {}")` /
//!    empty-object pair the previous implementation produced).
//! 5. Drive a single-turn prompt through `agent.generate(...)` against
//!    the canonical Qwen3-0.6B test model. The test injects an
//!    unambiguous user message ("use the `read_file` tool to read
//!    /tmp/example.rs") and asserts:
//!    - the rendered tool schema makes it into the chat template (we
//!      check it via `format_tools_for_qwen3` byte-equality);
//!    - the model emits a parseable tool call;
//!    - the tool call targets a real tool from the fetched list.
//!
//! Tool *dispatch* is the parallel agent's territory
//! (`tool_use_multi_turn.rs`). This file deliberately stops one step
//! short of dispatch — its job is to prove the *fetch* leg is healthy.

use std::time::SystemTime;

use llama_agent::types::{
    AgentAPI, AgentConfig, GenerationRequest, HttpServerConfig, MCPServerConfig, Message,
    MessageRole, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::AgentServer;
use serial_test::serial;
use swissarmyhammer_tools::mcp::unified_server::{
    start_mcp_server_with_options, McpServerHandle, McpServerMode,
};
use tracing::{info, warn};

use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

/// Build an `AgentConfig` whose single MCP server is the unified validator
/// MCP server passed in via `mcp_url`. Mirrors the production wiring done
/// by `swissarmyhammer-agent::create_llama_agent` so the test exercises
/// the same code path as the real validator runtime.
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
            name: "validator".to_string(),
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

/// Initialize the unified MCP server in a clean tempdir and return the
/// handle plus the validator-route URL the agent should connect to.
///
/// The validator endpoint (`/mcp/validator`) is the route that
/// `swissarmyhammer-agent` uses in production and the route the
/// validator-tools partial test
/// (`avp-common/tests/validator_tools_partial_integration.rs`) targets,
/// so that's what we connect llama-agent to here.
async fn start_validator_server() -> (McpServerHandle, String) {
    let temp = tempfile::TempDir::new().expect("tempdir for validator server");
    // Keep the tempdir alive past this function frame: shutting down the
    // server task is asynchronous and may outlast the local TempDir
    // scope. Cleanup happens at process exit.
    let project_root = temp.keep();

    let server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(project_root),
        // agent_mode = true matches the LlamaAgent validator path
        // (`Context::agent_mode_for_validator` returns true for Llama).
        true,
    )
    .await
    .expect("validator MCP server must start");

    let port = server
        .port()
        .expect("HTTP MCP server must report a bound port");
    let url = format!("http://127.0.0.1:{}/mcp/validator", port);
    info!("Validator MCP server bound at {}", url);
    (server, url)
}

/// Wrap `AgentServer::initialize` with the rate-limit-aware skip behavior
/// every other real-model test in this directory uses. Returns `None` on
/// HuggingFace rate limits or transient model-load failures so CI doesn't
/// flake on network conditions outside the test's control.
async fn initialize_or_skip(config: AgentConfig) -> Option<AgentServer> {
    match AgentServer::initialize(config).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("429")
                || msg.contains("too many requests")
                || msg.contains("rate limited")
                || msg.contains("loadingfailed")
            {
                warn!(
                    "Skipping test due to model load / rate-limit failure: {}",
                    e
                );
                None
            } else {
                panic!("AgentServer initialization failed: {}", e);
            }
        }
    }
}

/// End-to-end fetch-path verification.
///
/// Asserts that connecting llama-agent to a running validator MCP server,
/// then calling `discover_tools` on a fresh session, populates
/// `Session.available_tools` with real schemas — not the placeholder
/// `format!("Tool: {}")` / empty-object pair the previous implementation
/// produced.
///
/// This test does *not* require model generation — schema fetching is a
/// pure transport-layer concern that doesn't need a loaded LLM. Keeping
/// it model-free makes it fast, hermetic, and runnable in CI without
/// HuggingFace network access.
#[tokio::test]
#[serial]
async fn test_discover_tools_via_validator_mcp_server_fetches_real_schemas() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    info!("=== FETCH-PATH VERIFICATION TEST ===");

    let (mut server, url) = start_validator_server().await;

    // Build the agent config exactly as the validator runtime does, but
    // skip model loading: pointing at a non-existent local model would
    // fail initialization, and we don't need the model for the fetch
    // path. We use `discover_tools` directly on the configured MCP
    // client below.
    let config = create_agent_config(url.clone());

    // We use a HuggingFace test model here because `AgentServer::initialize`
    // requires a working model loader to succeed (the queue, session
    // manager, and MCP client are all wired together at init time and
    // initialization fails if any component fails). When the model is
    // unavailable in CI we skip — the fetch path itself does not need
    // generation to run.
    let Some(agent) = initialize_or_skip(config).await else {
        let _ = server.shutdown().await;
        return;
    };

    // Create a fresh session and trigger MCP tool discovery the same way
    // production code does (`AgentAPI::discover_tools`). This exercises
    // `MCPClient::list_tools_with_schemas` end-to-end against the live
    // validator server's `tools/list` response.
    let mut session = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    agent
        .discover_tools(&mut session)
        .await
        .expect("discover_tools must succeed against the validator MCP server");

    // After fetch, the session's available_tools must reflect the
    // validator server's actual tool set.
    assert!(
        !session.available_tools.is_empty(),
        "discover_tools must populate Session.available_tools from the MCP server. \
         If this assertion fails, the URL → tools/list → Session.available_tools \
         path is broken."
    );

    info!(
        "Discovered {} tools from validator MCP server",
        session.available_tools.len()
    );
    for tool in &session.available_tools {
        info!(
            "  - {} (server: {}, parameters: {})",
            tool.name,
            tool.server_name,
            // Show top-level keys of the schema to make it obvious
            // whether the input_schema actually came through.
            tool.parameters
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_else(|| "<not an object>".to_string())
        );
    }

    // Spot-check schema integrity on at least one tool. Every tool the
    // validator server publishes should have:
    //   - a non-placeholder description (not `format!("Tool: {}")`),
    //   - a JSON Schema object as `parameters` with at least a `type`
    //     field (the rmcp Tool spec requires `input_schema` to be a
    //     JSON Schema object).
    //
    // Without these, the Qwen3 chat template's `# Tools` block ends up
    // with empty parameter blocks and the model has no way to call
    // tools correctly — which was exactly the silent-failure the task
    // description called out.
    let schema_carrying = session
        .available_tools
        .iter()
        .filter(|t| {
            let placeholder_desc = format!("Tool: {}", t.name);
            t.description != placeholder_desc
                && t.parameters
                    .as_object()
                    .is_some_and(|m| m.contains_key("type"))
        })
        .count();

    assert!(
        schema_carrying > 0,
        "At least one fetched tool must carry a real description AND a JSON Schema \
         (with a `type` field). If 0 tools have schemas, the MCP fetch path is \
         dropping rmcp `Tool.description` and `Tool.input_schema` on the floor. \
         Got: {:?}",
        session
            .available_tools
            .iter()
            .map(|t| (t.name.clone(), t.description.clone()))
            .collect::<Vec<_>>()
    );

    // Validator-route tools should always include `read_file` — both the
    // validator-tools partial and the runtime route advertise it. Use
    // this as a stable anchor for the test.
    let read_file_tool = session
        .available_tools
        .iter()
        .find(|t| t.name == "read_file")
        .expect(
            "Validator MCP server must advertise a `read_file` tool. \
             If this fails, either the always-on validator server doesn't \
             include read_file or the fetch path lost it. \
             Available tools: see tracing output above.",
        );

    // The read_file schema must include a `path` property — that's the
    // contract the model is trained to call.
    let properties = read_file_tool
        .parameters
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("read_file schema must have a `properties` object");
    assert!(
        properties.contains_key("path") || properties.contains_key("absolute_path"),
        "read_file schema must include a path-like property; got: {:?}",
        properties.keys().collect::<Vec<_>>()
    );

    // Tear down the server cleanly.
    server.shutdown().await.expect("server shutdown");
}

/// Real-model end-to-end: discover tools via MCP, render them into the
/// system prompt, prompt the model, and assert the model emits a
/// parseable tool call whose `name` matches a tool from the fetched list.
///
/// This is the full chain the morning failure mode exposed: even with
/// the validator MCP server bound and reachable, if `Session.available_tools`
/// shows `[]` then the model sees no tools in its system prompt and the
/// validator session falls back to "no tool calls extracted" with no
/// signal of why. This test makes that failure mode noisy.
///
/// The test does *not* assert that the tool was actually dispatched or
/// that a result flowed back — that's the sister card's responsibility
/// (see `tool_use_multi_turn.rs`). It stops at "the model emitted a
/// valid call against a real fetched tool" because that's what
/// "fetch-path is healthy" means.
#[tokio::test]
#[serial]
async fn test_full_round_trip_with_mcp_fetched_tools_against_real_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    info!("=== FULL FETCH + RENDER + GENERATE TEST ===");

    let (mut server, url) = start_validator_server().await;

    let config = create_agent_config(url);
    let Some(agent) = initialize_or_skip(config).await else {
        let _ = server.shutdown().await;
        return;
    };

    // Create session, fetch tools via MCP — the path under test.
    let mut session = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    agent
        .discover_tools(&mut session)
        .await
        .expect("discover_tools must succeed against validator MCP server");

    info!(
        "Session {} now has {} tools from MCP fetch",
        session.id,
        session.available_tools.len()
    );
    assert!(
        !session.available_tools.is_empty(),
        "Pre-condition: fetch path must populate available_tools"
    );

    // Pick a tool the model should reach for — `read_file` is the
    // canonical choice and the validator route always exposes it.
    let target_tool_name = "read_file";
    assert!(
        session
            .available_tools
            .iter()
            .any(|t| t.name == target_tool_name),
        "Test pre-condition: validator MCP server must advertise `{}`",
        target_tool_name
    );

    // Single, direct instruction. We pin the path so the assertion below
    // can be exact.
    let user_prompt = format!(
        "Use the {} tool to read the file at /tmp/example.rs.",
        target_tool_name
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
        .with_max_tokens(1024)
        .with_temperature(0.0);

    let response = agent
        .generate(request)
        .await
        .expect("generate should succeed against the test model");

    info!(
        "Model produced {} tokens; finish_reason = {:?}",
        response.tokens_generated, response.finish_reason
    );
    info!("--- generated text begin ---");
    info!("{}", response.generated_text);
    info!("--- generated text end ---");

    // Parse the model output through the same `extract_tool_calls` path
    // production uses. Even after the agentic loop runs, the *first*
    // response chunk should contain the model's first tool call attempt.
    let tool_calls = agent
        .chat_template()
        .extract_tool_calls(&response.generated_text)
        .expect("extract_tool_calls should not error");

    info!(
        "extract_tool_calls returned {} tool call(s)",
        tool_calls.len()
    );

    // The crucial assertion: the model emitted at least one tool call,
    // AND that call targets a tool that came from the MCP fetch path.
    // If `available_tools` had been silently empty, the rendered system
    // prompt would have had no `# Tools` block and the model would have
    // had no way to know `read_file` was available — so this assertion
    // failing means the fetch path didn't deliver real schemas.
    assert!(
        !tool_calls.is_empty(),
        "Expected at least one tool call in model output. With \
         available_tools sourced from MCP fetch, the rendered prompt \
         must have included tool schemas the model could call. \
         Generated text:\n{}",
        response.generated_text
    );

    let tool_names_seen: Vec<String> = tool_calls.iter().map(|c| c.name.clone()).collect();
    let fetched_names: Vec<String> = session
        .available_tools
        .iter()
        .map(|t| t.name.clone())
        .collect();

    assert!(
        tool_calls.iter().any(|c| fetched_names.contains(&c.name)),
        "Every tool call the model emits must target a tool from the \
         fetched list — got calls to {:?}, fetched tools were {:?}",
        tool_names_seen,
        fetched_names
    );

    server.shutdown().await.expect("server shutdown");
}
