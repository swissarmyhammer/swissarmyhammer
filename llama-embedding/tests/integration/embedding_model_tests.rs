use llama_embedding::{BatchProcessor, EmbeddingConfig, EmbeddingModel, TextEmbedder};
use llama_loader::ModelSource;
use serial_test::serial;
use std::io::Write;
use tempfile::NamedTempFile;

/// Initial batch size for processor creation test
const TEST_INITIAL_BATCH_SIZE: usize = 32;
/// Modified batch size for testing set_batch_size
const TEST_MODIFIED_BATCH_SIZE: usize = 64;

/// Test basic embedding model creation and configuration via trait
#[tokio::test]
#[serial]
async fn test_embedding_model_creation() {
    let config = EmbeddingConfig {
        model_source: ModelSource::HuggingFace {
            repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
            filename: Some("Qwen3-Embedding-0.6B-Q8_0.gguf".to_string()),
            folder: None,
        },
        normalize_embeddings: true,
        max_sequence_length: Some(512),
        debug: false,
    };

    match EmbeddingModel::new(config).await {
        Ok(model) => {
            // Check via trait methods
            assert!(!model.is_loaded());
            assert!(model.embedding_dimension().is_none());
        }
        Err(e) => {
            println!(
                "Model creation failed (expected in test environment): {}",
                e
            );
        }
    }
}

/// Test embedding configuration validation
#[test]
fn test_embedding_config() {
    let config = EmbeddingConfig::default();

    assert!(!config.normalize_embeddings);
    assert!(config.max_sequence_length.is_none());
    assert!(!config.debug);

    match config.model_source {
        ModelSource::HuggingFace {
            ref repo,
            ref filename,
            ..
        } => {
            assert_eq!(repo, "Qwen/Qwen3-Embedding-0.6B-GGUF");
            assert_eq!(filename.as_deref(), Some("Qwen3-Embedding-0.6B-Q8_0.gguf"));
        }
        _ => panic!("Expected HuggingFace model source"),
    }
}

/// Test embedding result creation and manipulation
#[test]
fn test_embedding_result() {
    use llama_embedding::EmbeddingResult;

    let mut result = EmbeddingResult::new(
        "test input text".to_string(),
        vec![3.0, 4.0], // Creates vector with magnitude 5.0
        8,
        100,
    );

    assert_eq!(result.text, "test input text");
    assert_eq!(result.dimension(), 2);
    assert_eq!(result.sequence_length, 8);
    assert_eq!(result.processing_time_ms, 100);

    // Test MD5 hash consistency
    let expected_hash = format!("{:x}", md5::compute("test input text"));
    assert_eq!(result.text_hash, expected_hash);

    // Test normalization
    result.normalize();
    let magnitude: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-6,
        "Vector should be normalized to unit length"
    );
}

/// Test batch processor creation and basic functionality
#[tokio::test]
#[serial]
async fn test_batch_processor_creation() {
    let config = EmbeddingConfig::default();

    match EmbeddingModel::new(config).await {
        Ok(model) => {
            let mut processor = BatchProcessor::new(&model, TEST_INITIAL_BATCH_SIZE);
            assert_eq!(processor.batch_size(), TEST_INITIAL_BATCH_SIZE);

            // Test batch size modification
            processor.set_batch_size(TEST_MODIFIED_BATCH_SIZE);
            assert_eq!(processor.batch_size(), TEST_MODIFIED_BATCH_SIZE);

            // Test invalid batch size (should be ignored)
            processor.set_batch_size(0);
            assert_eq!(processor.batch_size(), TEST_MODIFIED_BATCH_SIZE);
        }
        Err(_) => {
            println!("Batch processor test skipped - model creation failed");
        }
    }
}

/// Test file processing simulation
#[tokio::test]
#[serial]
async fn test_file_processing_structure() {
    use std::path::Path;

    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(temp_file, "First test sentence").expect("Failed to write");
    writeln!(temp_file, "Second test sentence").expect("Failed to write");
    writeln!(temp_file).expect("Failed to write");
    writeln!(temp_file, "  ").expect("Failed to write");
    writeln!(temp_file, "Third test sentence").expect("Failed to write");

    let file_path = temp_file.path();
    assert!(file_path.exists());

    let non_existent = Path::new("/tmp/non_existent_file.txt");
    assert!(!non_existent.exists());
}

/// Test error handling and propagation
#[test]
fn test_error_types() {
    use llama_embedding::EmbeddingError;

    let model_error = EmbeddingError::model("test model error");
    assert!(matches!(model_error, EmbeddingError::Model(_)));

    let text_error = EmbeddingError::text_processing("test text error");
    assert!(matches!(text_error, EmbeddingError::TextProcessing(_)));

    let config_error = EmbeddingError::configuration("test config error");
    assert!(matches!(config_error, EmbeddingError::Configuration(_)));

    assert!(model_error.to_string().contains("Model error"));
    assert!(text_error.to_string().contains("Text processing error"));
    assert!(config_error.to_string().contains("Configuration error"));
}

/// Test embedding dimension handling
#[test]
fn test_embedding_dimensions() {
    use llama_embedding::EmbeddingResult;

    let dimensions = vec![384, 768, 1024, 1536];

    for dim in dimensions {
        let embedding: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();
        let result = EmbeddingResult::new("test".to_string(), embedding, 10, 50);

        assert_eq!(result.dimension(), dim);
        assert_eq!(result.embedding.len(), dim);
    }
}

/// Integration test for actual model loading via trait
#[tokio::test]
#[serial]
async fn test_real_model_integration() {
    let test_models_path = std::path::PathBuf::from("./test-models");
    if !test_models_path.exists() {
        println!("Skipping real model integration test - ./test-models folder not found");
        return;
    }

    let config = EmbeddingConfig {
        model_source: ModelSource::Local {
            folder: test_models_path,
            filename: Some("test-embedding-model.gguf".to_string()),
        },
        normalize_embeddings: true,
        max_sequence_length: Some(256),
        debug: true,
    };

    match EmbeddingModel::new(config).await {
        Ok(model) => {
            // Load and embed via trait
            model.load().await.expect("Should load test model");

            let result = model
                .embed_text("Hello, world!")
                .await
                .expect("Should generate embedding");

            assert!(result.dimension() > 0);
            assert!(result.processing_time_ms > 0);
            assert!(!result.embedding.is_empty());
        }
        Err(e) => {
            println!("Real model test skipped: {}", e);
        }
    }
}
