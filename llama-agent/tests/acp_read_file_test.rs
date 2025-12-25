//! Test file reading via ACP protocol
//!
//! Tests implementation-specific security and error handling for fs/read_text_file.
//! Basic conformance tests are in acp-conformance crate.
//!
//! These tests verify:
//! - Path security (allowed_paths, relative paths)
//! - Error handling (not found, size limits)

mod acp_read_file_tests {
    use llama_agent::acp::AcpServer;
    use llama_agent::AgentServer;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Create a test ACP server instance
    ///
    /// This helper creates a minimal ACP server with:
    /// - Temporary directory for model files (not actually used)
    /// - Default configuration
    /// - Filesystem operations enabled
    async fn create_test_server(
        temp_dir: &TempDir,
    ) -> Result<Arc<AcpServer>, Box<dyn std::error::Error>> {
        use llama_agent::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };

        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Create minimal agent config for testing
        let agent_config = AgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
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
        };

        // Create all the components needed for AgentServer
        let model_manager = Arc::new(
            llama_agent::model::ModelManager::new(agent_config.model.clone())
                .expect("Failed to create model manager"),
        );
        let request_queue = Arc::new(llama_agent::queue::RequestQueue::new(
            model_manager.clone(),
            agent_config.queue_config.clone(),
            agent_config.session_config.clone(),
        ));
        let session_manager = Arc::new(llama_agent::session::SessionManager::new(
            agent_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn llama_agent::mcp::MCPClient> =
            Arc::new(llama_agent::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(llama_agent::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer =
            Arc::new(llama_agent::dependency_analysis::DependencyAnalyzer::new(
                agent_config.parallel_execution_config.clone(),
            ));

        // Create an AgentServer instance
        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            agent_config,
        ));

        // Create ACP config with filesystem enabled for temp_dir
        let acp_config = llama_agent::acp::config::AcpConfig {
            filesystem: llama_agent::acp::config::FilesystemSettings {
                allowed_paths: vec![temp_dir.path().to_path_buf()],
                blocked_paths: vec![],
                max_file_size: 1024 * 1024, // 1MB
            },
            ..Default::default()
        };

        // Create the ACP server
        let server = AcpServer::new(agent_server, acp_config).0;
        Ok(Arc::new(server))
    }

    /// Test reading a file outside allowed paths
    #[tokio::test]
    async fn test_read_text_file_path_security() {
        use agent_client_protocol::Agent;

        // Create temp directory for server config
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a different directory outside allowed paths
        let outside_dir = TempDir::new().expect("Failed to create outside dir");
        let blocked_file = outside_dir.path().join("secret.txt");
        std::fs::write(&blocked_file, "secret data").expect("Failed to write blocked file");

        // Create server with restricted paths
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with fs capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(false),
            ),
        );

        let _init_response = server
            .initialize(init_request)
            .await
            .expect("Initialize failed");

        // Create a session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(temp_dir.path().to_path_buf());
        let session_response = server
            .new_session(new_session_request)
            .await
            .expect("New session failed");
        let session_id = session_response.session_id;

        // Try to read file outside allowed paths
        let read_request = agent_client_protocol::ReadTextFileRequest::new(
            session_id.clone(),
            blocked_file.to_string_lossy().to_string(),
        );

        let request_json =
            serde_json::to_value(&read_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/read_text_file", Arc::from(raw_value));

        // Call should fail due to path security
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when trying to read file outside allowed paths"
        );
    }

    /// Test reading a nonexistent file
    #[tokio::test]
    async fn test_read_text_file_not_found() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let nonexistent_file = temp_dir.path().join("nonexistent.txt");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with fs capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(false),
            ),
        );

        let _init_response = server
            .initialize(init_request)
            .await
            .expect("Initialize failed");

        // Create a session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(temp_dir.path().to_path_buf());
        let session_response = server
            .new_session(new_session_request)
            .await
            .expect("New session failed");
        let session_id = session_response.session_id;

        // Try to read nonexistent file
        let read_request = agent_client_protocol::ReadTextFileRequest::new(
            session_id.clone(),
            nonexistent_file.to_string_lossy().to_string(),
        );

        let request_json =
            serde_json::to_value(&read_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/read_text_file", Arc::from(raw_value));

        // Call should fail - file doesn't exist
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when trying to read nonexistent file"
        );
    }

    /// Test reading a file with relative path (should be rejected)
    #[tokio::test]
    async fn test_read_text_file_relative_path() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with fs capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(false),
            ),
        );

        let _init_response = server
            .initialize(init_request)
            .await
            .expect("Initialize failed");

        // Create a session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(temp_dir.path().to_path_buf());
        let session_response = server
            .new_session(new_session_request)
            .await
            .expect("New session failed");
        let session_id = session_response.session_id;

        // Try to read file with relative path
        let read_request = agent_client_protocol::ReadTextFileRequest::new(
            session_id.clone(),
            "relative/path/test.txt".to_string(),
        );

        let request_json =
            serde_json::to_value(&read_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/read_text_file", Arc::from(raw_value));

        // Call should fail - relative paths not allowed
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when trying to use relative path"
        );
    }

    /// Test reading a large file (should respect max_file_size limit)
    #[tokio::test]
    async fn test_read_text_file_size_limit() {
        use agent_client_protocol::Agent;

        // Create temp directory and large test file
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let large_file = temp_dir.path().join("large.txt");

        // Create a 2MB file (larger than our 1MB limit)
        let large_content = "x".repeat(2 * 1024 * 1024);
        std::fs::write(&large_file, &large_content).expect("Failed to write large file");

        // Create server with 1MB file size limit
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with fs capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(false),
            ),
        );

        let _init_response = server
            .initialize(init_request)
            .await
            .expect("Initialize failed");

        // Create a session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(temp_dir.path().to_path_buf());
        let session_response = server
            .new_session(new_session_request)
            .await
            .expect("New session failed");
        let session_id = session_response.session_id;

        // Try to read large file
        let read_request = agent_client_protocol::ReadTextFileRequest::new(
            session_id.clone(),
            large_file.to_string_lossy().to_string(),
        );

        let request_json =
            serde_json::to_value(&read_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/read_text_file", Arc::from(raw_value));

        // Call should fail due to file size limit
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when file exceeds max_file_size limit"
        );
    }
}
