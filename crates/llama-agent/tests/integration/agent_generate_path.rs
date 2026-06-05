//! Real-model coverage for `AgentServer`'s own (non-ACP) generate-path helpers.
//!
//! `agent.rs` carries an API surface distinct from the ACP `prompt` loop:
//! `generate_session_title` / `title_via_model`. The fallback (no-model /
//! model-error) branch is unit-tested in `acp/server.rs`; the **success**
//! branch — the model actually produces a title that `normalize_title`
//! shapes — needs a real model and is exercised here against the small
//! Qwen3-0.6B test model.

use llama_agent::types::{
    AgentAPI, AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
    SessionConfig,
};
use llama_agent::AgentServer;
use serial_test::serial;
use tracing::{info, warn};

use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

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
        tool_execution_config: Default::default(),
        queue_config: QueueConfig::default(),
    }
}

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

/// `generate_session_title` success branch: a real model call produces a
/// non-empty title for a non-empty first user message.
///
/// This drives `title_via_model` end-to-end — render title prompt → create
/// context → `GenerationHelper::generate_text_with_borrowed_model` → normalize —
/// and the `Ok(Some(title))` arm of `generate_session_title`. The result is
/// normalized (whitespace collapsed, capped to the title length), so we assert a
/// bounded, non-empty title rather than exact text the tiny model can't pin.
#[tokio::test]
#[serial]
async fn test_generate_session_title_success_branch() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    let title = agent
        .generate_session_title("Help me write a Python script to rename files in a folder.")
        .await;

    info!("generated title: {:?}", title);

    let title = title.expect("a non-empty first user message must yield Some(title)");
    assert!(
        !title.trim().is_empty(),
        "title-via-model success branch must produce a non-empty title"
    );
    // normalize_title caps the title length; a sane title is not a runaway
    // paragraph. The cap is generous; this just guards against an unnormalized
    // full generation leaking through.
    assert!(
        title.chars().count() <= 120,
        "normalized title should be short, got {} chars: {:?}",
        title.chars().count(),
        title
    );
}

/// Empty/whitespace first message must short-circuit to `None` WITHOUT a model
/// call — the guard at the top of `generate_session_title`. This pins the
/// early-return arm that the success test deliberately steps over.
#[tokio::test]
#[serial]
async fn test_generate_session_title_empty_message_returns_none() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_agent().await else {
        return;
    };

    assert!(
        agent.generate_session_title("   ").await.is_none(),
        "whitespace-only first message must yield None (no title to make)"
    );
}
