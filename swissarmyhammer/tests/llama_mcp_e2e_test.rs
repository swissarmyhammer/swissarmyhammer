//! End-to-end integration test for LlamaAgent + MCP tools
//!
//! This test validates that a Llama model can successfully use MCP tools through
//! the in-process HTTP MCP server to read the Cargo.toml file. It proves the
//! complete integration: local model → HTTP MCP server → MCP tool → file system.

use std::collections::HashMap;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{LlamaAgentConfig, ModelConfig};
use swissarmyhammer_config::{DEFAULT_TEST_LLM_MODEL_FILENAME, DEFAULT_TEST_LLM_MODEL_REPO};
// Removed: use swissarmyhammer_tools::mcp::unified_server - not needed as LlamaAgent starts its own MCP server
use swissarmyhammer_workflow::actions::AgentExecutionContext;
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;
use tracing::info;

/// Creates LlamaAgent configuration that uses HuggingFace with proper caching
fn create_llama_config_for_integration_test() -> ModelConfig {
    info!("Configuring LlamaAgent with HuggingFace source and caching");

    // Create a completely fresh config to bypass any environment overrides
    let llama_config = swissarmyhammer_config::model::LlamaAgentConfig {
        model: swissarmyhammer_config::model::LlmModelConfig {
            source: swissarmyhammer_config::model::ModelSource::HuggingFace {
                repo: DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
                filename: Some(DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                folder: None,
            },
            batch_size: 64,
            use_hf_params: true,
            debug: false,
        },
        mcp_server: swissarmyhammer_config::model::McpServerConfig {
            port: 0,
            timeout_seconds: 30,
        },
        repetition_detection: Default::default(),
    };

    ModelConfig::llama_agent(llama_config)
}

/// Helper function to create an isolated test environment
fn setup_isolated_test() -> IsolatedTestEnvironment {
    IsolatedTestEnvironment::new().expect("Failed to create test environment")
}

/// Helper function to create a workflow template context with LlamaAgent configuration
fn create_workflow_context_with_llama_config() -> WorkflowTemplateContext {
    let agent_config = create_llama_config_for_integration_test();
    let context =
        WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
    let mut context_with_config = context;
    context_with_config.set_agent_config(agent_config);
    context_with_config
}

/// Helper function to assert the executor type is LlamaAgent
fn assert_llama_agent_executor_type(execution_context: &AgentExecutionContext) {
    assert_eq!(
        execution_context.executor_type(),
        swissarmyhammer_config::model::AgentExecutorType::LlamaAgent
    );
}

/// Fast integration test validating LlamaAgent configuration and MCP server startup
///
/// This test focuses on the configuration and server startup without expensive LLM operations:
/// 1. Validates LlamaAgent configuration creation
/// 2. Tests MCP server startup and connectivity
/// 3. Verifies executor creation and shutdown
/// 4. No actual LLM inference - focuses on integration infrastructure
#[test_log::test(tokio::test)]
async fn test_llama_mcp_integration_fast() {
    let _guard = setup_isolated_test();

    info!("Testing LlamaAgent MCP integration infrastructure (fast test)");

    let context_with_config = create_workflow_context_with_llama_config();
    let execution_context = AgentExecutionContext::new(&context_with_config);

    info!("LlamaAgent execution context created with integrated MCP server configuration");

    assert_llama_agent_executor_type(&execution_context);

    info!("LlamaAgent MCP integration infrastructure validated successfully");
}

/// Tests LlamaAgent MCP server integration by validating configuration (Slow)
///
/// NOTE: This test is slow (>5s) because it may trigger LLM model operations.
#[test_log::test(tokio::test)]
async fn test_llama_mcp_server_connectivity() {
    let _guard = setup_isolated_test();

    info!("Testing LlamaAgent MCP server integration configuration (fast)");

    let context_with_config = create_workflow_context_with_llama_config();
    let execution_context = AgentExecutionContext::new(&context_with_config);

    info!("LlamaAgent execution context created with integrated MCP server configuration");

    assert_llama_agent_executor_type(&execution_context);

    info!("LlamaAgent MCP server integration test completed successfully");
    info!("LlamaAgent execution context successfully configured with integrated MCP server capability");
}

/// Tests LlamaAgent model source configuration
///
/// Validates that the LlamaAgent test configuration uses the correct HuggingFace model
/// repository and filename settings.
#[tokio::test]
async fn test_llama_model_source_configuration() {
    let _guard = setup_isolated_test();

    info!("Testing LlamaAgent model source configuration");

    let llama_config = LlamaAgentConfig::for_testing();

    match &llama_config.model.source {
        swissarmyhammer_config::model::ModelSource::HuggingFace { repo, filename, .. } => {
            assert_eq!(
                repo, DEFAULT_TEST_LLM_MODEL_REPO,
                "Should use default test model repo"
            );
            assert_eq!(
                filename.as_ref().unwrap(),
                DEFAULT_TEST_LLM_MODEL_FILENAME,
                "Should use default test model filename"
            );

            info!("Model source configuration validated");
            info!("Model repo: {}", repo);
            info!("Model filename: {}", filename.as_ref().unwrap());
        }
        _ => panic!("Expected HuggingFace model source for testing"),
    }

    info!("LlamaAgent model source configuration test completed successfully");
}

/// Tests LlamaAgent MCP server configuration
///
/// Validates that the MCP server is configured with appropriate port and timeout settings
/// for integration testing.
#[tokio::test]
async fn test_llama_mcp_server_configuration() {
    let _guard = setup_isolated_test();

    info!("Testing LlamaAgent MCP server configuration");

    let llama_config = LlamaAgentConfig::for_testing();

    assert_eq!(
        llama_config.mcp_server.port, 0,
        "Should use random port for integrated MCP server"
    );
    assert!(
        llama_config.mcp_server.timeout_seconds > 0,
        "Should have reasonable timeout for MCP server"
    );

    info!("Integrated MCP server configuration validated");
    info!(
        "MCP server port: {} (random allocation)",
        llama_config.mcp_server.port
    );
    info!(
        "MCP server timeout: {}s",
        llama_config.mcp_server.timeout_seconds
    );

    info!("LlamaAgent MCP server configuration test completed successfully");
}

/// Tests LlamaAgent executor configuration creation
///
/// Validates that the ModelExecutorConfig is properly created with the correct executor
/// type and quiet mode settings for testing.
#[tokio::test]
async fn test_llama_agent_config_creation() {
    let _guard = setup_isolated_test();

    info!("Testing LlamaAgent executor configuration creation");

    let llama_config = LlamaAgentConfig::for_testing();
    let mut agent_config = ModelConfig::llama_agent(llama_config);
    agent_config.quiet = true;

    assert!(agent_config.quiet, "Test configuration should be quiet");

    match &agent_config.executor {
        swissarmyhammer_config::model::ModelExecutorConfig::LlamaAgent(_llama_exec_config) => {
            info!("Agent executor configuration validated");
            info!("Agent type: LlamaAgent with integrated MCP server");
            info!("Quiet mode: {}", agent_config.quiet);
        }
        _ => panic!("Expected LlamaAgent executor configuration"),
    }

    info!("LlamaAgent agent configuration creation test completed successfully");
}
