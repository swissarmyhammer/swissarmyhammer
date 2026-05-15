//! Timing analysis for repeated message generation
//!
//! This example profiles different aspects of message generation to identify
//! performance bottlenecks in multi-turn conversations.
//!
//! Run with:
//! ```bash
//! cargo run --example timing_analysis --release
//! ```

use llama_agent::types::{
    AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource, ParallelConfig,
    QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::{AgentAPI, AgentServer};
use std::time::{Instant, SystemTime};

// Use standard test models from test_models module
use llama_agent::test_models::{TEST_MODEL_FILE as MODEL_FILE, TEST_MODEL_REPO as MODEL_REPO};

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
            debug: false,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TIMING ANALYSIS FOR REPEATED MESSAGE GENERATION ===\n");

    // Phase 1: Agent initialization
    println!("📊 Phase 1: Agent Initialization");
    let start = Instant::now();
    let agent = AgentServer::initialize(create_config()).await?;
    let init_time = start.elapsed();
    println!("✅ Agent initialized in {:?}\n", init_time);

    // Phase 2: Session creation
    println!("📊 Phase 2: Session Creation");
    let start = Instant::now();
    let session = agent.create_session().await?;
    let session_create_time = start.elapsed();
    println!("✅ Session created in {:?}\n", session_create_time);

    let session_id = session.id;

    // Phase 3: Message operations
    println!("📊 Phase 3: Message Operations");

    let start = Instant::now();
    agent
        .add_message(
            &session_id,
            Message {
                role: MessageRole::User,
                content: "What is 2 + 2?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        )
        .await?;
    let msg_add_time = start.elapsed();
    println!("✅ Message added in {:?}", msg_add_time);

    // Phase 4: First generation (baseline)
    println!("\n📊 Phase 4: First Generation (20 tokens)");
    let start = Instant::now();
    let request1 = GenerationRequest::new(session_id).with_max_tokens(20);
    let response1 = agent.generate(request1).await?;
    let gen1_time = start.elapsed();
    println!("✅ Generation 1 completed in {:?}", gen1_time);
    println!("   Tokens: {}", response1.tokens_generated);
    println!(
        "   Response: {}",
        response1
            .generated_text
            .chars()
            .take(50)
            .collect::<String>()
    );

    // Phase 5: Second generation (incremental)
    println!("\n📊 Phase 5: Second Generation (20 tokens)");

    let start = Instant::now();
    agent
        .add_message(
            &session_id,
            Message {
                role: MessageRole::User,
                content: "What is 5 + 5?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        )
        .await?;
    let msg_add2_time = start.elapsed();

    let start = Instant::now();
    let request2 = GenerationRequest::new(session_id).with_max_tokens(20);
    let response2 = agent.generate(request2).await?;
    let gen2_time = start.elapsed();

    println!("✅ Message added in {:?}", msg_add2_time);
    println!("✅ Generation 2 completed in {:?}", gen2_time);
    println!("   Tokens: {}", response2.tokens_generated);

    // Phase 6: Third generation (incremental)
    println!("\n📊 Phase 6: Third Generation (20 tokens)");

    let start = Instant::now();
    agent
        .add_message(
            &session_id,
            Message {
                role: MessageRole::User,
                content: "What is 10 + 10?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        )
        .await?;
    let msg_add3_time = start.elapsed();

    let start = Instant::now();
    let request3 = GenerationRequest::new(session_id).with_max_tokens(20);
    let response3 = agent.generate(request3).await?;
    let gen3_time = start.elapsed();

    println!("✅ Message added in {:?}", msg_add3_time);
    println!("✅ Generation 3 completed in {:?}", gen3_time);
    println!("   Tokens: {}", response3.tokens_generated);

    // Summary
    println!("\n=== TIMING SUMMARY ===");
    println!("Agent init:     {:?}", init_time);
    println!("Session create: {:?}", session_create_time);
    println!(
        "Message add:    {:?} (avg)",
        (msg_add_time + msg_add2_time + msg_add3_time) / 3
    );
    println!();
    println!("Generation 1:   {:?}", gen1_time);
    println!("Generation 2:   {:?}", gen2_time);
    println!("Generation 3:   {:?}", gen3_time);
    println!();

    let avg_gen = (gen1_time + gen2_time + gen3_time) / 3;
    println!("Average gen:    {:?}", avg_gen);

    let speedup_2 = gen1_time.as_millis() as f64 / gen2_time.as_millis() as f64;
    let speedup_3 = gen1_time.as_millis() as f64 / gen3_time.as_millis() as f64;
    println!();
    println!("Gen 2 speedup:  {:.2}x", speedup_2);
    println!("Gen 3 speedup:  {:.2}x", speedup_3);

    println!("\n=== BOTTLENECK ANALYSIS ===");
    let total_overhead = session_create_time + msg_add_time + msg_add2_time + msg_add3_time;
    let total_generation = gen1_time + gen2_time + gen3_time;
    let overhead_pct = (total_overhead.as_millis() as f64
        / (total_overhead.as_millis() + total_generation.as_millis()) as f64)
        * 100.0;

    println!(
        "Session/message overhead: {:?} ({:.1}%)",
        total_overhead, overhead_pct
    );
    println!(
        "Generation time:          {:?} ({:.1}%)",
        total_generation,
        100.0 - overhead_pct
    );

    if overhead_pct > 5.0 {
        println!(
            "\n⚠️  Session/message operations account for >{:.1}% of time",
            overhead_pct
        );
        println!("   Consider optimizing session creation or message handling");
    }

    if speedup_2 < 1.1 && speedup_3 < 1.1 {
        println!("\n⚠️  Little speedup observed in subsequent generations");
        println!("   Incremental processing may not be fully optimized");
    } else {
        println!("\n✅ Good incremental processing performance");
        println!(
            "   Subsequent generations show {:.0}% speedup",
            ((speedup_2 + speedup_3) / 2.0 - 1.0) * 100.0
        );
    }

    Ok(())
}
