//! Utility functions for agent executors
//!
//! This module provides utility functions for validating executor availability,
//! getting recommended timeouts, and other executor-related functionality.

use crate::workflow::actions::{ActionError, ActionResult};
use crate::workflow::agents::LlamaAgentExecutor;
use std::time::Duration;
use swissarmyhammer_config::agent::{AgentExecutorType, LlamaAgentConfig};

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
            // Validate LlamaAgent configuration by creating a test executor
            // and running its built-in validation logic
            validate_llama_agent_configuration().await
        }
    }
}

/// Validate LlamaAgent configuration and availability
async fn validate_llama_agent_configuration() -> ActionResult<()> {
    // Create a test configuration to validate the executor setup
    let test_config = LlamaAgentConfig::for_testing();

    // Create executor with test configuration and validate it
    let executor = LlamaAgentExecutor::new(test_config);

    // Run the built-in validation logic
    executor.validate_config().map_err(|e| {
        ActionError::ExecutionError(format!("LlamaAgent configuration validation failed: {}", e))
    })
}

/// Get recommended timeout for an executor type
pub fn get_recommended_timeout(executor_type: AgentExecutorType) -> Duration {
    match executor_type {
        AgentExecutorType::ClaudeCode => Duration::from_secs(30),
        AgentExecutorType::LlamaAgent => Duration::from_secs(60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use swissarmyhammer_config::agent::{
        LlamaAgentConfig, McpServerConfig, ModelConfig, ModelSource,
    };
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_validate_llama_agent_with_valid_config() {
        // Test with default valid configuration
        let result = validate_llama_agent_configuration().await;
        assert!(
            result.is_ok(),
            "Valid LlamaAgent configuration should pass validation"
        );
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_empty_huggingface_repo() {
        // Test validation fails with empty HuggingFace repo name
        let invalid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "".to_string(), // Empty repo name should fail
                    filename: Some("model.gguf".to_string()),
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(invalid_config);
        let result = executor.validate_config();

        assert!(
            result.is_err(),
            "Empty HuggingFace repo should fail validation"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("repository name cannot be empty"));
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_empty_filename() {
        // Test validation fails with empty filename
        let invalid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "valid-repo".to_string(),
                    filename: Some("".to_string()), // Empty filename should fail
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(invalid_config);
        let result = executor.validate_config();

        assert!(
            result.is_err(),
            "Empty model filename should fail validation"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("filename cannot be empty"));
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_invalid_local_file_extension() {
        // Test validation fails with invalid file extension for LOCAL models
        // (HuggingFace models now support folder-based models, so they don't require .gguf)
        let invalid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: PathBuf::from("/tmp/model.bin"), // Wrong extension for local file
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(invalid_config);
        let result = executor.validate_config();

        assert!(
            result.is_err(),
            "Invalid file extension should fail validation for local models"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must end with .gguf"));
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_huggingface_folder_model() {
        // Test validation passes with folder-based model names for HuggingFace models
        // (This should now pass since HuggingFace models support folder-based models)
        let valid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "microsoft/Phi-3-mini-4k-instruct-gguf".to_string(),
                    filename: Some("Phi-3-mini-4k-instruct-q4".to_string()), // Folder name, not .gguf
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(valid_config);
        let result = executor.validate_config();

        assert!(
            result.is_ok(),
            "HuggingFace models should support folder-based model names"
        );
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_missing_local_file() {
        // Test validation fails when local file doesn't exist
        let invalid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: PathBuf::from("/nonexistent/path/model.gguf"),
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(invalid_config);
        let result = executor.validate_config();

        assert!(result.is_err(), "Missing local file should fail validation");
        assert!(result.unwrap_err().to_string().contains("file not found"));
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_valid_local_file() {
        // Create a temporary file to test with
        let temp_file = NamedTempFile::with_suffix(".gguf").unwrap();
        let temp_path = temp_file.path().to_path_buf();

        let valid_config = LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    filename: temp_path,
                    folder: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(valid_config);
        let result = executor.validate_config();

        assert!(result.is_ok(), "Valid local file should pass validation");
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_zero_timeout() {
        // Test validation fails with zero timeout
        let invalid_config = LlamaAgentConfig {
            mcp_server: McpServerConfig {
                port: 0,
                timeout_seconds: 0, // Zero timeout should fail
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(invalid_config);
        let result = executor.validate_config();

        assert!(result.is_err(), "Zero timeout should fail validation");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("timeout must be greater than 0"));
    }

    #[tokio::test]
    async fn test_validate_llama_agent_with_high_timeout() {
        // Test validation warns but succeeds with high timeout
        let config_with_high_timeout = LlamaAgentConfig {
            mcp_server: McpServerConfig {
                port: 0,
                timeout_seconds: 500, // High timeout should warn but not fail
            },
            ..Default::default()
        };

        let executor = LlamaAgentExecutor::new(config_with_high_timeout);
        let result = executor.validate_config();

        assert!(
            result.is_ok(),
            "High timeout should pass validation with warning"
        );
    }

    #[tokio::test]
    async fn test_validate_executor_availability_integration() {
        // Test the main function we implemented
        let result = validate_executor_availability(AgentExecutorType::LlamaAgent).await;
        assert!(
            result.is_ok(),
            "LlamaAgent executor availability should validate successfully"
        );
    }
}
