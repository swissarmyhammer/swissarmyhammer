//! Real-model streaming-generation regression tests.
//!
//! These tests reproduce and lock down bug `01KSNJ7CBK9333J0T9G4TCA7DH`:
//! "qwen produces 0 tokens on first prompt; retry hits 'Queue is full'".
//!
//! # Why a dedicated streaming test
//!
//! Every prior real-model test (`tool_call_round_trip.rs`,
//! `tool_use_multi_turn.rs`, …) drives the **batch** path via
//! `AgentServer::generate()`. The in-app AI panel and the ACP agentic loop in
//! `acp/server.rs` instead drive the **streaming** path via
//! `AgentServer::generate_stream()`. The two paths used to diverge in how they
//! computed the generation budget:
//!
//! - batch (`GenerationHelper::generate_common`): `max_tokens` used directly.
//! - streaming (`generate_stream_with_borrowed_model`): `max_tokens -
//!   tokens_list.len()` — it subtracted the prompt length from the caller's
//!   budget a second time.
//!
//! The ACP loop already budgets `max_tokens` as "remaining context" =
//! `context_size - current_tokens`. When the rendered prompt is large (a real
//! system prompt plus tool schemas), `caller_budget - prompt_len` underflowed
//! (usize) or collapsed to a tiny/zero value, so the streaming loop generated
//! zero tokens while the batch path was healthy. No streaming real-model test
//! existed, so the regression was invisible.
//!
//! These tests close that gap by exercising the live streaming path against the
//! canonical Qwen3-0.6B test model and asserting a non-empty response with
//! `tokens_generated > 0`, plus a second prompt on the same agent to prove the
//! single worker is released after a turn (no "Queue is full").

use futures::StreamExt;
use llama_agent::types::{
    AgentAPI, AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource,
    ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::AgentServer;
use serial_test::serial;
use std::time::SystemTime;
use tracing::{info, warn};

use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

/// Build the agent config used by these tests, pointing at the canonical
/// Qwen3-0.6B HuggingFace model. Mirrors `tool_call_round_trip.rs`.
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

/// Initialize the agent, skipping (rather than failing) when HuggingFace
/// rate-limits the test runner — matching the convention in the other
/// real-model integration tests.
async fn try_init_agent() -> Option<AgentServer> {
    match AgentServer::initialize(create_test_agent_config()).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("429")
                || error_msg.contains("too many requests")
                || error_msg.contains("rate limited")
                || error_msg.contains("loadingfailed")
            {
                warn!("Skipping test due to HuggingFace rate limiting: {}", e);
                None
            } else {
                panic!("AgentServer initialization failed: {}", e);
            }
        }
    }
}

/// Drain a streaming generation to completion, returning the concatenated text
/// and the total token count summed from the chunks — exactly how the ACP
/// agentic loop in `acp/server.rs` accumulates `turn_tokens`.
async fn drain_stream(agent: &AgentServer, request: GenerationRequest) -> (String, usize) {
    let mut stream = agent
        .generate_stream(request)
        .await
        .expect("generate_stream should succeed against a healthy model");

    let mut text = String::new();
    let mut tokens = 0usize;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("stream chunk should not be an error");
        tokens += chunk.token_count;
        text.push_str(&chunk.text);
    }
    (text, tokens)
}

/// Add a single user message to the session.
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

/// Add the assistant's own turn (its raw generated text) back to the session.
/// This is what the ACP agentic loop does between turns; it keeps the rendered
/// conversation append-only so the next turn's prompt EXTENDS the tokens already
/// in the KV cache rather than diverging from them.
async fn add_assistant_message(
    agent: &AgentServer,
    session_id: &llama_agent::types::SessionId,
    body: &str,
) {
    let message = Message {
        role: MessageRole::Assistant,
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

/// A `tracing` writer that appends everything to a shared byte buffer so a test
/// can assert on log lines emitted by the queue worker (which runs on a
/// different thread than the test — a thread-local/`with_test_writer` capture
/// would miss it, so this installs a *global* subscriber).
#[derive(Clone)]
struct SharedLogBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for SharedLogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
    type Writer = SharedLogBuffer;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Regression test for symptom 1 (0 tokens generated).
///
/// Drives the streaming path with a `max_tokens` budget that mirrors what the
/// ACP agentic loop passes: a "remaining context" figure that is comparable to
/// — not strictly larger than — the rendered prompt length. Before the fix, the
/// streaming generator subtracted the prompt length from this budget and
/// produced zero tokens. After the fix it must produce a real response.
#[tokio::test]
#[serial]
async fn test_streaming_generation_produces_tokens() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    let session = agent
        .create_session()
        .await
        .expect("create_session should succeed");

    add_user_message(&agent, &session.id, "Say hello in one short sentence.").await;

    // The ACP loop computes max_tokens as remaining context space. We pass a
    // deliberately modest budget so that, under the old `budget - prompt_len`
    // arithmetic, the streaming loop would have produced zero tokens. A healthy
    // streaming path must still generate a real response.
    let request = GenerationRequest::new(session.id)
        .with_max_tokens(256)
        .with_temperature(0.0);

    let (text, tokens) = drain_stream(&agent, request).await;

    info!("Streaming produced {} tokens: {:?}", tokens, text);

    assert!(
        tokens > 0,
        "Streaming generation produced 0 tokens — the agentic loop would report \
         an empty turn. Generated text was: {:?}",
        text
    );
    assert!(
        !text.trim().is_empty(),
        "Streaming generation produced no visible text despite {} tokens",
        tokens
    );
}

/// Direct regression for the underflow that produced symptom 1 in production.
///
/// The bug appeared when the rendered prompt was large relative to the
/// caller-supplied `max_tokens` budget — the streaming generator computed
/// `max_tokens - prompt_len`, which underflowed (usize) in release and panicked
/// in debug. This test sends a deliberately long prompt together with a small
/// `max_tokens` so `max_tokens < prompt_len`. Before the fix this path produced
/// zero tokens (or panicked); after the fix it must produce a real, bounded
/// response.
#[tokio::test]
#[serial]
async fn test_streaming_with_prompt_larger_than_max_tokens() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    let session = agent
        .create_session()
        .await
        .expect("create_session should succeed");

    // Build a long prompt (well over 64 tokens) so it exceeds the small
    // max_tokens budget below and would have triggered the `budget - prompt_len`
    // underflow.
    let long_prompt = format!(
        "Here is some context you must read carefully before answering. {} \
         Now, in one short sentence, summarize the topic.",
        "The quick brown fox jumps over the lazy dog. ".repeat(40)
    );
    add_user_message(&agent, &session.id, &long_prompt).await;

    // Budget smaller than the prompt length — the exact trigger condition.
    let request = GenerationRequest::new(session.id)
        .with_max_tokens(64)
        .with_temperature(0.0);

    let (text, tokens) = drain_stream(&agent, request).await;

    info!(
        "Large-prompt streaming produced {} tokens: {:?}",
        tokens, text
    );

    assert!(
        tokens > 0,
        "Streaming with a prompt larger than max_tokens produced 0 tokens — the \
         budget arithmetic underflowed. Generated text was: {:?}",
        text
    );
    assert!(
        tokens <= 64,
        "Streaming generated {} tokens, exceeding the max_tokens budget of 64",
        tokens
    );
}

/// Pin the completion-reason and per-chunk token-accounting contract.
///
/// The ACP agentic loop relies on two stream invariants:
/// 1. Exactly one terminal chunk arrives last, with `is_complete == true` and a
///    `finish_reason` set (it is how the loop knows the turn ended and why).
/// 2. Per-token chunks each carry `token_count == 1` and the terminal chunk
///    carries `token_count == 0`, so summing `token_count` across the whole
///    stream equals the number of text-bearing chunks — never double-counts.
///
/// The "0-token" bug shipped partly because nothing asserted this contract on
/// the live streaming path; a healthy turn must satisfy both invariants.
#[tokio::test]
#[serial]
async fn test_streaming_completion_reason_and_chunk_accounting() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    let session = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    add_user_message(&agent, &session.id, "Reply with the single word: ok.").await;

    let mut stream = agent
        .generate_stream(
            GenerationRequest::new(session.id)
                .with_max_tokens(64)
                .with_temperature(0.0),
        )
        .await
        .expect("generate_stream should succeed");

    let mut text_chunks = 0usize;
    let mut summed_tokens = 0usize;
    let mut terminal_chunks = 0usize;
    let mut finish_reason: Option<llama_agent::types::FinishReason> = None;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("stream chunk should not be an error");
        summed_tokens += chunk.token_count;
        if chunk.is_complete {
            terminal_chunks += 1;
            finish_reason = chunk.finish_reason.clone();
            assert_eq!(
                chunk.token_count, 0,
                "terminal completion chunk must carry 0 new tokens"
            );
        } else {
            text_chunks += 1;
            assert_eq!(
                chunk.token_count, 1,
                "each per-token chunk must carry exactly 1 token"
            );
        }
    }

    info!(
        "completion: {} text chunks, summed_tokens={}, finish_reason={:?}",
        text_chunks, summed_tokens, finish_reason
    );

    assert_eq!(
        terminal_chunks, 1,
        "exactly one terminal (is_complete) chunk must close the stream"
    );
    assert_eq!(
        summed_tokens, text_chunks,
        "sum of per-chunk token_count must equal the number of text-bearing chunks"
    );
    assert!(text_chunks > 0, "a healthy turn must produce text chunks");

    let llama_agent::types::FinishReason::Stopped(reason) =
        finish_reason.expect("terminal chunk must carry a finish reason");
    // The single short reply ends either at EOS (model emits a stop) or, for the
    // tiny test model, by hitting the max-tokens budget — both are valid
    // terminal reasons produced by the live decode loop's completion path.
    assert!(
        [
            "EndOfSequence",
            "StopToken",
            "MaxTokens",
            "ContextWindowFull"
        ]
        .contains(&reason.as_str()),
        "unexpected finish reason: {reason}"
    );
}

/// Regression test for symptom 2 (queue jamming after a turn).
///
/// After one streaming turn completes, the single worker must be released so a
/// second prompt on the same agent enqueues and generates successfully. Before
/// the fix, a wedged first turn left the worker occupied and the retry was
/// rejected with "Queue is full".
#[tokio::test]
#[serial]
async fn test_second_streaming_prompt_after_turn_succeeds() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    // First turn.
    let session1 = agent
        .create_session()
        .await
        .expect("create_session should succeed");
    add_user_message(&agent, &session1.id, "Reply with the single word: one.").await;
    let (_text1, tokens1) = drain_stream(
        &agent,
        GenerationRequest::new(session1.id)
            .with_max_tokens(256)
            .with_temperature(0.0),
    )
    .await;
    assert!(tokens1 > 0, "First streaming turn produced 0 tokens");

    // Second turn — must not be rejected with "Queue is full". A separate
    // session proves the worker drained and the queue freed after turn 1.
    let session2 = agent
        .create_session()
        .await
        .expect("create_session should succeed for second prompt");
    add_user_message(&agent, &session2.id, "Reply with the single word: two.").await;
    let (_text2, tokens2) = drain_stream(
        &agent,
        GenerationRequest::new(session2.id)
            .with_max_tokens(256)
            .with_temperature(0.0),
    )
    .await;

    assert!(
        tokens2 > 0,
        "Second streaming prompt produced 0 tokens — worker may not have been \
         released after the first turn"
    );
}

/// Production-path proof that the **streaming** path reuses the session KV cache
/// across turns (prompt prefix caching), instead of re-prefilling the whole
/// prompt every turn.
///
/// # Why this test exists
///
/// The ACP agentic loop and the in-app AI panel drive `generate_stream()`. That
/// path used to pass `None` for the template offset and never restore/save the
/// llama.cpp context state — so every turn re-tokenized and re-decoded the
/// ENTIRE prompt (system prompt + all tool schemas + full history) from token 0.
/// With a large tool set that prefill dominates wall-clock and is why local
/// generation felt far slower than a hosted model that caches the prompt prefix.
/// The batch path (`generate()`) cached correctly; only streaming did not, and
/// no test covered it.
///
/// # What it asserts (deterministically, not via timing)
///
/// Two streaming turns on the same session, appended append-only (assistant turn
/// + new user turn between them, exactly as the ACP loop does). It captures the
/// queue worker's own logs and asserts:
///   - turn 1 logged a cold start ("no cached KV state … processing full
///     prompt") — the save side ran, populating the cache, and
///   - turn 2 logged "streaming reusing N cached tokens" — the restore side ran
///     and skipped the cached prefix.
///
/// That pair proves the save→restore lifecycle is wired on the streaming path.
///
/// The worker logs from a different thread, so this installs a global tracing
/// subscriber. Under the project's runner (nextest, process-per-test) that
/// always succeeds; if some other test already owns the global subscriber in a
/// shared-process run, we skip rather than report a false failure (matching the
/// rate-limit skip convention used throughout these tests).
#[tokio::test]
#[serial]
async fn test_streaming_reuses_kv_cache_across_turns() {
    let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let installed = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(SharedLogBuffer(buffer.clone()))
        .try_init()
        .is_ok();
    if !installed {
        warn!(
            "Skipping KV-reuse test: a global tracing subscriber is already \
             installed (run under nextest for per-process isolation)."
        );
        return;
    }

    let Some(agent) = try_init_agent().await else {
        return;
    };

    let session = agent
        .create_session()
        .await
        .expect("create_session should succeed");

    // Turn 1 — cold start. No cached KV state yet, so the worker must process the
    // full prompt and then save the resulting context state.
    add_user_message(
        &agent,
        &session.id,
        "Tell me one short fact about the moon.",
    )
    .await;
    let (text1, tokens1) = drain_stream(
        &agent,
        GenerationRequest::new(session.id)
            .with_max_tokens(64)
            .with_temperature(0.0),
    )
    .await;
    assert!(tokens1 > 0, "turn 1 must produce tokens, got 0: {text1:?}");

    // Append the assistant's turn and a new user turn so turn 2's prompt is a
    // strict EXTENSION of turn 1's (system + user1 + assistant1 + user2). This is
    // the precondition for a valid cached prefix, and is exactly what the ACP
    // loop now persists between turns.
    add_assistant_message(&agent, &session.id, &text1).await;
    add_user_message(&agent, &session.id, "Now one short fact about the sun.").await;

    // Turn 2 — warm. The cached state from turn 1 must be restored and the
    // already-processed prefix skipped.
    let (text2, tokens2) = drain_stream(
        &agent,
        GenerationRequest::new(session.id)
            .with_max_tokens(64)
            .with_temperature(0.0),
    )
    .await;
    assert!(tokens2 > 0, "turn 2 must produce tokens, got 0: {text2:?}");

    let logs = String::from_utf8_lossy(&buffer.lock().unwrap()).into_owned();

    assert!(
        logs.contains("no cached KV state"),
        "turn 1 should log a cold start (no cached KV state) — proving the cache \
         began empty. Captured logs:\n{logs}"
    );
    assert!(
        logs.contains("streaming reusing"),
        "turn 2 should log 'streaming reusing N cached tokens' — proving the \
         streaming path restored the prior turn's KV cache instead of \
         re-prefilling the whole prompt. Without this fix the streaming path \
         passed None and never restored/saved state. Captured logs:\n{logs}"
    );
}
