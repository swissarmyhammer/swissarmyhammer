//! End-to-end integration test for LlamaAgent + MCP tools
//!
//! This test validates that a Llama model can successfully use MCP tools through
//! the in-process HTTP MCP server to read the Cargo.toml file. It proves the
//! complete integration: local model → HTTP MCP server → MCP tool → file system.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
use swissarmyhammer_config::{DEFAULT_TEST_LLM_MODEL_FILENAME, DEFAULT_TEST_LLM_MODEL_REPO};
// Removed: use swissarmyhammer_tools::mcp::unified_server - not needed as LlamaAgent starts its own MCP server
use swissarmyhammer_workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;
use tokio::time::timeout;
use tracing::info;

// Test timeout constants
const INTEGRATION_TEST_TIMEOUT_SECS: u64 = 300; // 5 minutes for complete integration test
const MODEL_EXECUTION_TIMEOUT_SECS: u64 = 180; // 3 minutes for model execution

// Test prompt template
const FILE_READ_PROMPT: &str = "read the cargo.toml file using the file_read tool";
const SYSTEM_PROMPT: &str = "You are a helpful assistant that can use tools to read files.";

/// Creates LlamaAgent configuration with its own MCP server
fn create_llama_config_for_integration_test() -> AgentConfig {
    info!("Configuring LlamaAgent with its own MCP server for integration testing");

    let mut llama_config = LlamaAgentConfig::for_testing();
    // Use port 0 for dynamic allocation - LlamaAgent will start its own MCP server
    llama_config.mcp_server.port = 0;

    AgentConfig::llama_agent(llama_config)
}

/// Validates that response contains expected Cargo.toml content
fn validate_cargo_toml_response(response: &str) -> Result<(), String> {
    let response_lower = response.to_lowercase();
    println!("Validating response content:\n{}", response);

    // Check for key Cargo.toml sections
    if !response_lower.contains("[package]") && !response_lower.contains("package") {
        return Err(format!(
            "Response should contain [package] section or reference to package. Got: {}",
            response
        ));
    }

    if !response_lower.contains("swissarmyhammer") && !response_lower.contains("name") {
        return Err(format!(
            "Response should contain project name 'swissarmyhammer' or name field. Got: {}",
            response
        ));
    }

    // Look for dependency-related content
    let has_dependencies = response_lower.contains("dependencies")
        || response_lower.contains("tokio")
        || response_lower.contains("serde")
        || response_lower.contains("clap");

    if !has_dependencies {
        return Err(format!(
            "Response should contain dependency declarations or common dependencies. Got: {}",
            response
        ));
    }

    Ok(())
}

/// End-to-end integration test validating LlamaAgent can use its own MCP tools to read Cargo.toml
///
/// This test proves the complete integration workflow:
/// 1. Configures LlamaAgent to start its own HTTP MCP server with SwissArmyHammer tools
/// 2. Executes prompt asking model to read Cargo.toml using file_read tool
/// 3. Validates model makes correct MCP tool call to its own server and receives file contents
/// 4. Verifies response contains actual Cargo.toml content
#[tokio::test]
async fn test_llama_mcp_cargo_toml_integration() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    let test_timeout = Duration::from_secs(INTEGRATION_TEST_TIMEOUT_SECS);

    let test_result = timeout(test_timeout, async {
        // Create LlamaAgent configuration with its own MCP server
        let agent_config = create_llama_config_for_integration_test();

        // Create workflow context with agent configuration
        let context = WorkflowTemplateContext::with_vars(HashMap::new())?;
        let mut context_with_config = context;
        context_with_config.set_agent_config(agent_config);
        let execution_context = AgentExecutionContext::new(&context_with_config);

        info!("Creating LlamaAgent executor with integrated MCP server");
        let mut executor = AgentExecutorFactory::create_executor(&execution_context).await?;

        // Validate Cargo.toml exists at expected location
        let cargo_toml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("Failed to get parent directory")?
            .join("Cargo.toml");

        if !cargo_toml_path.exists() {
            return Err(format!(
                "Cargo.toml not found at expected path: {}",
                cargo_toml_path.display()
            )
            .into());
        }

        info!(
            "Validated Cargo.toml exists at: {}",
            cargo_toml_path.display()
        );

        // Execute prompt asking model to read Cargo.toml using integrated MCP tools
        info!("Executing prompt: '{}'", FILE_READ_PROMPT);
        info!("Expected workflow: LlamaAgent → Internal MCP Server → file_read tool → file system");

        let model_timeout = Duration::from_secs(MODEL_EXECUTION_TIMEOUT_SECS);

        let agent_response = executor
            .execute_prompt(
                SYSTEM_PROMPT.to_string(),
                FILE_READ_PROMPT.to_string(),
                &execution_context,
                model_timeout,
            )
            .await
            .map_err(|e| format!("Failed to execute prompt with LlamaAgent: {}", e))?;

        let response = &agent_response.content;
        info!("Model response received (length: {} chars)", response.len());

        // Validate the complete round-trip worked
        info!("Validating response contains Cargo.toml content");
        validate_cargo_toml_response(response)?;

        info!("Integration test validation successful");
        info!("Complete workflow verified: LlamaAgent ↔ Internal MCP Server ↔ file_read tool");

        // Properly shutdown the executor to clean up MCP server resources
        executor
            .shutdown()
            .await
            .map_err(|e| format!("Failed to shutdown executor: {}", e))?;
        info!("LlamaAgent executor shutdown successfully");

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await;

    match test_result {
        Ok(Ok(())) => {
            info!("End-to-end integration test completed successfully");
        }
        Ok(Err(e)) => {
            panic!("Integration test failed: {}", e);
        }
        Err(_) => {
            panic!(
                "Integration test timed out after {} seconds",
                test_timeout.as_secs()
            );
        }
    }
}

/// Tests LlamaAgent MCP server integration by validating configuration
#[tokio::test]
async fn test_llama_mcp_server_connectivity() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    info!("Testing LlamaAgent MCP server integration configuration");

    // Create LlamaAgent configuration - this will start its own integrated MCP server
    let agent_config = create_llama_config_for_integration_test();

    // Create workflow context with agent configuration
    let context =
        WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
    let mut context_with_config = context;
    context_with_config.set_agent_config(agent_config);
    let execution_context = AgentExecutionContext::new(&context_with_config);

    info!("Creating LlamaAgent executor which will start integrated MCP server");
    let mut executor = AgentExecutorFactory::create_executor(&execution_context)
        .await
        .expect("Failed to create LlamaAgent executor");

    info!("LlamaAgent MCP server integration test completed successfully");
    info!("LlamaAgent successfully created with integrated MCP server capability");

    // Properly shutdown the executor to clean up MCP server resources
    executor
        .shutdown()
        .await
        .expect("Failed to shutdown executor");
    info!("LlamaAgent executor shutdown successfully");
}

/// Tests LlamaAgent configuration with MCP server settings
#[tokio::test]
async fn test_llama_agent_config_with_mcp() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    info!("Testing LlamaAgent configuration with integrated MCP server settings");

    // Test that LlamaAgent configuration includes proper MCP server setup
    let llama_config = LlamaAgentConfig::for_testing();

    // Verify model configuration uses test constants
    match &llama_config.model.source {
        swissarmyhammer_config::agent::ModelSource::HuggingFace { repo, filename, .. } => {
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

    // Verify MCP server configuration is suitable for testing
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

    // Test agent configuration creation
    let mut agent_config = AgentConfig::llama_agent(llama_config);
    agent_config.quiet = true; // Set quiet mode for tests
    assert!(agent_config.quiet, "Test configuration should be quiet");

    match &agent_config.executor {
        swissarmyhammer_config::agent::AgentExecutorConfig::LlamaAgent(_llama_exec_config) => {
            info!("Agent executor configuration validated");
            info!("Agent type: LlamaAgent with integrated MCP server");
            info!("Quiet mode: {}", agent_config.quiet);
        }
        _ => panic!("Expected LlamaAgent executor configuration"),
    }

    info!("LlamaAgent integrated MCP configuration test completed successfully");
}
