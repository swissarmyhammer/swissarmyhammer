//! Utility functions for agent executors
//!
//! This module provides utility functions for validating executor availability,
//! getting recommended timeouts, and other executor-related functionality.

#[cfg(test)]
use crate::workflow::actions::{ActionError, ActionResult};
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use swissarmyhammer_config::agent::AgentExecutorType;

/// Validate that a specific executor type is available
#[cfg(test)]
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
            // LlamaAgent validation - for now always succeed
            // TODO: Add actual validation when LlamaAgent integration is complete
            Ok(())
        }
    }
}

/// Get recommended timeout for an executor type
#[cfg(test)]
pub fn get_recommended_timeout(executor_type: AgentExecutorType) -> Duration {
    match executor_type {
        AgentExecutorType::ClaudeCode => Duration::from_secs(30),
        AgentExecutorType::LlamaAgent => Duration::from_secs(60),
    }
}
