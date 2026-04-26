//! Real-model tool-call round-trip integration test.
//!
//! Until this test existed, every llama-agent test session was constructed with
//! `available_tools: Vec::new()`. The Qwen3-0.6B test model loads and the docs
//! claim "supports tool calling," but no real-model run ever saw a tool schema
//! in its system prompt or had its output parsed for tool calls. The result was
//! a permanent "0 tool calls extracted" — and no test ever noticed.
//!
//! This test closes that gap. It loads the canonical test model, attaches a real
//! `ToolDefinition` to the session, prompts the model to use it, and asserts that
//! `ChatTemplateEngine::extract_tool_calls` returns a non-empty result with the
//! expected name and arguments.
//!
//! # Findings under the `Qwen3` strategy with Qwen3-0.6B
//!
//! After `01KQ35KFJXJ70GNB4ZPRJD6R43` landed, `unsloth/Qwen3-0.6B-GGUF` is
//! detected as `Qwen3` (not `Default`) and the chat-template engine renders
//! tools in the canonical `# Tools` block from `Qwen/Qwen3-8B`'s
//! `tokenizer_config.json`. The model still produces a `<think>...</think>`
//! reasoning block in thinking mode, but the post-think payload now lands
//! in the canonical wrapper:
//!
//! ```text
//! <think>
//!   ... reasoning ...
//! </think>
//!
//! <tool_call>
//! {"name": "read_file", "arguments": {"path": "/tmp/example.rs"}}
//! </tool_call>
//! ```
//!
//! `Qwen3ToolParser` strips the reasoning block, extracts the wrapper body,
//! and parses the JSON straight through. If the model drifts to a fallback
//! shape (markdown-fenced JSON, `function_name` direct, `call_tool`
//! improvised wrapper, etc.) the same parser still recognises it — but the
//! canonical wrapper is the intended path now that input rendering matches
//! what the model was trained on.
//!
//! This test is the integration-level proof that detection → input
//! rendering → model generation → output parsing all line up. If a future
//! Qwen3 release changes the wrapper format, this test will start failing
//! and surface that drift.

use llama_agent::types::{
    AgentAPI, AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource,
    ParallelConfig, QueueConfig, RetryConfig, SessionConfig, ToolDefinition,
};
use llama_agent::AgentServer;
use serde_json::json;
use serial_test::serial;
use std::time::SystemTime;
use tracing::{info, warn};

// Use standard test models from test_models module
use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

/// Build the agent config used by this test, pointing at the canonical Qwen3-0.6B
/// HuggingFace model. Mirrors the conventions established in
/// `tests/integration/incremental_processing.rs` and `agent_cache_integration.rs`.
fn create_test_agent_config() -> AgentConfig {
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
        mcp_servers: Vec::new(),
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
    }
}

/// Build the canonical `read_file` tool definition this test prompts the model
/// to invoke. Schema is intentionally minimal and unambiguous — a single
/// required `path` string parameter — so any reasonable tool-call format
/// emitted by the model has somewhere to land.
fn read_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file from the filesystem".to_string(),
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
        server_name: "test".to_string(),
    }
}

/// Real-model end-to-end tool-call round-trip:
///
/// 1. Initialize an `AgentServer` against the canonical Qwen3-0.6B test model.
/// 2. Create a session and attach a `read_file` tool definition via the
///    `SessionManager` (so tool rendering picks it up at prompt-build time).
/// 3. Add an unambiguous user message instructing the model to use the tool.
/// 4. Run `agent.generate(req)` — this exercises detection, input rendering,
///    model generation, and output parsing in one shot.
/// 5. Run the generated text through `ChatTemplateEngine::extract_tool_calls`
///    and assert the result is a non-empty Vec with the expected name and
///    arguments.
///
/// Follows the same convention as the other real-model integration tests in
/// this directory (`agent_cache_integration.rs`, `incremental_processing.rs`):
/// runs against a live HuggingFace download, gracefully skips on rate-limit
/// failures, and uses `#[serial]` to avoid contending with other model tests
/// for GPU/RAM.
#[tokio::test]
#[serial]
async fn test_tool_call_round_trip_with_real_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    info!("=== TOOL-CALL ROUND-TRIP TEST ===");
    info!(
        "Loading canonical test model: {} / {}",
        TEST_MODEL_REPO, TEST_MODEL_FILE
    );

    let config = create_test_agent_config();
    let agent = match AgentServer::initialize(config).await {
        Ok(agent) => {
            info!("AgentServer initialized successfully");
            agent
        }
        Err(e) => {
            // Match the convention in agent_cache_integration.rs: skip rather
            // than panic when HuggingFace rate-limits the test runner.
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
                return;
            }
            panic!("AgentServer initialization failed: {}", e);
        }
    };

    // Create a session, then attach a real `read_file` tool definition. We
    // route the update through `SessionManager::update_session` because that
    // is the same path `discover_tools` uses in production — it ensures the
    // tools are picked up at prompt-build time inside the request queue.
    let mut session = agent
        .create_session()
        .await
        .expect("create_session should succeed once the agent is initialized");

    session.available_tools = vec![read_file_tool()];
    session.updated_at = SystemTime::now();

    agent
        .session_manager()
        .update_session(session.clone())
        .await
        .expect("update_session should accept tool definitions");

    info!(
        "Session {} prepared with {} available tool(s)",
        session.id,
        session.available_tools.len()
    );

    // Single, direct instruction. We pin the path so the assertion can be
    // exact rather than approximate.
    let user_prompt = "Use the read_file tool to read the file at /tmp/example.rs.";
    let user_message = Message {
        role: MessageRole::User,
        content: user_prompt.to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent
        .add_message(&session.id, user_message)
        .await
        .expect("add_message should succeed");

    // Generate. We deliberately request a small token budget — the tool-call
    // wrapper format Qwen3 emits is short, and capping max_tokens keeps the
    // test runtime bounded.
    let request = GenerationRequest::new(session.id)
        .with_max_tokens(256)
        .with_temperature(0.0);

    let response = agent
        .generate(request)
        .await
        .expect("generate should succeed against a healthy model");

    info!(
        "Model produced {} tokens of generated text",
        response.tokens_generated
    );
    info!("--- generated text begin ---");
    info!("{}", response.generated_text);
    info!("--- generated text end ---");

    // Run the generated text through the same `extract_tool_calls` path used
    // by `AgentServer::process_tool_calls` in production.
    let tool_calls = agent
        .chat_template()
        .extract_tool_calls(&response.generated_text)
        .expect("extract_tool_calls should not error on generated text");

    info!("extract_tool_calls returned {} call(s)", tool_calls.len());

    assert!(
        !tool_calls.is_empty(),
        "Expected at least one tool call in model output, got 0. \
         Generated text was:\n{}",
        response.generated_text
    );

    let first = &tool_calls[0];
    assert_eq!(
        first.name, "read_file",
        "First tool call should target read_file, got '{}'",
        first.name
    );

    let path_value = first
        .arguments
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(
        path_value, "/tmp/example.rs",
        "Tool call arguments.path should be /tmp/example.rs, got '{}' (full args: {})",
        path_value, first.arguments
    );
}
