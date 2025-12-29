//! Utility functions for agent executors
//!
//! This module provides utility functions for validating executor availability,
//! getting recommended timeouts, and other executor-related functionality.

use crate::actions::{ActionError, ActionResult};
use swissarmyhammer_config::model::ModelExecutorType;

/// Validate that a specific executor type is available
pub async fn validate_executor_availability(executor_type: ModelExecutorType) -> ActionResult<()> {
    match executor_type {
        ModelExecutorType::ClaudeCode => {
            // Check if Claude CLI binary exists in PATH by checking common locations
            // This is a basic check that doesn't execute the binary
            let path_var = std::env::var("PATH").unwrap_or_default();
            let claude_found = path_var
                .split(':')
                .any(|dir| std::path::Path::new(dir).join("claude").exists());
            
            if claude_found {
                Ok(())
            } else {
                Err(ActionError::ExecutionError(
                    "Claude CLI not found in PATH".to_string(),
                ))
            }
        }
        ModelExecutorType::LlamaAgent => {
            // Validate LlamaAgent configuration and availability
            validate_llama_agent_configuration().await
        }
    }
}

/// Validate LlamaAgent configuration and availability
async fn validate_llama_agent_configuration() -> ActionResult<()> {
    // Create a test configuration to validate the executor setup
    let test_config = swissarmyhammer_config::model::LlamaAgentConfig::for_testing();

    // Create executor with test configuration and validate it
    let executor = swissarmyhammer_agent_executor::LlamaAgentExecutor::new(test_config);

    // Run the built-in validation logic
    executor.validate_config().map_err(|e| {
        ActionError::ExecutionError(format!("LlamaAgent configuration validation failed: {}", e))
    })
}
