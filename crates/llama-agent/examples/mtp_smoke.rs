//! End-to-end smoke runner for the streaming MTP draft-mtp path.
//!
//! Loads a chosen MTP-capable model, drives a single prompt through the public
//! `AgentServer::generate_stream` API (the same path ACP and the kanban app
//! use), and prints every `StreamChunk` plus the finish reason and basic
//! timing. Lets you iterate on the MTP integration without a 35B unit test.
//!
//! ## Run
//!
//! Default: the small Qwen3.5-0.8B MTP test model.
//!
//! ```sh
//! cargo run --release --example mtp_smoke
//! ```
//!
//! Pass an HF repo + filename to use a different MTP GGUF (e.g. the 35B target
//! the kanban `qwen` model is consolidated on):
//!
//! ```sh
//! cargo run --release --example mtp_smoke -- \
//!     unsloth/Qwen3.6-35B-A3B-MTP-GGUF Qwen3.6-35B-A3B-MXFP4_MOE.gguf
//! ```
//!
//! Append a custom prompt as the third arg:
//!
//! ```sh
//! cargo run --release --example mtp_smoke -- <repo> <filename> "Say hello briefly."
//! ```

use std::time::{Instant, SystemTime};

use futures::StreamExt;
use llama_agent::test_models::{MTP_TEST_MODEL_FILE, MTP_TEST_MODEL_REPO};
use llama_agent::types::{
    AgentAPI, AgentConfig, FinishReason, GenerationRequest, Message, MessageRole, ModelConfig,
    ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::AgentServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // INFO+ tracing on stderr so the queue worker's MTP path messages
    // ("Worker N streaming with MTP speculative decoding (nextn_predict_layers=…)"
    // / "no cached KV state" / "streaming reusing N cached tokens") surface.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let mut args = std::env::args().skip(1);
    let repo = args
        .next()
        .unwrap_or_else(|| MTP_TEST_MODEL_REPO.to_string());
    let filename = args
        .next()
        .unwrap_or_else(|| MTP_TEST_MODEL_FILE.to_string());
    let prompt = args
        .next()
        .unwrap_or_else(|| "Reply with the single word: ok.".to_string());

    eprintln!("=== mtp_smoke ===");
    eprintln!("repo:     {repo}");
    eprintln!("filename: {filename}");
    eprintln!("prompt:   {prompt:?}");

    let cfg = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo,
                filename: Some(filename),
                folder: None,
            },
            batch_size: 512,
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
        queue_config: QueueConfig::default(),
    };

    let t0 = Instant::now();
    let agent = AgentServer::initialize(cfg).await?;
    eprintln!("agent initialized in {:?}", t0.elapsed());

    let session = agent.create_session().await?;
    let session_id = session.id;
    agent
        .add_message(
            &session_id,
            Message {
                role: MessageRole::User,
                content: prompt.clone(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        )
        .await?;

    eprintln!("\n=== streaming ===");
    let t_gen = Instant::now();
    let mut stream = agent
        .generate_stream(
            GenerationRequest::new(session_id)
                .with_max_tokens(64)
                .with_temperature(0.0),
        )
        .await?;

    let mut text = String::new();
    let mut chunks = 0usize;
    let mut tokens = 0usize;
    let mut finish: Option<FinishReason> = None;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        chunks += 1;
        tokens += chunk.token_count;
        if !chunk.text.is_empty() {
            print!("{}", chunk.text);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        text.push_str(&chunk.text);
        if chunk.is_complete {
            finish = chunk.finish_reason.clone();
        }
    }
    println!(); // newline after streamed text
    eprintln!(
        "\n=== done in {:?} ===\nchunks: {chunks}\ntokens: {tokens}\nfinish: {finish:?}\ntext:   {text:?}",
        t_gen.elapsed(),
    );

    Ok(())
}
