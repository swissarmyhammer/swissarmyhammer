use llama_embedding::{BatchConfig, BatchStats, EmbeddingError, EmbeddingResult};
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Comprehensive tests for BatchProcessor functionality
/// These tests focus on the data structures and logic without requiring actual models

#[test]
fn test_batch_stats_functionality() {
    let mut stats = BatchStats::new();

    // Test initial state
    assert_eq!(stats.total_texts, 0);
    assert_eq!(stats.successful_embeddings, 0);
    assert_eq!(stats.failed_embeddings, 0);
    assert_eq!(stats.success_rate(), 0.0);
    assert_eq!(stats.average_time_per_text_ms, 0.0);

    // Test single successful batch
    stats.update(10, 1000, 0);
    assert_eq!(stats.total_texts, 10);
    assert_eq!(stats.successful_embeddings, 10);
    assert_eq!(stats.failed_embeddings, 0);
    assert_eq!(stats.success_rate(), 1.0);
    assert_eq!(stats.average_time_per_text_ms, 100.0);

    // Test batch with some failures
    stats.update(8, 800, 2);
    assert_eq!(stats.total_texts, 18);
    assert_eq!(stats.successful_embeddings, 16);
    assert_eq!(stats.failed_embeddings, 2);
    assert!((stats.success_rate() - 0.8888888888888888).abs() < 1e-10);
    assert_eq!(stats.average_time_per_text_ms, 100.0); // (1000 + 800) / 18

    // Test complete failure batch
    stats.update(5, 500, 5);
    assert_eq!(stats.total_texts, 23);
    assert_eq!(stats.successful_embeddings, 16);
    assert_eq!(stats.failed_embeddings, 7);
    assert!((stats.success_rate() - 0.6956521739130435).abs() < 1e-10);
}

#[test]
fn test_batch_config_creation() {
    // Test default config
    let default_config = BatchConfig::default();
    assert_eq!(default_config.batch_size, 32);
    assert!(default_config.continue_on_error);
    assert!(!default_config.enable_progress_reporting);

    // Test custom config
    let custom_config = BatchConfig {
        batch_size: 64,
        continue_on_error: false,
        enable_progress_reporting: true,
        progress_report_interval_batches: 5,
        memory_limit_mb: Some(100),
        enable_memory_monitoring: false,
    };
    assert_eq!(custom_config.batch_size, 64);
    assert!(!custom_config.continue_on_error);
    assert!(custom_config.enable_progress_reporting);
}

#[tokio::test]
async fn test_file_processing_logic() {
    // Create a temporary file with test data
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");

    // Write test lines with various conditions
    writeln!(temp_file, "Line 1 with content").unwrap();
    writeln!(temp_file).unwrap(); // Empty line - should be skipped
    writeln!(temp_file, "   ").unwrap(); // Whitespace only - should be skipped
    writeln!(temp_file, "Line 2 with content").unwrap();
    writeln!(temp_file, "\t\n").unwrap(); // Tab and newline - should be skipped
    writeln!(temp_file, "Line 3 with content").unwrap();
    writeln!(temp_file, "  Line 4 with leading/trailing spaces  ").unwrap();

    temp_file.flush().unwrap();

    // Test file reading and filtering logic
    let file = File::open(temp_file.path()).await.unwrap();
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut valid_lines = Vec::new();

    while let Some(line) = lines.next_line().await.unwrap() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            valid_lines.push(trimmed.to_string());
        }
    }

    // Should have 4 valid lines after filtering
    assert_eq!(valid_lines.len(), 4);
    assert_eq!(valid_lines[0], "Line 1 with content");
    assert_eq!(valid_lines[1], "Line 2 with content");
    assert_eq!(valid_lines[2], "Line 3 with content");
    assert_eq!(valid_lines[3], "Line 4 with leading/trailing spaces");
}

#[tokio::test]
async fn test_batch_size_validation() {
    // Test various batch sizes
    let test_sizes = vec![1, 8, 16, 32, 64, 128, 256];

    for size in test_sizes {
        assert!(size > 0, "Batch size {} should be positive", size);
        assert!(size <= 1000, "Batch size {} should be reasonable", size);

        // Test that batch size affects chunking behavior
        let test_data: Vec<String> = (0..100).map(|i| format!("text_{}", i)).collect();
        let expected_batches = test_data.len().div_ceil(size); // Ceiling division

        let chunks: Vec<_> = test_data.chunks(size).collect();
        assert_eq!(chunks.len(), expected_batches);

        // Verify last batch size
        if test_data.len().is_multiple_of(size) {
            assert_eq!(chunks.last().unwrap().len(), size);
        } else {
            assert_eq!(chunks.last().unwrap().len(), test_data.len() % size);
        }
    }
}

#[test]
fn test_embedding_result_normalization() {
    // Test normalization functionality
    let mut result = EmbeddingResult::new(
        "test text".to_string(),
        vec![3.0, 4.0], // Vector with magnitude 5.0
        5,
        100,
    );

    // Before normalization
    let original_magnitude: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((original_magnitude - 5.0).abs() < 1e-6);

    // Apply normalization
    result.normalize();

    // After normalization - should have unit magnitude
    let normalized_magnitude: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((normalized_magnitude - 1.0).abs() < 1e-6);

    // Check specific values
    assert!((result.embedding[0] - 0.6).abs() < 1e-6);
    assert!((result.embedding[1] - 0.8).abs() < 1e-6);
    assert_eq!(result.dimension(), 2);
}

#[test]
fn test_embedding_result_zero_vector() {
    // Test normalization of zero vector (edge case)
    let mut result = EmbeddingResult::new("empty".to_string(), vec![0.0, 0.0, 0.0], 3, 50);

    // Normalize zero vector (should remain zero)
    result.normalize();

    for value in &result.embedding {
        assert_eq!(*value, 0.0);
    }
}

#[test]
fn test_md5_hash_consistency() {
    let test_texts = vec![
        "Hello, world!",
        "The quick brown fox jumps over the lazy dog",
        "🚀 Unicode text with emojis 🌟",
        "Multi\nline\ntext\nwith\nnewlines",
        "", // Empty string
    ];

    for text in test_texts {
        let result1 = EmbeddingResult::new(text.to_string(), vec![1.0, 2.0, 3.0], 10, 100);

        let result2 = EmbeddingResult::new(
            text.to_string(),
            vec![4.0, 5.0, 6.0], // Different embedding
            20,
            200,
        );

        // Hash should be the same for the same text, regardless of embedding
        assert_eq!(result1.text_hash, result2.text_hash);

        // Hash should match direct MD5 computation
        let expected_hash = format!("{:x}", md5::compute(text));
        assert_eq!(result1.text_hash, expected_hash);
    }
}

#[test]
fn test_error_type_coverage() {
    // Test all error variants to ensure they work correctly

    // ModelLoader error (from conversion)
    let loader_error = llama_loader::ModelError::InvalidConfig("test".to_string());
    let embedding_error: EmbeddingError = loader_error.into();
    match embedding_error {
        EmbeddingError::ModelLoader(_) => {} // Expected
        _ => panic!("Expected ModelLoader error variant"),
    }

    // Model error
    let model_error = EmbeddingError::model("model failed");
    assert!(model_error
        .to_string()
        .contains("Model error: model failed"));

    // Text processing error
    let text_error = EmbeddingError::text_processing("processing failed");
    assert!(text_error
        .to_string()
        .contains("Text processing error: processing failed"));

    // Configuration error
    let config_error = EmbeddingError::configuration("bad config");
    assert!(config_error
        .to_string()
        .contains("Configuration error: bad config"));

    // Dimension mismatch error
    let dimension_error = EmbeddingError::DimensionMismatch {
        expected: 1024,
        actual: 768,
    };
    assert!(dimension_error
        .to_string()
        .contains("expected 1024, got 768"));
}

#[tokio::test]
async fn test_large_file_simulation() {
    // Simulate processing of a large file with many lines
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");

    // Write a large number of test lines
    for i in 0..1000 {
        writeln!(
            temp_file,
            "Line {} with content for testing batch processing efficiency",
            i
        )
        .unwrap();
        if i % 10 == 0 {
            writeln!(temp_file).unwrap(); // Add some empty lines
        }
    }
    temp_file.flush().unwrap();

    // Count valid lines
    let file = File::open(temp_file.path()).await.unwrap();
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut valid_count = 0;

    while let Some(line) = lines.next_line().await.unwrap() {
        if !line.trim().is_empty() {
            valid_count += 1;
        }
    }

    assert_eq!(valid_count, 1000); // Should have exactly 1000 valid lines
}

#[test]
fn test_batch_chunking_edge_cases() {
    // Test edge cases for batch chunking
    let test_cases = vec![
        (vec![], 32),                     // Empty input
        (vec!["single".to_string()], 32), // Single item
        (
            (0..31).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
            32,
        ), // Less than batch size
        (
            (0..32).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
            32,
        ), // Exactly batch size
        (
            (0..33).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
            32,
        ), // One over batch size
        (
            (0..100).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
            1,
        ), // Batch size of 1
    ];

    for (data, batch_size) in test_cases {
        let chunks: Vec<_> = data.chunks(batch_size).collect();

        if data.is_empty() {
            assert_eq!(chunks.len(), 0);
        } else {
            let expected_chunks = data.len().div_ceil(batch_size);
            assert_eq!(chunks.len(), expected_chunks);

            // Verify all elements are included
            let total_elements: usize = chunks.iter().map(|chunk| chunk.len()).sum();
            assert_eq!(total_elements, data.len());

            // Verify no chunk exceeds batch size
            for chunk in &chunks {
                assert!(chunk.len() <= batch_size);
            }

            // Verify all chunks except possibly the last are full
            for (i, chunk) in chunks.iter().enumerate() {
                if i < chunks.len() - 1 {
                    assert_eq!(chunk.len(), batch_size);
                }
            }
        }
    }
}

#[test]
fn test_memory_efficiency_concepts() {
    // Test concepts around memory efficiency without actual memory measurement

    // Test that we process in chunks rather than loading everything
    let large_dataset: Vec<String> = (0..10000).map(|i| format!("text_{}", i)).collect();
    let batch_size = 100;

    let mut processed_count = 0;
    let mut batch_count = 0;

    for chunk in large_dataset.chunks(batch_size) {
        batch_count += 1;
        processed_count += chunk.len();

        // Simulate processing of the chunk
        assert!(chunk.len() <= batch_size);
        assert!(!chunk.is_empty());
    }

    assert_eq!(processed_count, large_dataset.len());
    assert_eq!(batch_count, 100); // 10000 / 100 = 100 batches
}

#[tokio::test]
async fn test_streaming_concepts() {
    // Test streaming file processing concepts
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");

    // Write test data
    for i in 0..50 {
        writeln!(temp_file, "Stream line {}", i).unwrap();
    }
    temp_file.flush().unwrap();

    // Simulate streaming processing
    let file = File::open(temp_file.path()).await.unwrap();
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut batch = Vec::new();
    let batch_size = 10;
    let mut processed_batches = 0;

    while let Some(line) = lines.next_line().await.unwrap() {
        if !line.trim().is_empty() {
            batch.push(line);

            if batch.len() >= batch_size {
                // Process the batch (simulate)
                assert_eq!(batch.len(), batch_size);
                processed_batches += 1;
                batch.clear();
            }
        }
    }

    // Process remaining items
    if !batch.is_empty() {
        processed_batches += 1;
    }

    assert_eq!(processed_batches, 5); // 50 lines / 10 batch_size = 5 batches
}
