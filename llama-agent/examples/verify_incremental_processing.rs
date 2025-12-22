//! Verify that incremental processing is actually working
//!
//! This example explicitly checks if incremental processing is active by:
//! 1. Monitoring how many tokens are processed on each turn
//! 2. Checking if context_state is being used
//! 3. Verifying template caching is active
//! 4. Measuring performance improvements
//!
//! Run with:
//! ```bash
//! RUST_LOG=debug cargo run --example verify_incremental_processing --release 2>&1 | grep -E "(Processing|Skipping|template|context|tokens)"
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
            debug: true, // Enable debug logs to see token processing
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        },
        mcp_servers: Vec::new(),
        session_config: SessionConfig {
            persistence_enabled: true, // Enable for context_state
            ..Default::default()
        },
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .init();

    println!("=== VERIFYING INCREMENTAL PROCESSING ===\n");

    // Initialize agent
    println!("Initializing agent...");
    let agent = AgentServer::initialize(create_config()).await?;
    println!("✅ Agent initialized\n");

    // Create session
    let session = agent.create_session().await?;
    let session_id = session.id;

    println!("📊 Test Structure:");
    println!("   - Generate 3 responses with same session");
    println!("   - Each adds one user message");
    println!("   - Monitor token processing in DEBUG logs\n");

    println!("Expected behavior if incremental processing works:");
    println!("   ✓ Gen 1: Processes ALL tokens (system + first message)");
    println!("   ✓ Gen 2: Processes ONLY new tokens (second message)");
    println!("   ✓ Gen 3: Processes ONLY new tokens (third message)\n");

    println!("What to look for in DEBUG logs:");
    println!("   • 'Skipping N template tokens' = Template caching active");
    println!("   • 'Found N common tokens' = Context state working");
    println!("   • 'Processing N message tokens' = How many new tokens\n");

    println!("========================================\n");

    // Generation 1
    println!("🔄 GENERATION 1 - Baseline");
    println!("Message: 'What is 2 + 2?'\n");

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

    let start = Instant::now();
    let request1 = GenerationRequest::new(session_id).with_max_tokens(50);
    let response1 = agent.generate(request1).await?;
    let time1 = start.elapsed();

    println!("\n✅ Gen 1 complete:");
    println!("   Time: {:?}", time1);
    println!("   Tokens: {}", response1.tokens_generated);
    println!(
        "   Rate: {:.2} tok/s",
        response1.tokens_generated as f64 / time1.as_secs_f64()
    );

    // Check session state
    if let Some(session) = agent.get_session(&session_id).await? {
        println!("\n   Session state after Gen 1:");
        if let Some(ref ctx) = session.context_state {
            println!("   ✓ context_state: {} tokens", ctx.processed_tokens.len());
        } else {
            println!("   ✗ context_state: NOT PRESENT");
        }
        if let Some(template_count) = session.template_token_count {
            println!("   ✓ template_cache: {} tokens", template_count);
        } else {
            println!("   ✗ template_cache: NOT PRESENT");
        }
    }

    println!("\n========================================\n");

    // Generation 2
    println!("🔄 GENERATION 2 - Should use incremental processing");
    println!("Message: 'What is 5 + 5?'\n");

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

    let start = Instant::now();
    let request2 = GenerationRequest::new(session_id).with_max_tokens(50);
    let response2 = agent.generate(request2).await?;
    let time2 = start.elapsed();

    println!("\n✅ Gen 2 complete:");
    println!("   Time: {:?}", time2);
    println!("   Tokens: {}", response2.tokens_generated);
    println!(
        "   Rate: {:.2} tok/s",
        response2.tokens_generated as f64 / time2.as_secs_f64()
    );
    println!(
        "   Speedup: {:.2}x",
        time1.as_secs_f64() / time2.as_secs_f64()
    );

    // Check session state
    if let Some(session) = agent.get_session(&session_id).await? {
        println!("\n   Session state after Gen 2:");
        if let Some(ref ctx) = session.context_state {
            println!("   ✓ context_state: {} tokens", ctx.processed_tokens.len());
        } else {
            println!("   ✗ context_state: NOT PRESENT");
        }
    }

    println!("\n========================================\n");

    // Generation 3
    println!("🔄 GENERATION 3 - Should also use incremental processing");
    println!("Message: 'What is 10 + 10?'\n");

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

    let start = Instant::now();
    let request3 = GenerationRequest::new(session_id).with_max_tokens(50);
    let response3 = agent.generate(request3).await?;
    let time3 = start.elapsed();

    println!("\n✅ Gen 3 complete:");
    println!("   Time: {:?}", time3);
    println!("   Tokens: {}", response3.tokens_generated);
    println!(
        "   Rate: {:.2} tok/s",
        response3.tokens_generated as f64 / time3.as_secs_f64()
    );
    println!(
        "   Speedup: {:.2}x",
        time1.as_secs_f64() / time3.as_secs_f64()
    );

    println!("\n========================================\n");

    // Final analysis
    println!("📊 ANALYSIS:");
    println!("\nTiming:");
    println!("   Gen 1: {:?}", time1);
    println!("   Gen 2: {:?}", time2);
    println!("   Gen 3: {:?}", time3);

    let speedup_2 = time1.as_secs_f64() / time2.as_secs_f64();
    let speedup_3 = time1.as_secs_f64() / time3.as_secs_f64();
    let avg_speedup = (speedup_2 + speedup_3) / 2.0;

    println!("\nSpeedup:");
    println!("   Gen 2: {:.2}x", speedup_2);
    println!("   Gen 3: {:.2}x", speedup_3);
    println!("   Avg:   {:.2}x", avg_speedup);

    println!("\n📋 VERDICT:");

    if avg_speedup > 1.2 {
        println!("   ✅ INCREMENTAL PROCESSING IS WORKING");
        println!(
            "   Subsequent generations are {:.0}% faster on average",
            (avg_speedup - 1.0) * 100.0
        );
    } else if avg_speedup > 1.05 {
        println!("   ⚠️  PARTIAL INCREMENTAL PROCESSING");
        println!(
            "   Some speedup observed ({:.0}%), but could be better",
            (avg_speedup - 1.0) * 100.0
        );
    } else {
        println!("   ❌ INCREMENTAL PROCESSING NOT WORKING");
        println!("   No significant speedup observed");
        println!("\n   Possible reasons:");
        println!("   • Token generation time dominates (caching doesn't help much)");
        println!("   • Context state not being populated");
        println!("   • Full prompt reprocessing happening");
    }

    println!("\n💡 Review the DEBUG logs above to see:");
    println!("   • How many tokens were processed each generation");
    println!("   • Whether template caching kicked in");
    println!("   • Whether context state was used");

    Ok(())
}
