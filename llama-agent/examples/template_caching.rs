//! Template caching example
//!
//! This example demonstrates template caching performance benefits
//! by creating multiple sessions with the same template.

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
        SessionConfig,
    },
    AgentServer,
};
use std::time::Instant;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with template cache debug level
    // This enables you to see cache hit/miss events in the output
    tracing_subscriber::fmt()
        .with_env_filter("llama_agent::template_cache=debug,llama_agent=info")
        .init();

    info!("Starting template caching example");
    info!("Debug logging enabled for template cache - you'll see cache hit/miss events");

    // Configure model
    let model_config = ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "bartowski/Qwen2.5-Coder-1.5B-Instruct-GGUF".to_string(),
            filename: Some("Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf".to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 4,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    };

    // Configure agent
    let config = AgentConfig {
        model: model_config,
        queue_config: QueueConfig {
            max_queue_size: 100,
            worker_threads: 1,
        },
        mcp_servers: vec![],
        session_config: SessionConfig {
            max_sessions: 20,
            auto_compaction: None,
            model_context_size: 32768,
            persistence_enabled: false,
            session_storage_dir: None,
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        },
        parallel_execution_config: ParallelConfig::default(),
    };

    println!("Creating agent and loading model...");
    let agent = AgentServer::initialize(config).await?;

    println!("\n=== Template Caching Demo ===\n");

    // Create first session (cache miss)
    println!("Creating first session (cache miss)...");
    let start = Instant::now();
    let _session1 = agent.create_session().await?;
    let duration1 = start.elapsed();
    println!("Session 1 created in: {:?}", duration1);

    // Get cache stats after first session
    let stats = agent.get_template_cache_stats();
    println!("\nCache stats after session 1:");
    println!("  Entries: {}", stats.entries);
    println!("  Hits: {}", stats.hits);
    println!("  Misses: {}", stats.misses);

    // Create second session (cache hit)
    println!("\nCreating second session (cache hit)...");
    let start = Instant::now();
    let _session2 = agent.create_session().await?;
    let duration2 = start.elapsed();
    println!("Session 2 created in: {:?}", duration2);

    // Get cache stats after second session
    let stats = agent.get_template_cache_stats();
    println!("\nCache stats after session 2:");
    println!("  Entries: {}", stats.entries);
    println!("  Hits: {}", stats.hits);
    println!("  Misses: {}", stats.misses);
    if stats.hits + stats.misses > 0 {
        println!("  Hit rate: {:.2}%", stats.hit_rate * 100.0);
    }

    // Show speedup
    if duration1.as_millis() > 0 && duration2.as_millis() > 0 {
        let speedup = duration1.as_secs_f64() / duration2.as_secs_f64();
        println!("\nSpeedup: {:.1}x faster", speedup);
        let percent_faster = ((duration1.as_millis() - duration2.as_millis()) as f64
            / duration1.as_millis() as f64)
            * 100.0;
        println!("Session 2 was {:.1}% faster", percent_faster);
    }

    // Create 10 more sessions to demonstrate cache effectiveness
    println!("\n=== Creating 10 More Sessions ===\n");
    let start = Instant::now();
    let mut total_millis = 0u128;
    for i in 3..=12 {
        let session_start = Instant::now();
        let _session = agent.create_session().await?;
        let session_duration = session_start.elapsed();
        total_millis += session_duration.as_millis();
        println!("Session {} created in: {:?}", i, session_duration);
    }
    let total_duration = start.elapsed();

    // Final cache stats
    let stats = agent.get_template_cache_stats();
    println!("\n=== Final Cache Statistics ===");
    println!("Cache entries: {}", stats.entries);
    println!("Total tokens cached: {}", stats.total_tokens);
    println!("Total hits: {}", stats.hits);
    println!("Total misses: {}", stats.misses);
    if stats.hits + stats.misses > 0 {
        println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
    }

    // Final timing stats
    println!("\n=== Final Timing Statistics ===");
    println!("10 sessions created in: {:?}", total_duration);
    println!(
        "Average time per session: {:?}",
        std::time::Duration::from_millis((total_millis / 10) as u64)
    );

    // Show comparison with theoretical non-cached performance
    println!("\n=== Performance Comparison ===");
    println!("With caching:");
    println!("  Session 1 (miss): {:?}", duration1);
    println!("  Session 2 (hit): {:?}", duration2);
    println!(
        "  Sessions 3-12 average: {:?}",
        std::time::Duration::from_millis((total_millis / 10) as u64)
    );
    println!(
        "  Total for 12 sessions: {:?}",
        duration1 + duration2 + total_duration
    );

    if duration1.as_millis() > 0 {
        let theoretical_no_cache = duration1 * 12;
        let actual_with_cache = duration1 + duration2 + total_duration;
        let savings = theoretical_no_cache - actual_with_cache;
        let savings_percent =
            (savings.as_millis() as f64 / theoretical_no_cache.as_millis() as f64) * 100.0;

        println!("\nWithout caching (theoretical):");
        println!("  Total for 12 sessions: {:?}", theoretical_no_cache);
        println!(
            "\nTime saved: {:?} ({:.1}% faster)",
            savings, savings_percent
        );
    }

    println!("\n=== Key Takeaways ===");
    println!("1. First session with new template: Slower (cache miss)");
    println!("2. Subsequent sessions: Much faster (cache hit)");
    println!("3. Benefit scales with number of sessions");
    println!("4. Template caching is automatic - no code changes needed");
    println!("5. Each unique template creates its own cache entry");

    info!("Template caching example completed");
    Ok(())
}
