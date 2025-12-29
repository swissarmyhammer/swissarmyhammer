//! Test file writing via ACP protocol
//!
//! This test verifies the fs/write_text_file extension method:
//! 1. Initializes an ACP session with write capability
//! 2. Calls fs/write_text_file via ext_method
//! 3. Verifies the file was created with correct content
//! 4. Tests various security and error conditions

#[cfg(feature = "acp")]
mod acp_write_file_tests {
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
        let server = AcpServer::new(agent_server, acp_config);
        Ok(Arc::new(server))
    }

    /// Test writing a text file via ACP fs/write_text_file
    #[tokio::test]
    async fn test_write_text_file_success() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("output.txt");
        let test_content = "Hello, ACP Write!\nThis is a test file.\n";

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with client capabilities
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Create fs/write_text_file request
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            test_content.to_string(),
        );

        // Serialize to JSON Value
        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");

        // Create ExtRequest
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        // Call ext_method
        let ext_response = server
            .ext_method(ext_request)
            .await
            .expect("ext_method failed");

        // Parse response
        let response_value: serde_json::Value =
            serde_json::from_str(ext_response.0.get()).expect("Failed to parse response");
        let _response: agent_client_protocol::WriteTextFileResponse =
            serde_json::from_value(response_value).expect("Failed to deserialize response");

        // Verify file was created with correct content
        let written_content =
            std::fs::read_to_string(&test_file_path).expect("Failed to read written file");
        assert_eq!(
            written_content, test_content,
            "Written content should match requested content"
        );
    }

    /// Test writing a file without required capability
    #[tokio::test]
    async fn test_write_text_file_without_capability() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("output.txt");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize without write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(false), // Capability disabled
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

        // Try to write file
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            "test content".to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        // Call should fail due to missing capability
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when client doesn't have write_text_file capability"
        );
    }

    /// Test writing a file outside allowed paths
    #[tokio::test]
    async fn test_write_text_file_path_security() {
        use agent_client_protocol::Agent;

        // Create temp directory for server config
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a different directory outside allowed paths
        let outside_dir = TempDir::new().expect("Failed to create outside dir");
        let blocked_file = outside_dir.path().join("blocked.txt");

        // Create server with restricted paths
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Try to write file outside allowed paths
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            blocked_file.to_string_lossy().to_string(),
            "secret data".to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        // Call should fail due to path security
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when trying to write file outside allowed paths"
        );
    }

    /// Test writing to a file with relative path (should be rejected)
    #[tokio::test]
    async fn test_write_text_file_relative_path() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Try to write file with relative path
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            "relative/path/test.txt".to_string(),
            "test content".to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        // Call should fail - relative paths not allowed
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when trying to use relative path"
        );
    }

    /// Test overwriting an existing file
    #[tokio::test]
    async fn test_write_text_file_overwrite() {
        use agent_client_protocol::Agent;

        // Create temp directory and initial test file
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("overwrite.txt");
        let initial_content = "Initial content";
        let new_content = "New content after overwrite";
        std::fs::write(&test_file_path, initial_content).expect("Failed to write initial file");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Overwrite the file
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            new_content.to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        let _ext_response = server
            .ext_method(ext_request)
            .await
            .expect("ext_method failed");

        // Verify file was overwritten with new content
        let written_content =
            std::fs::read_to_string(&test_file_path).expect("Failed to read written file");
        assert_eq!(
            written_content, new_content,
            "File should be overwritten with new content"
        );
    }

    /// Test writing an empty file
    #[tokio::test]
    async fn test_write_text_file_empty() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("empty.txt");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Write empty file
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            String::new(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        let _ext_response = server
            .ext_method(ext_request)
            .await
            .expect("ext_method failed");

        // Verify empty file was created
        let written_content =
            std::fs::read_to_string(&test_file_path).expect("Failed to read written file");
        assert_eq!(written_content, "", "File should be empty");
    }

    /// Test writing to a subdirectory
    #[tokio::test]
    async fn test_write_text_file_subdirectory() {
        use agent_client_protocol::Agent;

        // Create temp directory and subdirectory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let subdir = temp_dir.path().join("subdir");
        std::fs::create_dir(&subdir).expect("Failed to create subdir");
        let test_file_path = subdir.join("test.txt");
        let test_content = "Content in subdirectory";

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Write to subdirectory
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            test_content.to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        let _ext_response = server
            .ext_method(ext_request)
            .await
            .expect("ext_method failed");

        // Verify file was created in subdirectory
        let written_content =
            std::fs::read_to_string(&test_file_path).expect("Failed to read written file");
        assert_eq!(
            written_content, test_content,
            "File in subdirectory should have correct content"
        );
    }

    /// Test writing a file with unicode content
    #[tokio::test]
    async fn test_write_text_file_unicode() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("unicode.txt");
        let test_content = "Hello 世界! 🌍 Привет мир! مرحبا بالعالم";

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Write file with unicode content
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            test_content.to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        let _ext_response = server
            .ext_method(ext_request)
            .await
            .expect("ext_method failed");

        // Verify unicode content was written correctly
        let written_content =
            std::fs::read_to_string(&test_file_path).expect("Failed to read written file");
        assert_eq!(
            written_content, test_content,
            "Unicode content should be preserved"
        );
    }

    /// Test writing to a nonexistent parent directory (should fail)
    #[tokio::test]
    async fn test_write_text_file_nonexistent_parent() {
        use agent_client_protocol::Agent;

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file_path = temp_dir.path().join("nonexistent").join("test.txt");

        // Create server
        let server = create_test_server(&temp_dir)
            .await
            .expect("Failed to create server");

        // Initialize with write capability
        let init_request = agent_client_protocol::InitializeRequest::new(
            agent_client_protocol::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new().fs(
                agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(true),
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

        // Try to write to nonexistent parent directory
        let write_request = agent_client_protocol::WriteTextFileRequest::new(
            session_id.clone(),
            test_file_path.to_string_lossy().to_string(),
            "test content".to_string(),
        );

        let request_json =
            serde_json::to_value(&write_request).expect("Failed to serialize request");
        let raw_value = agent_client_protocol::RawValue::from_string(request_json.to_string())
            .expect("Failed to create RawValue");
        let ext_request =
            agent_client_protocol::ExtRequest::new("fs/write_text_file", Arc::from(raw_value));

        // Call should fail - parent directory doesn't exist
        let result = server.ext_method(ext_request).await;
        assert!(
            result.is_err(),
            "Should fail when parent directory doesn't exist"
        );
    }
}
