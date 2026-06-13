//! Shared helpers for the real-model (Qwen3-0.6B) integration tests.
//!
//! Centralizes the canonical real-model `AgentConfig`, the server bootstrap
//! with its environmental-failure skip heuristic, and the plain-text
//! `PromptRequest` constructor, so the sibling real-model tests
//! (`acp_agentic_loop.rs`, `acp_hooks_real_model.rs`,
//! `session_fork_real_model.rs`) cannot drift apart.

use std::sync::Arc;

use agent_client_protocol::schema::{PromptRequest, SessionId, SessionNotification};
use llama_agent::acp::config::AcpConfig;
use llama_agent::acp::AcpServer;
use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
use llama_agent::types::{
    AgentAPI, AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
    SessionConfig,
};
use llama_agent::AgentServer;
use tokio::sync::broadcast::Receiver;
use tracing::warn;

/// Build the canonical `AgentConfig` against the small Qwen3-0.6B test model,
/// so every real-model test exercises the identical production setup. MCP
/// servers attach per-session via `NewSessionRequest.mcp_servers` (the ACP
/// path), not the agent-level config, so this config carries none.
pub fn real_model_config() -> AgentConfig {
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
            debug: false,
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

/// True when `AgentServer::initialize` failed for reasons outside the test's
/// control — HuggingFace rate limiting or transient network trouble while
/// fetching the model — so the test should SKIP rather than fail.
///
/// Deliberately narrow: a `LoadingFailed` from a corrupt GGUF, a bad model
/// config, or a broken loader code path is a real regression and must PANIC,
/// never skip. `msg` is the lowercased error string.
fn is_environmental_model_failure(msg: &str) -> bool {
    const ENVIRONMENTAL_SIGNATURES: &[&str] = &[
        // HuggingFace rate limiting.
        "429",
        "too many requests",
        "rate limited",
        // Transient network trouble while downloading the model.
        "timed out",
        "timeout",
        "connection",
        "network",
        "dns error",
    ];
    ENVIRONMENTAL_SIGNATURES.iter().any(|sig| msg.contains(sig))
}

/// Build an `AcpServer` on a fully-initialized real-model `AgentServer`
/// (model loaded), returning the server plus the notification receiver.
///
/// Returns `None` (skip) only for environmental failures per
/// [`is_environmental_model_failure`]; any other initialization failure —
/// including genuine model-loading regressions — panics so it can never be
/// silently skipped. Callers that don't observe notifications discard the
/// receiver: `let Some((server, _rx)) = build_real_model_server(..)`.
pub async fn build_real_model_server(
    config: AgentConfig,
) -> Option<(Arc<AcpServer>, Receiver<SessionNotification>)> {
    let agent = match AgentServer::initialize(config).await {
        Ok(agent) => agent,
        Err(e) => {
            if is_environmental_model_failure(&e.to_string().to_lowercase()) {
                warn!("Skipping test: environmental model-load failure: {}", e);
                return None;
            }
            panic!("AgentServer initialization failed: {}", e);
        }
    };

    let mount = Arc::new(llama_agent::InProcessMount::new(
        llama_agent::echo::EchoService::new(),
    ));
    let (server, rx) = AcpServer::new(Arc::new(agent), AcpConfig::default(), mount);
    Some((Arc::new(server), rx))
}

/// Build a `PromptRequest` carrying a single user text block.
pub fn text_prompt(session_id: SessionId, text: &str) -> PromptRequest {
    PromptRequest::new(
        session_id,
        vec![agent_client_protocol::schema::ContentBlock::from(
            text.to_string(),
        )],
    )
}

/// The skip heuristic must stay narrow: rate-limit/network failures are
/// environmental (skip), but a `LoadingFailed` from a corrupt model or broken
/// loader is a regression the suite must fail on, not silently skip.
#[test]
fn environmental_skip_is_narrow() {
    for msg in [
        "request error: status code 429",
        "request error: too many requests",
        "huggingface download rate limited",
        "request error: operation timed out",
        "request error: connection reset by peer",
        "network is unreachable",
        "dns error: failed to lookup address",
    ] {
        assert!(
            is_environmental_model_failure(msg),
            "{msg:?} is environmental and must skip"
        );
    }

    for msg in [
        "loadingfailed: failed to parse gguf header",
        "model loading failed: unknown model architecture",
        "loadingfailed: model not loaded",
    ] {
        assert!(
            !is_environmental_model_failure(msg),
            "{msg:?} is a real loading regression and must panic, not skip"
        );
    }
}
