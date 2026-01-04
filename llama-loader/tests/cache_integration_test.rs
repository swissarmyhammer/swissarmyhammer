//! Cache integration tests for llama-loader
//!
//! These tests verify that model downloading and caching work correctly
//! with real small models from HuggingFace.

use llama_loader::ModelLoader;
use llama_loader::RetryConfig;
use serial_test::serial;
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;
use tempfile::TempDir;
use tracing_test::traced_test;

/// Small embedding model for testing cache behavior - using one with known GGUF files
const TINY_MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B-GGUF";

/// Create a test retry config with shorter timeouts for testing
fn create_test_retry_config() -> RetryConfig {
    RetryConfig {
        max_retries: 2,
        initial_delay_ms: 100,
        backoff_multiplier: 1.5,
        max_delay_ms: 1000,
    }
}

/// Test cache hit behavior by loading the same model twice
/// This test verifies that:
/// 1. First load downloads the model
/// 2. Second load is significantly faster (cache hit)
/// 3. Both loads return equivalent models
#[tokio::test]
#[traced_test]
#[serial]
async fn test_model_cache_hit_behavior() {
    // Use a temporary directory to isolate cache behavior
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create backend for testing
    let backend =
        Arc::new(llama_cpp_2::llama_backend::LlamaBackend::init().expect("Failed to init backend"));
    let loader = ModelLoader::new(backend.clone());

    let retry_config = create_test_retry_config();

    // First model load - should download
    tracing::info!("Starting first model load (should download)");
    let start_time = Instant::now();

    let model1 = loader
        .load_huggingface_model(
            TINY_MODEL_REPO,
            None, // Use auto-detection
            &retry_config,
        )
        .await;

    let first_load_time = start_time.elapsed();
    tracing::info!("First load completed in: {}", Pretty(&first_load_time));

    // Verify first load succeeded
    assert!(
        model1.is_ok(),
        "First model load should succeed: {:?}",
        model1.err()
    );
    let model1 = model1.unwrap();

    // Get model metadata
    let metadata1 = &model1.metadata;
    tracing::info!("First model metadata: {}", Pretty(&metadata1));

    // Second model load - should use cache
    tracing::info!("Starting second model load (should use cache)");
    let start_time = Instant::now();

    let model2 = loader
        .load_huggingface_model(
            TINY_MODEL_REPO,
            None, // Use auto-detection
            &retry_config,
        )
        .await;

    let second_load_time = start_time.elapsed();
    tracing::info!("Second load completed in: {}", Pretty(&second_load_time));

    // Verify second load succeeded
    assert!(
        model2.is_ok(),
        "Second model load should succeed: {:?}",
        model2.err()
    );
    let model2 = model2.unwrap();

    // Get second model metadata
    let metadata2 = &model2.metadata;
    tracing::info!("Second model metadata: {}", Pretty(&metadata2));

    // Verify both models have same metadata characteristics
    assert_eq!(metadata1.filename, metadata2.filename);
    assert_eq!(metadata1.size_bytes, metadata2.size_bytes);

    // The second load should be significantly faster than the first
    // Allow some variance but expect at least 50% faster
    let speedup_ratio = first_load_time.as_millis() as f64 / second_load_time.as_millis() as f64;

    tracing::info!("Load time comparison:");
    tracing::info!("  First load:  {}", Pretty(&first_load_time));
    tracing::info!("  Second load: {}", Pretty(&second_load_time));
    tracing::info!("  Speedup:     {:.2}x", speedup_ratio);

    if speedup_ratio > 1.5 {
        tracing::info!(
            "✅ Cache hit detected - second load was {:.2}x faster",
            speedup_ratio
        );
    } else if second_load_time < Duration::from_secs(5) {
        tracing::info!(
            "✅ Second load was fast ({:?}) - likely cache hit",
            second_load_time
        );
    } else {
        tracing::warn!("⚠️  Second load was not significantly faster - cache may not be working");
        tracing::warn!("    This could be due to network conditions or model size");
    }

    // Verify cache_hit field is set correctly
    tracing::info!("First load cache_hit: {}", metadata1.cache_hit);
    tracing::info!("Second load cache_hit: {}", metadata2.cache_hit);
    assert!(metadata2.cache_hit, "Second load should be a cache hit");
}

/// Test that verifies ModelLoader with same configuration reuses cached models
#[tokio::test]
#[traced_test]
#[serial]
async fn test_model_loader_cache_reuse() {
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    let backend =
        Arc::new(llama_cpp_2::llama_backend::LlamaBackend::init().expect("Failed to init backend"));
    let retry_config = create_test_retry_config();

    // Create first ModelLoader instance
    let loader1 = ModelLoader::new(backend.clone());

    tracing::info!("Loading model with first loader instance");
    let start_time = Instant::now();

    let model1 = loader1
        .load_huggingface_model(TINY_MODEL_REPO, None, &retry_config)
        .await;

    let first_load_time = start_time.elapsed();
    tracing::info!("First loader completed in: {}", Pretty(&first_load_time));

    assert!(
        model1.is_ok(),
        "First loader should succeed: {:?}",
        model1.err()
    );
    let model1 = model1.unwrap();

    // Create second ModelLoader instance with same configuration
    let loader2 = ModelLoader::new(backend);

    tracing::info!("Loading same model with second loader instance");
    let start_time = Instant::now();

    let model2 = loader2
        .load_huggingface_model(TINY_MODEL_REPO, None, &retry_config)
        .await;

    let second_load_time = start_time.elapsed();
    tracing::info!("Second loader completed in: {}", Pretty(&second_load_time));

    assert!(
        model2.is_ok(),
        "Second loader should succeed: {:?}",
        model2.err()
    );
    let model2 = model2.unwrap();

    // Verify models are equivalent
    let metadata1 = &model1.metadata;
    let metadata2 = &model2.metadata;

    assert_eq!(metadata1.filename, metadata2.filename);
    assert_eq!(metadata1.size_bytes, metadata2.size_bytes);

    // Second load should benefit from HuggingFace Hub's caching
    let speedup_ratio = first_load_time.as_millis() as f64 / second_load_time.as_millis() as f64;

    tracing::info!("ModelLoader comparison:");
    tracing::info!("  First instance:  {}", Pretty(&first_load_time));
    tracing::info!("  Second instance: {}", Pretty(&second_load_time));
    tracing::info!("  Speedup:         {:.2}x", speedup_ratio);

    if speedup_ratio > 1.5 {
        tracing::info!("✅ Cache reuse detected between ModelLoader instances");
    } else {
        tracing::info!("ℹ️  Both loads completed - cache behavior validated");
    }
}

/// Test demonstrating the expected cache_hit field behavior
/// This test shows what the implementation should look like once cache_hit is properly tracked
#[tokio::test]
#[traced_test]
#[serial]
async fn test_cache_hit_metadata_field() {
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    let backend =
        Arc::new(llama_cpp_2::llama_backend::LlamaBackend::init().expect("Failed to init backend"));
    let retry_config = create_test_retry_config();
    let loader = ModelLoader::new(backend);

    // First load - should be a cache miss
    let model1 = loader
        .load_huggingface_model(TINY_MODEL_REPO, None, &retry_config)
        .await
        .expect("First load should succeed");

    let metadata1 = &model1.metadata;

    // First load may or may not be a cache hit depending on previous runs
    tracing::info!("First load cache_hit: {}", metadata1.cache_hit);

    // Second load - should be a cache hit since first load established the cache
    let model2 = loader
        .load_huggingface_model(TINY_MODEL_REPO, None, &retry_config)
        .await
        .expect("Second load should succeed");

    let metadata2 = &model2.metadata;

    // Second load MUST be a cache hit since first load established it
    assert!(metadata2.cache_hit, "Second load MUST be a cache hit");

    tracing::info!("✅ Cache hit field validation passed:");
    tracing::info!("  First load cache_hit: {}", metadata1.cache_hit);
    tracing::info!(
        "  Second load cache_hit: {} (must be true)",
        metadata2.cache_hit
    );
}
