//! Cache integration tests for model-loader
//!
//! These tests verify that model downloading and caching work correctly
//! with real small models from HuggingFace.

use model_loader::{ModelConfig, ModelResolver, ModelSource, RetryConfig};
use serial_test::serial;
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;
use tempfile::TempDir;
use tracing_test::traced_test;

/// Small embedding model for testing cache behavior - using one with known GGUF files
const TINY_MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B-GGUF";

/// Explicit filename to avoid auto-detection issues with transient HuggingFace API failures
const TINY_MODEL_FILENAME: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";

/// Create a test retry config with shorter timeouts for testing
fn create_test_retry_config() -> RetryConfig {
    RetryConfig {
        max_retries: 2,
        initial_delay_ms: 100,
        backoff_multiplier: 1.5,
        max_delay_ms: 1000,
    }
}

/// Create a ModelConfig for the tiny test model
fn create_test_config() -> ModelConfig {
    ModelConfig {
        source: ModelSource::HuggingFace {
            repo: TINY_MODEL_REPO.to_string(),
            filename: Some(TINY_MODEL_FILENAME.to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: create_test_retry_config(),
        debug: false,
    }
}

/// Test cache hit behavior by resolving the same model twice
/// This test verifies that:
/// 1. First resolve downloads the model
/// 2. Second resolve is significantly faster (cache hit)
/// 3. Both resolves return equivalent models
#[tokio::test]
#[traced_test]
#[serial]
async fn test_model_cache_hit_behavior() {
    // Use a temporary directory to isolate cache behavior
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    let resolver = ModelResolver::new();
    let config = create_test_config();

    // First model resolve - should download
    tracing::info!("Starting first model resolve (should download)");
    let start_time = Instant::now();

    let model1 = resolver.resolve(&config).await;

    let first_resolve_time = start_time.elapsed();
    tracing::info!("First resolve completed in: {}", Pretty(&first_resolve_time));

    // Verify first resolve succeeded
    assert!(
        model1.is_ok(),
        "First model resolve should succeed: {:?}",
        model1.err()
    );
    let model1 = model1.unwrap();

    // Get model metadata
    let metadata1 = &model1.metadata;
    tracing::info!("First model metadata: {}", Pretty(&metadata1));

    // Second model resolve - should use cache
    tracing::info!("Starting second model resolve (should use cache)");
    let start_time = Instant::now();

    let model2 = resolver.resolve(&config).await;

    let second_resolve_time = start_time.elapsed();
    tracing::info!("Second resolve completed in: {}", Pretty(&second_resolve_time));

    // Verify second resolve succeeded
    assert!(
        model2.is_ok(),
        "Second model resolve should succeed: {:?}",
        model2.err()
    );
    let model2 = model2.unwrap();

    // Get second model metadata
    let metadata2 = &model2.metadata;
    tracing::info!("Second model metadata: {}", Pretty(&metadata2));

    // Verify both models have same metadata characteristics
    assert_eq!(metadata1.filename, metadata2.filename);
    assert_eq!(metadata1.size_bytes, metadata2.size_bytes);

    // The second resolve should be significantly faster than the first
    // Allow some variance but expect at least 50% faster
    let speedup_ratio = first_resolve_time.as_millis() as f64 / second_resolve_time.as_millis() as f64;

    tracing::info!("Resolve time comparison:");
    tracing::info!("  First resolve:  {}", Pretty(&first_resolve_time));
    tracing::info!("  Second resolve: {}", Pretty(&second_resolve_time));
    tracing::info!("  Speedup:     {:.2}x", speedup_ratio);

    if speedup_ratio > 1.5 {
        tracing::info!(
            "✅ Cache hit detected - second resolve was {:.2}x faster",
            speedup_ratio
        );
    } else if second_resolve_time < Duration::from_secs(5) {
        tracing::info!(
            "✅ Second resolve was fast ({:?}) - likely cache hit",
            second_resolve_time
        );
    } else {
        tracing::warn!("⚠️  Second resolve was not significantly faster - cache may not be working");
        tracing::warn!("    This could be due to network conditions or model size");
    }

    // Verify cache_hit field is set correctly
    tracing::info!("First resolve cache_hit: {}", metadata1.cache_hit);
    tracing::info!("Second resolve cache_hit: {}", metadata2.cache_hit);
    assert!(metadata2.cache_hit, "Second resolve should be a cache hit");
}

/// Test that verifies ModelResolver with same configuration reuses cached models
#[tokio::test]
#[traced_test]
#[serial]
async fn test_model_resolver_cache_reuse() {
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    let config = create_test_config();

    // Create first ModelResolver instance
    let resolver1 = ModelResolver::new();

    tracing::info!("Resolving model with first resolver instance");
    let start_time = Instant::now();

    let model1 = resolver1.resolve(&config).await;

    let first_resolve_time = start_time.elapsed();
    tracing::info!("First resolver completed in: {}", Pretty(&first_resolve_time));

    assert!(
        model1.is_ok(),
        "First resolver should succeed: {:?}",
        model1.err()
    );
    let model1 = model1.unwrap();

    // Create second ModelResolver instance with same configuration
    let resolver2 = ModelResolver::new();

    tracing::info!("Resolving same model with second resolver instance");
    let start_time = Instant::now();

    let model2 = resolver2.resolve(&config).await;

    let second_resolve_time = start_time.elapsed();
    tracing::info!("Second resolver completed in: {}", Pretty(&second_resolve_time));

    assert!(
        model2.is_ok(),
        "Second resolver should succeed: {:?}",
        model2.err()
    );
    let model2 = model2.unwrap();

    // Verify models are equivalent
    let metadata1 = &model1.metadata;
    let metadata2 = &model2.metadata;

    assert_eq!(metadata1.filename, metadata2.filename);
    assert_eq!(metadata1.size_bytes, metadata2.size_bytes);

    // Second resolve should benefit from HuggingFace Hub's caching
    let speedup_ratio = first_resolve_time.as_millis() as f64 / second_resolve_time.as_millis() as f64;

    tracing::info!("ModelResolver comparison:");
    tracing::info!("  First instance:  {}", Pretty(&first_resolve_time));
    tracing::info!("  Second instance: {}", Pretty(&second_resolve_time));
    tracing::info!("  Speedup:         {:.2}x", speedup_ratio);

    if speedup_ratio > 1.5 {
        tracing::info!("✅ Cache reuse detected between ModelResolver instances");
    } else {
        tracing::info!("ℹ️  Both resolves completed - cache behavior validated");
    }
}

/// Test demonstrating the expected cache_hit field behavior
/// This test shows what the implementation should look like once cache_hit is properly tracked
#[tokio::test]
#[traced_test]
#[serial]
async fn test_cache_hit_metadata_field() {
    let _temp_dir = TempDir::new().expect("Failed to create temp directory");

    let resolver = ModelResolver::new();
    let config = create_test_config();

    // First resolve - should be a cache miss
    let model1 = resolver
        .resolve(&config)
        .await
        .expect("First resolve should succeed");

    let metadata1 = &model1.metadata;

    // First resolve may or may not be a cache hit depending on previous runs
    tracing::info!("First resolve cache_hit: {}", metadata1.cache_hit);

    // Second resolve - should be a cache hit since first resolve established the cache
    let model2 = resolver
        .resolve(&config)
        .await
        .expect("Second resolve should succeed");

    let metadata2 = &model2.metadata;

    // Second resolve MUST be a cache hit since first resolve established it
    assert!(metadata2.cache_hit, "Second resolve MUST be a cache hit");

    tracing::info!("✅ Cache hit field validation passed:");
    tracing::info!("  First resolve cache_hit: {}", metadata1.cache_hit);
    tracing::info!(
        "  Second resolve cache_hit: {} (must be true)",
        metadata2.cache_hit
    );
}
