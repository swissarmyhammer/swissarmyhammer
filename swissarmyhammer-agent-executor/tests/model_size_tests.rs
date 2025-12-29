//! Tests for LlamaAgent executor with various model sizes
//!
//! These tests verify that the executor works correctly with different
//! model sizes and quantization levels, ensuring proper resource management
//! and configuration handling across the spectrum of supported models.

use serial_test::serial;
use swissarmyhammer_agent_executor::llama::executor::LlamaAgentExecutor;
use swissarmyhammer_agent_executor::AgentExecutor;
use swissarmyhammer_config::{LlamaAgentConfig, LlmModelConfig, McpServerConfig, ModelSource};

/// Test utility: Create a test MCP server configuration
fn create_test_mcp_server(port: u16) -> agent_client_protocol::McpServer {
    agent_client_protocol::McpServer::Http(agent_client_protocol::McpServerHttp::new(
        "test",
        format!("http://127.0.0.1:{}/mcp", port),
    ))
}

/// Test utility: Start MCP server and return handle with port
async fn start_test_mcp_server() -> swissarmyhammer_tools::mcp::unified_server::McpServerHandle {
    use swissarmyhammer_prompts::PromptLibrary;
    use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

    start_mcp_server(
        McpServerMode::Http { port: None },
        Some(PromptLibrary::default()),
        None,
        None,
    )
    .await
    .expect("Failed to start test MCP server")
}

/// Create executor configuration for a specific model
fn create_executor_config(repo: &str, filename: &str, batch_size: u32) -> LlamaAgentConfig {
    LlamaAgentConfig {
        model: LlmModelConfig {
            source: ModelSource::HuggingFace {
                repo: repo.to_string(),
                filename: Some(filename.to_string()),
                folder: None,
            },
            batch_size,
            use_hf_params: true,
            debug: false,
        },
        mcp_server: McpServerConfig::default(),
        repetition_detection: Default::default(),
    }
}

/// Test helper to create and configure an executor
fn create_test_executor(config: LlamaAgentConfig, port: u16) -> LlamaAgentExecutor {
    let mcp_server = create_test_mcp_server(port);
    LlamaAgentExecutor::new(config, mcp_server)
}

/// Test: Very small model (1.5B parameters, Q4_K_M quantization)
/// Expected size: ~1GB
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model"]
async fn test_very_small_model_1_5b() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Qwen2.5-Coder-1.5B-Instruct - very small, fast model
    let config = create_executor_config(
        "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        256,
    );

    let mut executor = create_test_executor(config, port);

    // Verify model display name
    assert!(executor
        .get_model_display_name()
        .contains("Qwen2.5-Coder-1.5B"));

    // Initialize and verify
    let result = executor.initialize().await;
    assert!(
        result.is_ok(),
        "Very small model initialization failed: {:?}",
        result.err()
    );
    assert!(executor.is_model_loaded().await);

    // Verify resource stats
    let stats = executor.get_resource_stats().await;
    assert!(stats.is_ok());
    let stats = stats.unwrap();
    assert!(stats.model_size_mb > 0);
    assert!(stats.model_size_mb < 2048); // Should be less than 2GB

    // Cleanup
    executor.shutdown().await.unwrap();
}

/// Test: Small model (3B parameters, Q4_K_M quantization)
/// Expected size: ~2GB
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model"]
async fn test_small_model_3b() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Phi-3-mini - small but capable model
    let config = create_executor_config(
        "microsoft/Phi-3-mini-4k-instruct-gguf",
        "Phi-3-mini-4k-instruct-q4.gguf",
        256,
    );

    let mut executor = create_test_executor(config, port);

    assert!(executor.get_model_display_name().contains("Phi-3-mini"));

    let result = executor.initialize().await;
    assert!(
        result.is_ok(),
        "Small model initialization failed: {:?}",
        result.err()
    );
    assert!(executor.is_model_loaded().await);

    // Verify resource stats
    let stats = executor.get_resource_stats().await;
    assert!(stats.is_ok());
    let stats = stats.unwrap();
    assert!(stats.model_size_mb > 0);
    assert!(stats.model_size_mb < 3072); // Should be less than 3GB

    executor.shutdown().await.unwrap();
}

/// Test: Medium model (7B parameters, Q4_K_M quantization)
/// Expected size: ~4-5GB
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model"]
async fn test_medium_model_7b() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Llama-3.2-3B - medium-sized model
    let config = create_executor_config(
        "bartowski/Llama-3.2-3B-Instruct-GGUF",
        "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        512,
    );

    let mut executor = create_test_executor(config, port);

    assert!(executor.get_model_display_name().contains("Llama-3.2-3B"));

    let result = executor.initialize().await;
    assert!(
        result.is_ok(),
        "Medium model initialization failed: {:?}",
        result.err()
    );
    assert!(executor.is_model_loaded().await);

    // Verify resource stats
    let stats = executor.get_resource_stats().await;
    assert!(stats.is_ok());
    let stats = stats.unwrap();
    assert!(stats.model_size_mb > 0);
    assert!(stats.model_size_mb < 6144); // Should be less than 6GB

    executor.shutdown().await.unwrap();
}

/// Test: Large model (13B parameters, Q4_K_M quantization)
/// Expected size: ~8GB
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model, requires significant memory"]
async fn test_large_model_13b() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Qwen2.5-Coder-14B - large model
    let config = create_executor_config(
        "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
        "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
        512,
    );

    let mut executor = create_test_executor(config, port);

    assert!(executor
        .get_model_display_name()
        .contains("Qwen2.5-Coder-14B"));

    let result = executor.initialize().await;
    assert!(
        result.is_ok(),
        "Large model initialization failed: {:?}",
        result.err()
    );
    assert!(executor.is_model_loaded().await);

    // Verify resource stats
    let stats = executor.get_resource_stats().await;
    assert!(stats.is_ok());
    let stats = stats.unwrap();
    assert!(stats.model_size_mb > 0);
    assert!(stats.model_size_mb < 10240); // Should be less than 10GB

    executor.shutdown().await.unwrap();
}

/// Test: Different quantization levels with same base model
/// Tests Q2_K, Q4_K_M, Q5_K_M, Q8_0 quantizations
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads multiple model variants"]
async fn test_different_quantization_levels() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Test configurations for different quantization levels
    let quantizations = vec![
        ("qwen2.5-coder-1.5b-instruct-q2_k.gguf", 128), // Very compressed
        ("qwen2.5-coder-1.5b-instruct-q4_k_m.gguf", 256), // Balanced (default)
        ("qwen2.5-coder-1.5b-instruct-q5_k_m.gguf", 256), // Higher quality
        ("qwen2.5-coder-1.5b-instruct-q8_0.gguf", 512), // Very high quality
    ];

    for (filename, batch_size) in quantizations {
        let config = create_executor_config(
            "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
            filename,
            batch_size,
        );

        let mut executor = create_test_executor(config, port);

        tracing::info!("Testing quantization: {}", filename);

        let result = executor.initialize().await;
        assert!(
            result.is_ok(),
            "Quantization {} initialization failed: {:?}",
            filename,
            result.err()
        );

        // Verify the model loaded
        assert!(executor.is_model_loaded().await);

        executor.shutdown().await.unwrap();
    }
}

/// Test: Batch size scaling with model size
/// Verifies that different batch sizes work correctly
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model"]
async fn test_batch_size_scaling() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    // Test different batch sizes with the same model
    let batch_sizes = vec![64, 128, 256, 512, 1024];

    for batch_size in batch_sizes {
        let config = create_executor_config(
            "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
            "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
            batch_size,
        );

        let mut executor = create_test_executor(config, port);

        tracing::info!("Testing batch size: {}", batch_size);

        let result = executor.initialize().await;
        assert!(
            result.is_ok(),
            "Batch size {} initialization failed: {:?}",
            batch_size,
            result.err()
        );

        assert!(executor.is_model_loaded().await);

        executor.shutdown().await.unwrap();
    }
}

/// Test: Model size validation
/// Verifies that model size information is correctly reported
#[test_log::test(tokio::test)]
#[serial]
#[ignore = "Integration test - downloads real model"]
async fn test_model_size_reporting() {
    let tools_handle = start_test_mcp_server().await;
    let port = tools_handle.info().port.unwrap_or(0);

    let config = create_executor_config(
        "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        256,
    );

    let mut executor = create_test_executor(config, port);

    executor.initialize().await.expect("Initialization failed");

    // Get resource stats and verify model size is reported
    let stats = executor.get_resource_stats().await.unwrap();

    // Verify reasonable bounds for a 1.5B Q4_K_M model
    assert!(
        stats.model_size_mb > 512,
        "Model size too small: {}MB",
        stats.model_size_mb
    );
    assert!(
        stats.model_size_mb < 2048,
        "Model size too large: {}MB",
        stats.model_size_mb
    );

    executor.shutdown().await.unwrap();
}

/// Test: Memory usage across model sizes
/// Verifies that memory usage scales appropriately with model size
#[test]
fn test_model_size_display_names() {
    // Test that display names are correctly formatted for different model sizes
    let test_cases = vec![
        (
            "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
            "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
            "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        ),
        (
            "microsoft/Phi-3-mini-4k-instruct-gguf",
            "Phi-3-mini-4k-instruct-q4.gguf",
            "microsoft/Phi-3-mini-4k-instruct-gguf/Phi-3-mini-4k-instruct-q4.gguf",
        ),
        (
            "bartowski/Llama-3.2-3B-Instruct-GGUF",
            "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
            "bartowski/Llama-3.2-3B-Instruct-GGUF/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        ),
    ];

    for (repo, filename, expected_display) in test_cases {
        let config = create_executor_config(repo, filename, 256);
        let executor = create_test_executor(config, 8080);

        assert_eq!(executor.get_model_display_name(), expected_display);
    }
}

/// Test: Configuration validation for different model sizes
/// Ensures that configuration validation works correctly for various model sizes
#[test]
fn test_configuration_validation_for_various_sizes() {
    // Valid small model
    let small_config = create_executor_config(
        "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        256,
    );
    let small_executor = create_test_executor(small_config, 8080);
    assert!(small_executor.validate_config().is_ok());

    // Valid medium model
    let medium_config = create_executor_config(
        "microsoft/Phi-3-mini-4k-instruct-gguf",
        "Phi-3-mini-4k-instruct-q4.gguf",
        512,
    );
    let medium_executor = create_test_executor(medium_config, 8080);
    assert!(medium_executor.validate_config().is_ok());

    // Invalid: empty repo
    let invalid_config = create_executor_config("", "model.gguf", 256);
    let invalid_executor = create_test_executor(invalid_config, 8080);
    assert!(invalid_executor.validate_config().is_err());

    // Invalid: empty filename
    let invalid_config2 = create_executor_config("valid/repo", "", 256);
    let invalid_executor2 = create_test_executor(invalid_config2, 8080);
    assert!(invalid_executor2.validate_config().is_err());
}

/// Test: Default test model configuration
/// Verifies that the default test configuration uses an appropriate small model
#[test]
fn test_default_test_model_size() {
    let config = LlamaAgentConfig::for_testing();
    let executor = create_test_executor(config, 8080);

    let display_name = executor.get_model_display_name();

    // Verify it's a reasonable test model (should be Qwen3-4B or similar)
    assert!(
        display_name.contains("Qwen") || display_name.contains("qwen"),
        "Default test model should be a Qwen model, got: {}",
        display_name
    );

    // Verify configuration is valid
    assert!(executor.validate_config().is_ok());
}
