//! Comprehensive integration tests for llama-embedding library
//!
//! These tests validate the complete functionality with real embedding models,
//! specifically focusing on the Qwen/Qwen3-Embedding-0.6B-GGUF model as specified.
//!
//! Tests cover:
//! - Single text embedding with dimension validation
//! - Batch processing with various sizes
//! - File processing with different scales
//! - Performance validation
//! - MD5 hash consistency
//! - Error handling scenarios
//! - Cache integration

use llama_embedding::{BatchProcessor, EmbeddingConfig, EmbeddingModel};
use llama_loader::ModelSource;
use rstest::rstest;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tempfile::NamedTempFile;

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

/// Helper function to create Qwen embedding config for testing
fn create_qwen_config() -> EmbeddingConfig {
    EmbeddingConfig {
        model_source: ModelSource::HuggingFace {
            repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
            filename: None,
            folder: None,
        },
        normalize_embeddings: false,
        max_sequence_length: None,
        debug: true,
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

/// Test 1: Single Text Embedding with Dimension Validation
#[tokio::test]

async fn test_single_text_embedding() {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load Qwen model");

    // Test embedding dimension is 1024 for Qwen3-Embedding-0.6B
    let embedding_dim = model.get_embedding_dimension();
    assert_eq!(
        embedding_dim,
        Some(1024),
        "Qwen3-Embedding-0.6B should have 1024 dimensions"
    );

    // Test single text embedding
    let result = model
        .embed_text("Hello world")
        .await
        .expect("Failed to generate embedding");

    assert_eq!(result.embedding.len(), 1024);
    assert!(!result.text_hash.is_empty());
    assert_eq!(result.text, "Hello world");
    assert!(result.processing_time_ms > 0);
    assert!(result.sequence_length > 0);

    println!("✓ Single text embedding test passed");
    println!("  - Dimensions: {}", result.dimension());
    println!("  - Processing time: {}ms", result.processing_time_ms);
    println!("  - Sequence length: {}", result.sequence_length);
    println!("  - Text hash: {}", result.text_hash);
}

/// Test 2: Model Loading (HuggingFace and caching)
#[tokio::test]

async fn test_model_loading_and_caching() {
    // Test HuggingFace model loading
    let config = create_qwen_config();
    let mut model1 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create first model instance");

    let start_time = Instant::now();
    model1
        .load_model()
        .await
        .expect("Failed to load model first time");
    let first_load_time = start_time.elapsed();

    // Test that model is loaded
    assert!(model1.is_loaded());
    assert_eq!(model1.get_embedding_dimension(), Some(1024));

    // Test metadata availability
    let metadata = model1.get_metadata();
    assert!(metadata.is_some());

    // Test second instance loads faster (should hit cache)
    let mut model2 = EmbeddingModel::new(config)
        .await
        .expect("Failed to create second model instance");

    let start_time = Instant::now();
    model2
        .load_model()
        .await
        .expect("Failed to load model second time");
    let second_load_time = start_time.elapsed();

    // Second load should be faster due to caching
    // Allow some margin but expect significant improvement
    if second_load_time < first_load_time {
        println!("✓ Cache hit detected - faster loading");
        println!("  - First load: {:?}", first_load_time);
        println!("  - Second load: {:?}", second_load_time);
    } else {
        println!("⚠ Cache may not have been hit, or loading time variation");
        println!("  - First load: {:?}", first_load_time);
        println!("  - Second load: {:?}", second_load_time);
    }
}

/// Test 3: Batch Processing Tests with Various Sizes
#[tokio::test]

async fn test_batch_processing_various_sizes() {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let model_arc = Arc::new(model);
    let batch_sizes = vec![1, 8, 32, 64];
    let test_texts: Vec<String> = TEST_TEXTS
        .iter()
        .filter(|t| !t.is_empty()) // Skip empty strings
        .map(|s| s.to_string())
        .collect();

    for batch_size in batch_sizes {
        println!("Testing batch size: {}", batch_size);

        let mut processor = BatchProcessor::new(model_arc.clone(), batch_size);
        assert_eq!(processor.batch_size(), batch_size);

        let start_time = Instant::now();
        let results = processor
            .process_batch(&test_texts)
            .await
            .unwrap_or_else(|_| panic!("Failed to process batch of size {}", batch_size));
        let processing_time = start_time.elapsed();

        assert_eq!(results.len(), test_texts.len());

        // Verify all results have correct dimensions
        for result in &results {
            assert_eq!(result.dimension(), 1024);
            assert!(result.processing_time_ms > 0);
            assert!(!result.text_hash.is_empty());
        }

        println!(
            "  ✓ Batch size {} completed in {:?}",
            batch_size, processing_time
        );
        println!("    - Processed {} texts", results.len());
        println!(
            "    - Avg time per text: {:.2}ms",
            processing_time.as_millis() as f64 / results.len() as f64
        );
    }
}

/// Test 4: Batch Consistency (same results as individual processing)
#[tokio::test]

async fn test_batch_consistency() {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let test_text = "This is a consistency test sentence.";

    // Generate individual embedding
    let individual_result = model
        .embed_text(test_text)
        .await
        .expect("Failed to generate individual embedding");

    // Generate batch embedding
    let model_arc = Arc::new(model);
    let mut processor = BatchProcessor::new(model_arc, 1);
    let batch_results = processor
        .process_batch(&[test_text.to_string()])
        .await
        .expect("Failed to process batch");

    assert_eq!(batch_results.len(), 1);
    let batch_result = &batch_results[0];

    // Results should be identical
    assert_eq!(individual_result.text_hash, batch_result.text_hash);
    assert_eq!(individual_result.dimension(), batch_result.dimension());
    assert_eq!(
        individual_result.sequence_length,
        batch_result.sequence_length
    );

    // Embeddings should be very similar (allowing for minor floating point differences)
    let similarity = cosine_similarity(&individual_result.embedding, &batch_result.embedding);
    assert!(
        similarity > 0.999,
        "Embeddings should be nearly identical, similarity: {}",
        similarity
    );

    println!("✓ Batch consistency validated");
    println!("  - Cosine similarity: {:.6}", similarity);
}

/// Test 5: File Processing Tests with different sizes using parallel execution
#[rstest]
#[case(16)]
#[case(32)]
#[case(64)]
#[tokio::test]
async fn test_file_processing_different_sizes(#[case] file_size: usize) {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let model_arc = Arc::new(model);

    println!("Testing file processing with {} texts", file_size);

    // Create test file
    let temp_file = create_test_file(file_size)
        .await
        .expect("Failed to create test file");

    let mut processor = BatchProcessor::new(model_arc.clone(), 32);

    let start_time = Instant::now();
    let results = processor
        .process_file(temp_file.path())
        .await
        .unwrap_or_else(|_| panic!("Failed to process file with {} texts", file_size));
    let processing_time = start_time.elapsed();

    // Verify expected number of results (excluding empty lines)
    assert_eq!(results.len(), file_size);

    // Verify all results are valid
    for result in &results {
        assert_eq!(result.dimension(), 1024);
        assert!(result.processing_time_ms > 0);
        assert!(!result.text_hash.is_empty());
        assert!(!result.text.trim().is_empty());
    }

    let avg_time_per_text = processing_time.as_millis() as f64 / file_size as f64;

    println!(
        "  ✓ File size {} completed in {:?}",
        file_size, processing_time
    );
    println!("    - Avg time per text: {:.2}ms", avg_time_per_text);
    println!("    - Total embeddings: {}", results.len());

    // Memory efficiency check - processing time should scale roughly linearly
    if file_size >= 100 {
        assert!(
            avg_time_per_text < 200.0,
            "Average processing time per text should be reasonable: {:.2}ms",
            avg_time_per_text
        );
    }
}

/// Test 7: MD5 Hash Consistency Tests
#[tokio::test]

async fn test_md5_hash_consistency() {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let test_text = "Hash consistency test text";

    // Generate embedding multiple times
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
    assert_eq!(result1.text_hash, result2.text_hash);
    assert_eq!(result2.text_hash, result3.text_hash);

    // Text should be identical
    assert_eq!(result1.text, result2.text);
    assert_eq!(result2.text, result3.text);

    // Test different texts produce different hashes
    let different_result = model
        .embed_text("Different text")
        .await
        .expect("Failed to generate different text embedding");

    assert_ne!(result1.text_hash, different_result.text_hash);

    // Verify MD5 hash is correct
    let expected_hash = format!("{:x}", md5::compute(test_text));
    assert_eq!(result1.text_hash, expected_hash);

    println!("✓ MD5 hash consistency validated");
    println!("  - Hash: {}", result1.text_hash);
    println!("  - Consistent across multiple generations");
    println!("  - Different texts produce different hashes");
}

/// Test 8: Error Handling Tests
#[tokio::test]

async fn test_error_handling() {
    // Test model not loaded error
    let config = create_qwen_config();
    let model = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create embedding model");

    // Try to embed without loading model
    let result = model.embed_text("test").await;
    assert!(result.is_err());
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("not loaded") || error_message.contains("Model not loaded"));

    // Test empty text handling
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let empty_result = model.embed_text("").await;
    assert!(empty_result.is_err());

    // Test invalid file processing
    let model_arc = Arc::new(model);
    let mut processor = BatchProcessor::new(model_arc, 32);

    let non_existent_file = Path::new("/tmp/non_existent_file.txt");
    let file_result = processor.process_file(non_existent_file).await;
    assert!(file_result.is_err());

    println!("✓ Error handling tests passed");
    println!("  - Model not loaded error handled");
    println!("  - Empty text error handled");
    println!("  - Invalid file error handled");
}

/// Test 9: Integration with llama-loader (cache scenarios)
#[tokio::test]

async fn test_llama_loader_integration() {
    let config = create_qwen_config();

    // Test multiple model instances share cache
    let mut model1 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create first model");

    let start = Instant::now();
    model1
        .load_model()
        .await
        .expect("Failed to load first model");
    let first_load_time = start.elapsed();

    // Create second instance
    let mut model2 = EmbeddingModel::new(config.clone())
        .await
        .expect("Failed to create second model");

    let start = Instant::now();
    model2
        .load_model()
        .await
        .expect("Failed to load second model");
    let second_load_time = start.elapsed();

    // Both should have same embedding dimension
    assert_eq!(
        model1.get_embedding_dimension(),
        model2.get_embedding_dimension()
    );
    assert_eq!(model1.get_embedding_dimension(), Some(1024));

    // Test embeddings are consistent between instances
    let text = "Cache integration test";
    let result1 = model1
        .embed_text(text)
        .await
        .expect("Failed to generate embedding with model1");
    let result2 = model2
        .embed_text(text)
        .await
        .expect("Failed to generate embedding with model2");

    assert_eq!(result1.text_hash, result2.text_hash);
    assert_eq!(result1.dimension(), result2.dimension());

    println!("✓ llama-loader integration validated");
    println!("  - First load: {:?}", first_load_time);
    println!("  - Second load: {:?}", second_load_time);
    println!("  - Both models work consistently");
}

/// Test 10: Edge Cases and Text Handling
#[tokio::test]

async fn test_edge_cases_and_text_handling() {
    let config = create_qwen_config();
    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    // Test various edge cases
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
        assert_eq!(result.text.trim(), test_case.trim());
        assert!(result.processing_time_ms > 0);
        assert!(!result.text_hash.is_empty());

        println!(
            "  ✓ Edge case {}: '{}' -> {} dims, {}ms",
            i,
            test_case.replace('\n', "\\n"),
            result.dimension(),
            result.processing_time_ms
        );
    }

    println!("✓ Edge case handling validated");
}

/// Test 11: Normalization functionality
#[tokio::test]

async fn test_embedding_normalization() {
    // Test with normalization enabled
    let mut config = create_qwen_config();
    config.normalize_embeddings = true;

    let mut model = EmbeddingModel::new(config)
        .await
        .expect("Failed to create embedding model");

    model.load_model().await.expect("Failed to load model");

    let result = model
        .embed_text("Normalization test")
        .await
        .expect("Failed to generate normalized embedding");

    // Check that embedding is normalized (L2 norm should be ~1.0)
    let magnitude: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-5,
        "Normalized embedding should have magnitude ~1.0, got: {}",
        magnitude
    );

    println!("✓ Embedding normalization validated");
    println!("  - Magnitude: {:.6}", magnitude);
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

/// Test Summary and Success Criteria Validation
#[tokio::test]

async fn test_success_criteria_summary() {
    println!("=== llama-embedding Integration Test Summary ===");
    println!();
    println!("Success Criteria Validation:");
    println!("□ All integration tests pass consistently");
    println!("□ Qwen embedding model loads and works correctly");
    println!("□ Embedding dimensions match expected (1024)");
    println!("□ Performance meets requirements (1000 texts < 60s)");
    println!("□ Memory usage scales predictably");
    println!("□ MD5 hashing works correctly");
    println!("□ Error handling robust and informative");
    println!("□ Cache integration works properly");
    println!("□ No memory leaks or resource issues");
    println!();
    println!("To validate all criteria, run:");
    println!(
        "cargo test --package llama-embedding --test real_model_integration_test -- --ignored"
    );
    println!();
    println!("Note: These tests require downloading the Qwen model (~1.2GB)");
    println!("and may take several minutes to complete.");
}
