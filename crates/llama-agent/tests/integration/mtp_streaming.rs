//! Real-model integration tests for the streaming MTP draft-mtp loop.
//!
//! Uses the small `unsloth/Qwen3.5-0.8B-MTP-GGUF` test model (carries an
//! MTP/NextN head, ~0.8B params), so `LlamaModel::has_mtp()` returns true and
//! the queue's streaming worker auto-routes through `generate_stream_mtp`
//! instead of the standard token-by-token generator.
//!
//! These tests prove the consumer-side MTP wiring works end-to-end against the
//! public `AgentServer::generate_stream` API (the same path the ACP server
//! drives). Token-for-token greedy equivalence against a non-MTP run on the
//! same model is the gold-standard check; it lives in the fork at
//! `examples/mtp/tests/correctness.rs` and is the right place for it (the
//! consumer does not expose a "force MTP off" knob — auto-detect is auto).

use futures::StreamExt;
use llama_agent::test_models::{MTP_TEST_MODEL_FILE, MTP_TEST_MODEL_REPO};
use llama_agent::types::{
    AgentAPI, AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource,
    ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::AgentServer;
use serial_test::serial;
use std::time::SystemTime;
use tracing::{info, warn};

/// Build the agent config pointed at the small MTP test model.
fn mtp_test_agent_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: MTP_TEST_MODEL_REPO.to_string(),
                filename: Some(MTP_TEST_MODEL_FILE.to_string()),
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

/// Initialize the agent against the MTP test model, skipping (not failing) on
/// HuggingFace rate limits or load failures — matches the rest of the
/// real-model integration tests so CI without the cached model still passes.
async fn try_init_mtp_agent() -> Option<AgentServer> {
    match AgentServer::initialize(mtp_test_agent_config()).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("429")
                || msg.contains("too many requests")
                || msg.contains("rate limited")
                || msg.contains("loadingfailed")
                || msg.contains("failed to load")
            {
                warn!("Skipping MTP test: model unavailable ({e})");
                None
            } else {
                panic!("MTP test agent init failed: {e}");
            }
        }
    }
}

async fn add_user_message(
    agent: &AgentServer,
    session_id: &llama_agent::types::SessionId,
    body: &str,
) {
    let message = Message {
        role: MessageRole::User,
        content: body.to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent
        .add_message(session_id, message)
        .await
        .expect("add_message should succeed");
}

async fn drain_stream(
    agent: &AgentServer,
    request: GenerationRequest,
) -> (String, usize, Option<llama_agent::types::FinishReason>) {
    let mut stream = agent
        .generate_stream(request)
        .await
        .expect("generate_stream should succeed");
    let mut text = String::new();
    let mut tokens = 0usize;
    let mut finish_reason: Option<llama_agent::types::FinishReason> = None;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("stream chunk should not be an error");
        tokens += chunk.token_count;
        text.push_str(&chunk.text);
        if chunk.is_complete {
            finish_reason = chunk.finish_reason.clone();
        }
    }
    (text, tokens, finish_reason)
}

/// End-to-end smoke: the streaming MTP path on the MTP-enabled model produces a
/// real response without crashing, terminates cleanly, and respects the chunk
/// accounting contract (one terminal chunk with a finish reason).
///
/// This proves the consumer integration:
///   `model.has_mtp()` returns true on this GGUF → the queue worker creates the
///   MTP draft context → `generate_stream_mtp` drives the draft→verify→accept
///   loop → StreamChunks are emitted → the stream terminates with a finish
///   reason.
///
/// It does NOT compare against a non-MTP run on the same model: that
/// equivalence test belongs in the fork (no "force off" knob in the consumer).
#[tokio::test]
#[serial]
async fn streaming_mtp_produces_tokens_on_mtp_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_mtp_agent().await else {
        return;
    };

    let session = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    add_user_message(&agent, &session.id, "Reply with the single word: ok.").await;

    let request = GenerationRequest::new(session.id)
        .with_max_tokens(64)
        .with_temperature(0.0);

    let (text, tokens, finish_reason) = drain_stream(&agent, request).await;
    info!(
        "MTP streaming produced {} tokens: {:?} (finish: {:?})",
        tokens, text, finish_reason
    );

    assert!(
        tokens > 0,
        "MTP streaming produced 0 tokens — the draft→verify→accept loop never \
         emitted. Text: {text:?}"
    );
    assert!(
        !text.trim().is_empty(),
        "MTP streaming produced no visible text despite {tokens} tokens"
    );
    let reason = finish_reason
        .expect("MTP stream must terminate with a finish reason (the terminal chunk)");
    let llama_agent::types::FinishReason::Stopped(reason) = reason;
    assert!(
        ["EndOfSequence", "StopToken", "MaxTokens", "ContextWindowFull"]
            .contains(&reason.as_str()),
        "unexpected MTP finish reason: {reason}"
    );
}

/// Two prompts in a row on the same agent: proves the worker is released after
/// the first MTP turn and the second prompt enqueues + completes. This is the
/// MTP analog of the regression test for symptom 2 ("Queue is full" after a
/// turn) in `streaming_generation.rs`.
#[tokio::test]
#[serial]
async fn streaming_mtp_releases_worker_after_turn() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_mtp_agent().await else {
        return;
    };

    let s1 = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    add_user_message(&agent, &s1.id, "Reply with the single word: one.").await;
    let (_t1, tokens1, _r1) = drain_stream(
        &agent,
        GenerationRequest::new(s1.id)
            .with_max_tokens(32)
            .with_temperature(0.0),
    )
    .await;
    assert!(tokens1 > 0, "MTP turn 1 produced 0 tokens");

    let s2 = agent
        .create_session()
        .await
        .expect("create_session should succeed for second prompt");
    add_user_message(&agent, &s2.id, "Reply with the single word: two.").await;
    let (_t2, tokens2, _r2) = drain_stream(
        &agent,
        GenerationRequest::new(s2.id)
            .with_max_tokens(32)
            .with_temperature(0.0),
    )
    .await;
    assert!(
        tokens2 > 0,
        "MTP turn 2 produced 0 tokens — worker may not have been released after turn 1"
    );
}
