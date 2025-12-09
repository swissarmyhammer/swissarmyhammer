use llama_loader::{ModelConfig, ModelSource, RetryConfig};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Create a test GGUF file with some content
async fn create_test_gguf_file(path: &PathBuf, content: &[u8]) -> Result<(), std::io::Error> {
    let mut file = File::create(path).await?;
    file.write_all(content).await?;
    file.sync_all().await?;
    Ok(())
}

/// Test that ModelConfig can be created successfully
#[tokio::test]
async fn test_model_config_creation() {
    let temp_dir = TempDir::new().unwrap();
    let retry_config = RetryConfig::default();

    // Note: We can't create a real LlamaBackend in tests without proper initialization
    // This is mainly testing the structure compilation

    // Test that we can create the config structs
    let model_config = ModelConfig {
        source: ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: Some("test.gguf".to_string()),
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: retry_config.clone(),
        debug: false,
    };

    assert!(model_config.validate().is_err()); // Should fail because file doesn't exist
}

#[tokio::test]
async fn test_model_config_validation() {
    let _temp_dir = TempDir::new().unwrap();

    // Valid config
    let valid_config = ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "microsoft/DialoGPT-medium".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    };
    assert!(valid_config.validate().is_ok());

    // Invalid batch size
    let invalid_config = ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "microsoft/DialoGPT-medium".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: None,
        },
        batch_size: 0, // Invalid
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    };
    assert!(invalid_config.validate().is_err());

    // Invalid HuggingFace repo format
    let invalid_hf_config = ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "invalid-repo-format".to_string(), // Missing '/'
            filename: Some("model.gguf".to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    };
    assert!(invalid_hf_config.validate().is_err());
}

#[tokio::test]
async fn test_model_source_validation() {
    let temp_dir = TempDir::new().unwrap();

    // Create a test GGUF file for local testing
    let test_file = temp_dir.path().join("test.gguf");
    let test_content = b"test gguf content";
    create_test_gguf_file(&test_file, test_content)
        .await
        .unwrap();

    // Valid local source
    let valid_local = ModelSource::Local {
        folder: temp_dir.path().to_path_buf(),
        filename: Some("test.gguf".to_string()),
    };
    assert!(valid_local.validate().is_ok());

    // Invalid local source (file doesn't exist)
    let invalid_local = ModelSource::Local {
        folder: temp_dir.path().to_path_buf(),
        filename: Some("nonexistent.gguf".to_string()),
    };
    assert!(invalid_local.validate().is_err());

    // Valid HuggingFace source
    let valid_hf = ModelSource::HuggingFace {
        repo: "microsoft/DialoGPT-medium".to_string(),
        filename: Some("model.gguf".to_string()),
        folder: None,
    };
    assert!(valid_hf.validate().is_ok());

    // Invalid HuggingFace source (bad filename extension)
    let invalid_hf = ModelSource::HuggingFace {
        repo: "microsoft/DialoGPT-medium".to_string(),
        filename: Some("model.txt".to_string()), // Wrong extension
        folder: None,
    };
    assert!(invalid_hf.validate().is_err());
}

#[tokio::test]
async fn test_retry_config() {
    let config = RetryConfig::default();
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.initial_delay_ms, 1000);
    assert_eq!(config.backoff_multiplier, 2.0);
    assert_eq!(config.max_delay_ms, 30000);

    let custom_config = RetryConfig {
        max_retries: 5,
        initial_delay_ms: 500,
        backoff_multiplier: 1.5,
        max_delay_ms: 10000,
    };
    assert_eq!(custom_config.max_retries, 5);
    assert_eq!(custom_config.initial_delay_ms, 500);
}

// Note: Integration tests with actual LlamaBackend and model loading would require:
// 1. A real model file
// 2. Proper llama-cpp-2 backend initialization
// 3. These are better suited for end-to-end integration tests in a separate test suite
// that can handle larger test files and longer running tests
