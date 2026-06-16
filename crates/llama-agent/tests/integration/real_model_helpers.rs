//! Shared helpers for the real-model (Qwen3-0.6B) integration tests.
//!
//! Centralizes the canonical real-model `AgentConfig`, the server bootstrap
//! with its environmental-failure skip heuristic, and the plain-text
//! `PromptRequest` constructor, so the sibling real-model tests
//! (`acp_agentic_loop.rs`, `acp_hooks_real_model.rs`,
//! `session_fork_real_model.rs`) cannot drift apart.

use std::sync::Arc;

use agent_client_protocol::schema::SessionNotification;
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

/// Decode batch size for the test models. 64 is plenty; the tiny test models
/// prefill fast, so a larger batch buys nothing.
const TEST_MODEL_BATCH_SIZE: u32 = 64;

/// CPU thread count for the test models. 4 keeps the test models responsive on
/// CI runners without saturating a shared host running the suite in parallel.
const TEST_MODEL_THREADS: i32 = 4;

/// Model-download retry policy for the test models: a few quick retries with a
/// gentle backoff, capped low, so a transient HuggingFace hiccup self-heals
/// without a slow test hanging on a doomed download.
const TEST_MODEL_MAX_RETRIES: u32 = 2;
const TEST_MODEL_RETRY_INITIAL_DELAY_MS: u64 = 100;
const TEST_MODEL_RETRY_BACKOFF_MULTIPLIER: f64 = 1.5;
const TEST_MODEL_RETRY_MAX_DELAY_MS: u64 = 1000;

/// Build an `AgentConfig` against a HuggingFace GGUF test model. The canonical
/// per-model constructors ([`real_model_config`], [`mtp_model_config`]) call
/// this so every real-model test shares one set of model knobs (batch size,
/// threads, retry policy) and cannot drift from it. `session_config` is the one
/// real variation axis the siblings need (e.g. persistence enabled).
pub fn hf_model_config(repo: &str, filename: &str, session_config: SessionConfig) -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: repo.to_string(),
                filename: Some(filename.to_string()),
                folder: None,
            },
            batch_size: TEST_MODEL_BATCH_SIZE,
            use_hf_params: true,
            retry_config: RetryConfig {
                max_retries: TEST_MODEL_MAX_RETRIES,
                initial_delay_ms: TEST_MODEL_RETRY_INITIAL_DELAY_MS,
                backoff_multiplier: TEST_MODEL_RETRY_BACKOFF_MULTIPLIER,
                max_delay_ms: TEST_MODEL_RETRY_MAX_DELAY_MS,
            },
            debug: false,
            n_seq_max: 1,
            n_threads: TEST_MODEL_THREADS,
            n_threads_batch: TEST_MODEL_THREADS,
        },
        mcp_servers: Vec::new(),
        session_config,
        parallel_execution_config: ParallelConfig::default(),
        tool_execution_config: Default::default(),
        queue_config: QueueConfig::default(),
    }
}

/// Build the canonical `AgentConfig` against the small Qwen3-0.6B test model,
/// so every real-model test exercises the identical production setup. MCP
/// servers attach per-session via `NewSessionRequest.mcp_servers` (the ACP
/// path), not the agent-level config, so this config carries none.
pub fn real_model_config() -> AgentConfig {
    hf_model_config(TEST_MODEL_REPO, TEST_MODEL_FILE, SessionConfig::default())
}

/// Build the canonical `AgentConfig` against the small MTP test model
/// (carries an MTP/NextN head, so `LlamaModel::has_mtp()` returns true). Same
/// shared model knobs as [`real_model_config`]; the model source is the one
/// variation axis the streaming-MTP tests need.
pub fn mtp_model_config() -> AgentConfig {
    hf_model_config(
        llama_agent::test_models::MTP_TEST_MODEL_REPO,
        llama_agent::test_models::MTP_TEST_MODEL_FILE,
        SessionConfig::default(),
    )
}

/// True when `AgentServer::initialize` failed for reasons outside the test's
/// control — HuggingFace rate limiting or transient network trouble while
/// fetching the model — so the test should SKIP rather than fail.
///
/// Deliberately narrow: a `LoadingFailed` from a corrupt GGUF, a bad model
/// config, or a broken loader code path is a real regression and must PANIC,
/// never skip. `msg` is the lowercased error string.
pub fn is_environmental_model_failure(msg: &str) -> bool {
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

/// Initialize a bare `AgentServer` (no ACP wrapper) on `config`, for the
/// non-ACP real-model tests that drive `AgentServer`'s own generate API.
///
/// Same skip policy as [`build_real_model_server`]: `None` (skip) ONLY for an
/// environmental failure per [`is_environmental_model_failure`]; any other
/// initialization failure — including a genuine model-loading regression —
/// panics so it can never be silently skipped.
pub async fn try_init_real_model_agent(config: AgentConfig) -> Option<AgentServer> {
    match AgentServer::initialize(config).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            if is_environmental_model_failure(&e.to_string().to_lowercase()) {
                warn!("Skipping test: environmental model-load failure: {}", e);
                return None;
            }
            panic!("AgentServer initialization failed: {}", e);
        }
    }
}

/// Build a `PromptRequest` carrying a single user text block. Re-exported from
/// the in-crate [`llama_agent::acp::test_utils::text_prompt`] so the real-model
/// integration tests and the `server.rs` unit tests share one definition.
pub use llama_agent::acp::test_utils::text_prompt;

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
