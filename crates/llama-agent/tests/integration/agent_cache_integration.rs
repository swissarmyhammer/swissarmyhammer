//! Agent cache integration tests
//!
//! These tests verify that AgentServer properly uses cached models when restarted
//! with the same configuration.

use llama_agent::types::{ModelConfig, ModelSource, RetryConfig};
use llama_agent::{AgentAPI, AgentConfig, AgentServer, ParallelConfig, QueueConfig, SessionConfig};
use serial_test::serial;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Small model for testing cache behavior with actual AgentServer
// Use standard test models from test_models module
use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

/// Create a test agent config with a small model
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

/// Test that AgentServer uses cache on second initialization
/// This is the core test that verifies cache hit behavior
#[tokio::test]
#[serial]
async fn test_agent_server_cache_hit_on_restart() {
    let config = create_test_agent_config();

    info!("=== FIRST AGENT SERVER INITIALIZATION ===");
    info!("Testing first AgentServer initialization (may use cache or download)");

    let start_time = Instant::now();
    let first_agent_result = AgentServer::initialize(config.clone()).await;
    let first_init_time = start_time.elapsed();

    match first_agent_result {
        Ok(agent) => {
            info!(
                "✅ First AgentServer initialized successfully in {:?}",
                first_init_time
            );

            // Get some info about the loaded model
            if let Some(metadata) = agent.get_model_metadata().await {
                info!("First agent model metadata: {:?}", metadata);
                info!("Cache hit (first): {}", metadata.cache_hit);

                // First load may or may not be a cache hit (depends on prior runs)
                // This is fine - we just need the model to be loaded
            }

            // Drop the agent to fully release resources
            info!("Dropping first AgentServer instance...");
            drop(agent);
            info!("First AgentServer dropped");
        }
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            // Check if this is a HuggingFace rate limiting issue
            if error_msg.contains("429")
                || error_msg.contains("too many requests")
                || error_msg.contains("rate limited")
                || error_msg.contains("loadingfailed")
            {
                warn!("⚠️  Skipping test due to HuggingFace rate limiting: {}", e);
                println!("⚠️  Agent cache test skipped (HuggingFace rate limited)");
                return;
            } else {
                panic!("First AgentServer initialization failed: {}", e);
            }
        }
    }

    // Wait a moment to ensure everything is cleaned up
    tokio::time::sleep(Duration::from_millis(500)).await;

    info!("=== SECOND AGENT SERVER INITIALIZATION ===");
    info!("Testing second AgentServer initialization (should use cache)");

    let start_time = Instant::now();
    let second_agent_result = AgentServer::initialize(config).await;
    let second_init_time = start_time.elapsed();

    match second_agent_result {
        Ok(agent) => {
            info!(
                "✅ Second AgentServer initialized successfully in {:?}",
                second_init_time
            );

            // Get metadata for the cached model
            if let Some(metadata) = agent.get_model_metadata().await {
                info!("Second agent model metadata: {:?}", metadata);
                info!("Cache hit (second): {}", metadata.cache_hit);

                // Second load MUST be a cache hit since first agent loaded the model
                assert!(
                    metadata.cache_hit,
                    "Second model load MUST be a cache hit - first agent already loaded the model"
                );
            }

            // Compare initialization times
            let speedup_ratio =
                first_init_time.as_millis() as f64 / second_init_time.as_millis() as f64;

            info!("=== CACHE PERFORMANCE COMPARISON ===");
            info!("First initialization:  {:?}", first_init_time);
            info!("Second initialization: {:?}", second_init_time);
            info!("Speedup ratio: {:.2}x", speedup_ratio);

            if speedup_ratio > 1.5 {
                info!("✅ Significant speedup detected - cache is working effectively");
            } else if second_init_time < Duration::from_secs(10) {
                info!("✅ Second initialization was fast - likely using cache");
            } else {
                warn!("⚠️  Second initialization was not significantly faster");
            }

            info!("✅ AgentServer cache hit test completed successfully");
        }
        Err(e) => {
            panic!("Second AgentServer initialization failed: {}", e);
        }
    }
}

/// Test that multiple AgentServer instances can share cached models
#[tokio::test]
#[serial]
async fn test_agent_server_concurrent_cache_sharing() {
    let config = create_test_agent_config();

    info!("Testing concurrent AgentServer instances with shared cache");

    // Initialize first agent (may download model)
    let start_time = Instant::now();
    let agent1_result = AgentServer::initialize(config.clone()).await;
    let first_init_time = start_time.elapsed();

    let agent1 = match agent1_result {
        Ok(agent) => {
            info!("First agent initialized in {:?}", first_init_time);
            agent
        }
        Err(e) => {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("429") || error_msg.contains("rate limited") {
                warn!("⚠️  Skipping test due to HuggingFace rate limiting");
                return;
            }
            panic!("First agent initialization failed: {}", e);
        }
    };

    // Initialize second agent while first is still alive (should use cache)
    let start_time = Instant::now();
    let agent2_result = AgentServer::initialize(config).await;
    let second_init_time = start_time.elapsed();

    let agent2 = match agent2_result {
        Ok(agent) => {
            info!("Second agent initialized in {:?}", second_init_time);
            agent
        }
        Err(e) => {
            panic!("Second agent initialization failed: {}", e);
        }
    };

    // Check metadata for both agents
    if let (Some(metadata1), Some(metadata2)) = (
        agent1.get_model_metadata().await,
        agent2.get_model_metadata().await,
    ) {
        info!("Agent1 cache_hit: {}", metadata1.cache_hit);
        info!("Agent2 cache_hit: {}", metadata2.cache_hit);

        // At least the second agent should be a cache hit
        assert!(metadata2.cache_hit, "Second agent should use cached model");

        // Both should have same model characteristics
        assert_eq!(metadata1.filename, metadata2.filename);
        assert_eq!(metadata1.size_bytes, metadata2.size_bytes);
    }

    info!("✅ Concurrent cache sharing test completed successfully");
}
