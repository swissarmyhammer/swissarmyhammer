//! Utility functions for agent executors
//!
//! This module provides utility functions for validating executor availability,
//! getting recommended timeouts, and other executor-related functionality.

use crate::actions::{ActionError, ActionResult};
use swissarmyhammer_config::agent::AgentExecutorType;

/// Validate that a specific executor type is available
pub async fn validate_executor_availability(executor_type: AgentExecutorType) -> ActionResult<()> {
    match executor_type {
        AgentExecutorType::ClaudeCode => {
            // Check if Claude CLI is available
            match tokio::process::Command::new("claude")
                .arg("--version")
                .output()
                .await
            {
                Ok(output) if output.status.success() => Ok(()),
                _ => Err(ActionError::ExecutionError(
                    "Claude CLI not found or not working".to_string(),
                )),
            }
        }
        AgentExecutorType::LlamaAgent => {
            // Validate LlamaAgent configuration and availability
            validate_llama_agent_configuration().await
        }
    }
}

/// Validate LlamaAgent configuration and availability
async fn validate_llama_agent_configuration() -> ActionResult<()> {
    // Create a test configuration to validate the executor setup
    let test_config = swissarmyhammer_config::agent::LlamaAgentConfig::for_testing();

    // Create executor with test configuration and validate it
    let executor = swissarmyhammer_agent_executor::LlamaAgentExecutor::new(test_config);

    // Run the built-in validation logic
    executor.validate_config().map_err(|e| {
        ActionError::ExecutionError(format!("LlamaAgent configuration validation failed: {}", e))
    })
}
