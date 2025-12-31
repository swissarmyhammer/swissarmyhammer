//! Detailed timing analysis with instrumentation
//!
//! This example adds detailed timing measurements to identify exactly where
//! time is spent during repeated message generation.
//!
//! Run with:
//! ```bash
//! RUST_LOG=debug cargo run --example detailed_timing --release
//! ```

use llama_agent::types::{
    AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource, ParallelConfig,
    QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::{AgentAPI, AgentServer};
use std::time::{Duration, Instant, SystemTime};

const MODEL_REPO: &str = "unsloth/Qwen3-0.6B-GGUF";
const MODEL_FILE: &str = "Qwen3-0.6B-UD-Q4_K_XL.gguf";

fn create_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: MODEL_REPO.to_string(),
                filename: Some(MODEL_FILE.to_string()),
                folder: None,
            },
            batch_size: 64,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: true, // Enable debug logging for more detail
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        },
        mcp_servers: Vec::new(),
        session_config: SessionConfig {
            persistence_enabled: true,
            ..Default::default()
        },
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
    }
}

struct TimingMeasurement {
    name: String,
    duration: Duration,
}

impl TimingMeasurement {
    fn new(name: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            duration,
        }
    }
}

async fn measure_generation(
    agent: &AgentServer,
    session_id: &llama_agent::types::ids::SessionId,
    message: &str,
    generation_num: usize,
) -> Result<Vec<TimingMeasurement>, Box<dyn std::error::Error>> {
    let mut timings = Vec::new();

    println!("\n=== GENERATION {} ===", generation_num);
    println!("Message: {}", message);

    // Time: Add message
    let start = Instant::now();
    agent
        .add_message(
            session_id,
            Message {
                role: MessageRole::User,
                content: message.to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        )
        .await?;
    timings.push(TimingMeasurement::new("add_message", start.elapsed()));

    // Time: Generate (this includes all sub-operations)
    let start = Instant::now();
    let request = GenerationRequest::new(*session_id).with_max_tokens(20);
    let response = agent.generate(request).await?;
    let gen_time = start.elapsed();
    timings.push(TimingMeasurement::new("generate_total", gen_time));

    println!(
        "Generated {} tokens in {:?}",
        response.tokens_generated, gen_time
    );
    println!(
        "Rate: {:.2} tokens/sec",
        response.tokens_generated as f64 / gen_time.as_secs_f64()
    );

    // Get session to check context state
    let start = Instant::now();
    if let Some(session) = agent.get_session(session_id).await? {
        timings.push(TimingMeasurement::new("get_session", start.elapsed()));

        if let Some(ref ctx) = session.context_state {
            println!(
                "Context state: {} tokens processed",
                ctx.processed_tokens.len()
            );
        } else {
            println!("Context state: NOT PRESENT");
        }

        if session.cached_token_count > 0 {
            println!("Cached token count: {} tokens", session.cached_token_count);
        }
    }

    Ok(timings)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== DETAILED TIMING ANALYSIS ===\n");

    // Initialize agent
    println!("Initializing agent...");
    let start = Instant::now();
    let agent = AgentServer::initialize(create_config()).await?;
    println!("✅ Agent initialized in {:?}\n", start.elapsed());

    // Create session
    let session = agent.create_session().await?;
    let session_id = session.id;

    // Run three generations with detailed timing
    let mut all_timings = Vec::new();

    all_timings.push(measure_generation(&agent, &session_id, "What is 2 + 2?", 1).await?);
    all_timings.push(measure_generation(&agent, &session_id, "What is 5 + 5?", 2).await?);
    all_timings.push(measure_generation(&agent, &session_id, "What is 10 + 10?", 3).await?);

    // Analyze timings
    println!("\n=== TIMING BREAKDOWN ===\n");

    for (gen_num, timings) in all_timings.iter().enumerate() {
        println!("Generation {}:", gen_num + 1);
        for timing in timings {
            println!("  {:<20} {:?}", timing.name, timing.duration);
        }
        println!();
    }

    // Calculate averages
    println!("=== AVERAGES ===\n");

    let avg_add_message: Duration = all_timings
        .iter()
        .flat_map(|t| t.iter())
        .filter(|t| t.name == "add_message")
        .map(|t| t.duration)
        .sum::<Duration>()
        / 3;

    let avg_generate: Duration = all_timings
        .iter()
        .flat_map(|t| t.iter())
        .filter(|t| t.name == "generate_total")
        .map(|t| t.duration)
        .sum::<Duration>()
        / 3;

    let avg_get_session: Duration = all_timings
        .iter()
        .flat_map(|t| t.iter())
        .filter(|t| t.name == "get_session")
        .map(|t| t.duration)
        .sum::<Duration>()
        / 3;

    println!("Average add_message:  {:?}", avg_add_message);
    println!("Average generate:     {:?}", avg_generate);
    println!("Average get_session:  {:?}", avg_get_session);

    let pct_overhead =
        (avg_add_message + avg_get_session).as_secs_f64() / avg_generate.as_secs_f64() * 100.0;
    println!("\nOverhead: {:.2}% of generation time", pct_overhead);

    println!("\n=== KEY INSIGHTS ===\n");
    println!("The DEBUG logs above show:");
    println!("1. How many tokens are being processed per generation");
    println!("2. Whether template caching is active");
    println!("3. Whether context_state is being populated");
    println!("4. Tokenization, batch processing, and sampling times");
    println!("\nLook for log lines containing:");
    println!("  - 'Tokenized prompt to N tokens'");
    println!("  - 'Processing N message tokens'");
    println!("  - 'Skipping N template tokens'");
    println!("  - 'Found N common tokens'");
    println!("  - 'Using template cache'");

    Ok(())
}
