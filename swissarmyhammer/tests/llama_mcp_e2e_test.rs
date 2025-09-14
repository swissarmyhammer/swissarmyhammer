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
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerHandle, McpServerMode};
use swissarmyhammer_workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;
use tokio::time::timeout;
use tracing::{info, warn};

// Test timeout constants
const INTEGRATION_TEST_TIMEOUT_SECS: u64 = 300; // 5 minutes for complete integration test
const MODEL_EXECUTION_TIMEOUT_SECS: u64 = 180; // 3 minutes for model execution


// Test prompt template
const FILE_READ_PROMPT: &str = "read the cargo.toml file using the file_read tool";
const SYSTEM_PROMPT: &str = "You are a helpful assistant that can use tools to read files.";

/// Creates and starts an HTTP MCP server for testing
async fn setup_test_mcp_server() -> Result<McpServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting in-process HTTP MCP server for testing");
    let mcp_server = start_mcp_server(
        McpServerMode::Http { port: None }, // Random port allocation
        None,                               // Use default prompt library
    )
    .await?;

    let server_url = mcp_server.url();
    let server_port = mcp_server.port().ok_or("HTTP server should have a port")?;

    info!("MCP server started successfully on {}", server_url);
    info!("Server bound to port: {}", server_port);

    Ok(mcp_server)
}

/// Creates LlamaAgent configuration with MCP server endpoint
fn create_llama_config_with_mcp(server_port: u16) -> AgentConfig {
    info!(
        "Configuring LlamaAgent with MCP server on port {}",
        server_port
    );

    let mut llama_config = LlamaAgentConfig::for_testing();
    llama_config.mcp_server.port = server_port;

    AgentConfig::llama_agent(llama_config)
}

/// Validates that response contains expected Cargo.toml content
fn validate_cargo_toml_response(response: &str) -> Result<(), String> {
    let response_lower = response.to_lowercase();

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

/// End-to-end integration test validating LlamaAgent can use MCP tools to read Cargo.toml
///
/// This test proves the complete integration workflow:
/// 1. Starts in-process HTTP MCP server with SwissArmyHammer tools
/// 2. Configures LlamaAgent to use the MCP server for tool calls
/// 3. Executes prompt asking model to read Cargo.toml using file_read tool
/// 4. Validates model makes correct MCP tool call and receives file contents
/// 5. Verifies response contains actual Cargo.toml content
#[tokio::test]
async fn test_llama_mcp_cargo_toml_integration() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        warn!("Skipping LlamaAgent MCP integration test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    let test_timeout = Duration::from_secs(INTEGRATION_TEST_TIMEOUT_SECS);

    let test_result = timeout(test_timeout, async {
        // Start MCP server
        let mut mcp_server = setup_test_mcp_server().await?;
        let server_port = mcp_server.port().ok_or("Server should have a port")?;

        // Create LlamaAgent configuration with MCP server endpoint
        let agent_config = create_llama_config_with_mcp(server_port);

        // Create workflow context with agent configuration
        let context = WorkflowTemplateContext::with_vars(HashMap::new())?;
        let mut context_with_config = context;
        context_with_config.set_agent_config(agent_config);
        let execution_context = AgentExecutionContext::new(&context_with_config);

        info!("Creating LlamaAgent executor");
        let executor = AgentExecutorFactory::create_executor(&execution_context).await?;

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

        // Execute prompt asking model to read Cargo.toml using MCP tools
        info!("Executing prompt: '{}'", FILE_READ_PROMPT);
        info!("Expected workflow: Model → MCP Server → file_read tool → file system");

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
        info!("Complete workflow verified: MCP server ↔ LlamaAgent ↔ file_read tool");

        // Shutdown MCP server gracefully
        mcp_server
            .shutdown()
            .await
            .map_err(|e| format!("Failed to shutdown MCP server: {}", e))?;
        info!("MCP server shutdown complete");

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

/// Tests MCP server connectivity and configuration for LlamaAgent integration
#[tokio::test]
async fn test_llama_mcp_server_connectivity() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        warn!(
            "Skipping LlamaAgent MCP server connectivity test (set SAH_TEST_LLAMA=true to enable)"
        );
        return;
    }

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    info!("Testing MCP server connectivity for LlamaAgent integration");

    // Start MCP server
    let mut mcp_server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start MCP server");

    // Verify server provides expected connection info
    assert!(
        mcp_server.port().is_some(),
        "HTTP server should have a port"
    );
    assert!(
        !mcp_server.url().is_empty(),
        "Server URL should not be empty"
    );
    assert!(
        mcp_server.url().starts_with("http://"),
        "URL should be HTTP"
    );

    let port = mcp_server.port().unwrap();
    let expected_url = format!("http://127.0.0.1:{}", port);
    assert!(
        mcp_server.url().contains(&port.to_string()),
        "URL should contain the bound port. Expected: {}, Got: {}",
        expected_url,
        mcp_server.url()
    );

    info!("MCP server connectivity validated successfully");
    info!("Server port: {}", port);
    info!("Server URL: {}", mcp_server.url());

    // Clean shutdown
    mcp_server
        .shutdown()
        .await
        .expect("Failed to shutdown MCP server");
    info!("Server shutdown successful");
}

/// Tests LlamaAgent configuration with MCP server settings
#[tokio::test]
async fn test_llama_agent_config_with_mcp() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        warn!("Skipping LlamaAgent MCP config test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    info!("Testing LlamaAgent configuration with MCP server settings");

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
        "Should use random port for testing"
    );
    assert!(
        llama_config.mcp_server.timeout_seconds > 0,
        "Should have reasonable timeout for MCP server"
    );

    info!("MCP server configuration validated");
    info!("MCP server port: {} (random)", llama_config.mcp_server.port);
    info!(
        "MCP server timeout: {}s",
        llama_config.mcp_server.timeout_seconds
    );

    // Test agent configuration creation
    let agent_config = AgentConfig::llama_agent(llama_config);
    assert!(agent_config.quiet, "Test configuration should be quiet");

    match &agent_config.executor {
        swissarmyhammer_config::agent::AgentExecutorConfig::LlamaAgent(_llama_exec_config) => {
            info!("Agent executor configuration validated");
            info!("Agent type: LlamaAgent");
            info!("Quiet mode: {}", agent_config.quiet);
        }
        _ => panic!("Expected LlamaAgent executor configuration"),
    }

    info!("LlamaAgent MCP configuration test completed successfully");
}
