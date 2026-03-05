//! Comprehensive integration tests for llama-embedding library
//!
//! All tests exercise the `TextEmbedder` trait interface, validating that
//! `EmbeddingModel` works correctly as a `dyn TextEmbedder` implementation.
//!
//! Tests cover:
//! - Single text embedding with dimension validation
//! - Batch processing with various sizes
//! - File processing with different scales
//! - Performance validation
//! - MD5 hash consistency
//! - Error handling scenarios
//! - Cache integration

use llama_embedding::{BatchProcessor, EmbeddingConfig, EmbeddingModel, TextEmbedder};
use model_loader::ModelSource;
use rstest::rstest;
use serial_test::serial;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;
use tempfile::NamedTempFile;

/// Default batch size for file processing tests
const TEST_FILE_BATCH_SIZE: usize = 32;

/// Test texts covering various scenarios as specified in the issue
const TEST_TEXTS: &[&str] = &[
    "Hello world, this is a test sentence.",
    "The quick brown fox jumps over the lazy dog.",
    "Artificial intelligence is transforming our world.",
    "短い日本語のテスト文です。", // Unicode/multilingual
    "", // Empty string edge case will be handled separately
    "This is a much longer text that will test how the embedding model handles sequences of varying lengths and complexity, including punctuation, numbers like 123, and mixed content. The purpose is to validate that the embedding model can handle realistic text inputs with diverse characteristics.",
    "Simple text.",
    "Text with numbers: 12345 and symbols @#$%",
    "Multiple sentences. First one is short. Second one is a bit longer with more content to test sequence handling.",
    "🚀 Emojis and unicode characters: café naïve résumé"
];

/// Helper function to create Qwen embedding config for testing.
///
/// Uses max_sequence_length=512 to limit KV cache allocation (the model's
/// full context window is 32K+ which allocates several GB of KV cache).
/// Test texts are short so 512 tokens is more than sufficient.
fn create_qwen_config() -> EmbeddingConfig {
    EmbeddingConfig {
        model_source: ModelSource::HuggingFace {
            repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
            filename: Some("Qwen3-Embedding-0.6B-Q8_0.gguf".to_string()),
            folder: None,
        },
        normalize_embeddings: false,
        max_sequence_length: Some(512),
        debug: false,
    }
}

/// Helper function to create test data file with specified number of texts
async fn create_test_file(num_texts: usize) -> Result<NamedTempFile, Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let mut writer = BufWriter::new(&temp_file);

    for i in 0..num_texts {
        let text_index = i % TEST_TEXTS.len();
        // Skip empty string for file tests
        if !TEST_TEXTS[text_index].is_empty() {
            writeln!(writer, "{}", TEST_TEXTS[text_index])?;
        } else {
            writeln!(writer, "Fallback text for index {}", i)?;
        }
    }

    writer.flush()?;
    drop(writer);
    Ok(temp_file)
}

/// Helper: create model and load via trait
async fn create_and_load() -> EmbeddingModel {
    let config = create_qwen_config();
    let model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");
    model.load().await.expect("Failed to load model");
    model
}

/// Test 1: Single Text Embedding with Dimension Validation (via trait)
#[tokio::test]
#[serial]
async fn test_single_text_embedding() {
    let model = create_and_load().await;

    // Test embedding dimension via trait
    assert_eq!(
        model.embedding_dimension(),
        Some(1024),
        "Qwen3-Embedding-0.6B should have 1024 dimensions"
    );

    // Test single text embedding via trait
    let result = model
        .embed_text("Hello world")
        .await
        .expect("Failed to generate embedding");

    assert_eq!(result.embedding().len(), 1024);
    assert!(!result.text_hash().is_empty());
    assert_eq!(result.text(), "Hello world");
    assert!(result.processing_time_ms() > 0);
    assert!(result.sequence_length() > 0);
}

/// Test 2: Model Loading (HuggingFace and caching) via trait
#[tokio::test]
#[serial]
async fn test_model_loading_and_caching() {
    let config = create_qwen_config();
    let model1 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create first model instance");

    // Load via trait
    model1.load().await.expect("Failed to load model first time");

    // Check via trait
    assert!(model1.is_loaded());
    assert_eq!(model1.embedding_dimension(), Some(1024));

    // Metadata is llama-specific (not on trait), still valid to check
    let metadata = model1.metadata();
    assert!(metadata.is_some());

    // Second instance loads via trait (should hit HF cache)
    let model2 = EmbeddingModel::new(config)
        .await
        .expect("Failed to create second model instance");

    model2.load().await.expect("Failed to load model second time");

    assert_eq!(model1.embedding_dimension(), Some(1024));
    assert_eq!(model2.embedding_dimension(), Some(1024));
}

/// Test 3: Batch Processing Tests with Various Sizes
#[tokio::test]
#[serial]
async fn test_batch_processing_various_sizes() {
    let model = create_and_load().await;

    let batch_sizes = vec![1, 8, 32, 64];
    let test_texts: Vec<String> = TEST_TEXTS
        .iter()
        .filter(|t| !t.is_empty()) // Skip empty strings
        .map(|s| s.to_string())
        .collect();

    for batch_size in batch_sizes {
        let mut processor = BatchProcessor::new(&model, batch_size);
        assert_eq!(processor.batch_size(), batch_size);

        let results = processor
            .process_batch(&test_texts)
            .await
            .unwrap_or_else(|_| panic!("Failed to process batch of size {}", batch_size));

        assert_eq!(results.len(), test_texts.len());

        for result in &results {
            assert_eq!(result.dimension(), 1024);
            assert!(result.processing_time_ms() > 0);
            assert!(!result.text_hash().is_empty());
        }
    }
}

/// Test 4: Batch Consistency (same results as individual processing) via trait
#[tokio::test]
#[serial]
async fn test_batch_consistency() {
    let model = create_and_load().await;

    let test_text = "This is a consistency test sentence.";

    // Generate individual embedding via trait
    let individual_result = model
        .embed_text(test_text)
        .await
        .expect("Failed to generate individual embedding");

    // Generate batch embedding
    let mut processor = BatchProcessor::new(&model, 1);
    let batch_results = processor
        .process_batch(&[test_text.to_string()])
        .await
        .expect("Failed to process batch");

    assert_eq!(batch_results.len(), 1);
    let batch_result = &batch_results[0];

    // Results should be identical
    assert_eq!(individual_result.text_hash(), batch_result.text_hash());
    assert_eq!(individual_result.dimension(), batch_result.dimension());
    assert_eq!(
        individual_result.sequence_length(),
        batch_result.sequence_length()
    );

    // Embeddings should be very similar (allowing for minor floating point differences)
    let similarity = cosine_similarity(individual_result.embedding(), batch_result.embedding());
    assert!(
        similarity > 0.999,
        "Embeddings should be nearly identical, similarity: {}",
        similarity
    );
}

/// Test 5: File Processing Tests with different sizes
#[rstest]
#[case(16)]
#[case(32)]
#[case(64)]
#[tokio::test]
#[serial]
async fn test_file_processing_different_sizes(#[case] file_size: usize) {
    let model = create_and_load().await;

    // Create test file
    let temp_file = create_test_file(file_size)
        .await
        .expect("Failed to create test file");

    let mut processor = BatchProcessor::new(&model, TEST_FILE_BATCH_SIZE);

    let start_time = Instant::now();
    let results = processor
        .process_file(temp_file.path())
        .await
        .unwrap_or_else(|_| panic!("Failed to process file with {} texts", file_size));
    let processing_time = start_time.elapsed();

    assert_eq!(results.len(), file_size);

    for result in &results {
        assert_eq!(result.dimension(), 1024);
        assert!(result.processing_time_ms() > 0);
        assert!(!result.text_hash().is_empty());
        assert!(!result.text().trim().is_empty());
    }

    // Processing time should scale roughly linearly
    let avg_time_per_text = processing_time.as_millis() as f64 / file_size as f64;
    if file_size >= 100 {
        assert!(
            avg_time_per_text < 200.0,
            "Average processing time per text should be reasonable: {:.2}ms",
            avg_time_per_text
        );
    }
}

/// Test 7: MD5 Hash Consistency Tests via trait
#[tokio::test]
#[serial]
async fn test_md5_hash_consistency() {
    let model = create_and_load().await;

    let test_text = "Hash consistency test text";

    // Generate embedding multiple times via trait
    let result1 = model
        .embed_text(test_text)
        .await
        .expect("Failed to generate first embedding");

    let result2 = model
        .embed_text(test_text)
        .await
        .expect("Failed to generate second embedding");

    let result3 = model
        .embed_text(test_text)
        .await
        .expect("Failed to generate third embedding");

    // Hash should be identical across runs
    assert_eq!(result1.text_hash(), result2.text_hash());
    assert_eq!(result2.text_hash(), result3.text_hash());

    // Text should be identical
    assert_eq!(result1.text(), result2.text());
    assert_eq!(result2.text(), result3.text());

    // Test different texts produce different hashes
    let different_result = model
        .embed_text("Different text")
        .await
        .expect("Failed to generate different text embedding");

    assert_ne!(result1.text_hash(), different_result.text_hash());

    // Verify MD5 hash is correct
    let expected_hash = format!("{:x}", md5::compute(test_text));
    assert_eq!(result1.text_hash(), expected_hash);
}

/// Test 8: Error Handling Tests via trait
#[tokio::test]
#[serial]
async fn test_error_handling() {
    // Test model not loaded error via trait
    let config = create_qwen_config();
    let model = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create embedding model");

    // embed_text before load should return Backend error wrapping ModelNotLoaded
    let result = TextEmbedder::embed_text(&model, "test").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, model_embedding::EmbeddingError::Backend(_)),
        "Expected Backend variant, got: {:?}",
        err
    );

    // Test empty text handling via trait
    let model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load().await.expect("Failed to load model");

    let empty_result = TextEmbedder::embed_text(&model, "").await;
    assert!(empty_result.is_err());

    // Test invalid file processing
    let mut processor = BatchProcessor::new(&model, TEST_FILE_BATCH_SIZE);

    let non_existent_file = Path::new("/tmp/non_existent_file.txt");
    let file_result = processor.process_file(non_existent_file).await;
    assert!(file_result.is_err());
}

/// Test 9: Integration with model-loader (cache scenarios) via trait
#[tokio::test]
#[serial]
async fn test_model_loader_integration() {
    let config = create_qwen_config();

    // Test multiple model instances share cache
    let model1 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create first model");

    model1.load().await.expect("Failed to load first model");

    let model2 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create second model");

    model2.load().await.expect("Failed to load second model");

    // Both should have same embedding dimension via trait
    assert_eq!(model1.embedding_dimension(), model2.embedding_dimension());
    assert_eq!(model1.embedding_dimension(), Some(1024));

    // Test embeddings are consistent between instances via trait
    let text = "Cache integration test";
    let result1 = model1
        .embed_text(text)
        .await
        .expect("Failed to generate embedding with model1");
    let result2 = model2
        .embed_text(text)
        .await
        .expect("Failed to generate embedding with model2");

    assert_eq!(result1.text_hash(), result2.text_hash());
    assert_eq!(result1.dimension(), result2.dimension());
}

/// Test 10: Edge Cases and Text Handling via trait
#[tokio::test]
#[serial]
async fn test_edge_cases_and_text_handling() {
    let model = create_and_load().await;

    // Test various edge cases via trait
    let edge_cases = [
        "Single word",
        "A", // Very short
        "  whitespace padded  ",
        "Numbers: 12345",
        "Symbols: @#$%^&*()",
        "Unicode: café naïve résumé 🚀",
        "Mixed: Hello 世界 123 @test",
    ];

    for (i, test_case) in edge_cases.iter().enumerate() {
        let result = model
            .embed_text(test_case)
            .await
            .unwrap_or_else(|_| panic!("Failed to process edge case {}: '{}'", i, test_case));

        assert_eq!(result.dimension(), 1024);
        assert_eq!(result.text().trim(), test_case.trim());
        assert!(result.processing_time_ms() > 0);
        assert!(!result.text_hash().is_empty());
    }
}

/// Test 11: Normalization functionality via trait
#[tokio::test]
#[serial]
async fn test_embedding_normalization() {
    let mut config = create_qwen_config();
    config.normalize_embeddings = true;

    let model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load().await.expect("Failed to load model");

    let result = model
        .embed_text("Normalization test")
        .await
        .expect("Failed to generate normalized embedding");

    // Check that embedding is normalized (L2 norm should be ~1.0)
    let magnitude: f32 = result.embedding().iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-5,
        "Normalized embedding should have magnitude ~1.0, got: {}",
        magnitude
    );
}

/// Test: trait object usage — proves EmbeddingModel works as dyn TextEmbedder
#[tokio::test]
#[serial]
async fn test_trait_object_usage() {
    let model = create_and_load().await;

    // Use as a trait object — this is the whole point of the refactor
    let embedder: &dyn TextEmbedder = &model;

    assert!(embedder.is_loaded());
    assert_eq!(embedder.embedding_dimension(), Some(1024));

    let result = embedder
        .embed_text("Trait object test")
        .await
        .expect("Failed to embed via trait object");

    assert_eq!(result.embedding().len(), 1024);
    assert_eq!(result.text(), "Trait object test");
}

/// Test: generic function accepts any TextEmbedder
#[tokio::test]
#[serial]
async fn test_generic_embedder_function() {
    let model = create_and_load().await;

    async fn embed_via_trait(
        embedder: &dyn TextEmbedder,
        text: &str,
    ) -> model_embedding::EmbeddingResult {
        embedder
            .embed_text(text)
            .await
            .expect("embed via trait failed")
    }

    let result = embed_via_trait(&model, "Generic function test").await;
    assert_eq!(result.embedding().len(), 1024);
    assert_eq!(result.text(), "Generic function test");
}

/// Helper function to calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}
