/// Example demonstrating the ModelLoader API
/// This example shows how to use ModelLoader with both HuggingFace and local models
use llama_loader::{ModelConfig, ModelSource, RetryConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Note: This is a compilation example only, not a functional example
    // since we would need a real llama-cpp-2 backend initialized

    println!("ModelLoader API Example");

    // Create model configurations
    let hf_config = ModelConfig {
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

    let local_config = ModelConfig {
        source: ModelSource::Local {
            folder: PathBuf::from("./models"),
            filename: Some("local-model.gguf".to_string()),
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
        use_hf_params: false,
        retry_config: RetryConfig::default(),
        debug: false,
    };

    println!("HuggingFace config: {:?}", hf_config);
    println!("Local config: {:?}", local_config);

    // Validate configurations
    hf_config.validate()?;
    println!("HuggingFace config is valid!");

    // Local config validation will fail because the directory doesn't exist
    match local_config.validate() {
        Ok(_) => println!("Local config is valid!"),
        Err(e) => println!("Local config validation failed as expected: {}", e),
    }

    // Cache is now handled automatically by hf-hub
    println!("Model loading will use hf-hub's internal caching");

    // Example of retry configuration
    let retry_config = RetryConfig {
        max_retries: 5,
        initial_delay_ms: 500,
        backoff_multiplier: 1.5,
        max_delay_ms: 15000,
    };

    println!("Custom retry config: {:?}", retry_config);

    println!("ModelLoader API example completed successfully!");
    Ok(())
}
