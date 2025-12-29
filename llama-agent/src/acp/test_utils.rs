//! Test utilities for ACP testing
//!
//! This module provides documentation and helpers for creating mock ACP clients
//! for testing without requiring a full AgentServer with model files.
//!
//! # Creating Mock ACP Clients
//!
//! To test ACP functionality without a full server:
//!
//! 1. **Use in-memory test doubles**: Create simple structs that implement
//!    the necessary behavior for your test case
//! 2. **Focus on the specific methods under test**: Don't implement the full
//!    Agent trait unless necessary
//! 3. **Use the existing test patterns**: See the ACP integration tests for examples
//!
//! # Example Test Pattern
//!
//! ```rust,ignore
//! use agent_client_protocol::{InitializeRequest, ProtocolVersion};
//! use std::sync::Arc;
//!
//! #[tokio::test]
//! async fn test_acp_feature() {
//!     // Create the minimal server setup needed for your test
//!     let server = create_test_acp_server().await.unwrap();
//!     
//!     // Initialize
//!     let init_request = InitializeRequest::new(ProtocolVersion::V1);
//!     let init_response = server.initialize(init_request).await.unwrap();
//!     
//!     // Test specific functionality
//!     assert_eq!(init_response.protocol_version(), &ProtocolVersion::V1);
//! }
//! ```
//!
//! # Testing Without Models
//!
//! For tests that don't require actual model inference:
//!
//! - Use the `#[ignore]` attribute and add a comment explaining that the test
//!   requires a model file
//! - Create focused unit tests for individual components (translation, permissions, etc.)
//! - Mock the AgentServer responses at the boundary
//!
//! # Available Test Helpers
//!
//! The existing ACP tests provide helper functions for common test scenarios:
//!
//! - `create_test_server()` - Creates a minimal ACP server (requires model)
//! - Session management helpers
//! - Filesystem operation helpers
//! - Permission policy helpers

use std::sync::Arc;

/// Helper to create test ACP configuration
///
/// This creates a minimal ACP config for testing with permissive settings.
pub fn create_test_acp_config(temp_dir: &std::path::Path) -> crate::acp::config::AcpConfig {
    crate::acp::config::AcpConfig {
        filesystem: crate::acp::config::FilesystemSettings {
            allowed_paths: vec![temp_dir.to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 1024 * 1024, // 1MB
        },
        ..Default::default()
    }
}

/// Helper to create agent configuration for testing
///
/// This creates minimal agent configuration that can be used in tests.
/// Note: This still requires a valid model file path.
pub fn create_test_agent_config(temp_dir: &std::path::Path) -> crate::types::AgentConfig {
    use crate::types::{
        AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
        SessionConfig,
    };

    AgentConfig {
        model: ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.to_path_buf(),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig::default(),
        mcp_servers: Vec::new(),
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    }
}

/// Helper to create a test ACP server
///
/// This creates a minimal ACP server for testing. Note that this still requires
/// valid model files to be present.
///
/// For tests that don't need actual inference, consider using `#[ignore]` and
/// testing individual components instead.
pub async fn create_test_acp_server(
    temp_dir: &std::path::Path,
) -> Result<Arc<crate::acp::AcpServer>, Box<dyn std::error::Error>> {
    let agent_config = create_test_agent_config(temp_dir);
    let acp_config = create_test_acp_config(temp_dir);

    // Create all the components needed for AgentServer
    let model_manager = Arc::new(crate::model::ModelManager::new(agent_config.model.clone())?);
    let request_queue = Arc::new(crate::queue::RequestQueue::new(
        model_manager.clone(),
        agent_config.queue_config.clone(),
        agent_config.session_config.clone(),
    ));
    let session_manager = Arc::new(crate::session::SessionManager::new(
        agent_config.session_config.clone(),
    ));
    let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
    let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
    let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
        agent_config.parallel_execution_config.clone(),
    ));

    // Create an AgentServer instance
    let agent_server = Arc::new(crate::AgentServer::new(
        model_manager,
        request_queue,
        session_manager,
        mcp_client,
        chat_template,
        dependency_analyzer,
        agent_config,
    ));

    // Create the ACP server
    let server = crate::acp::AcpServer::new(agent_server, acp_config);
    Ok(Arc::new(server))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_test_acp_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_acp_config(temp_dir.path());

        assert_eq!(config.filesystem.allowed_paths.len(), 1);
        assert_eq!(config.filesystem.blocked_paths.len(), 0);
        assert_eq!(config.filesystem.max_file_size, 1024 * 1024);
    }

    #[test]
    fn test_create_test_agent_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_agent_config(temp_dir.path());

        assert_eq!(config.model.batch_size, 512);
        assert_eq!(config.model.n_seq_max, 1);
        assert_eq!(config.mcp_servers.len(), 0);
    }
}
