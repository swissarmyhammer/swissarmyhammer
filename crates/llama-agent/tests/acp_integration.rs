//! Integration tests for ACP protocol support
//!
//! These tests verify the basic ACP protocol flow:
//! initialize → new_session → prompt
//!
//! Reference: ideas/acp.md
//!
//! NOTE: The main test is currently ignored because it requires the Agent trait
//! to be implemented on AcpServer. This is tracked in the implementation todo list.
//!
//! The tests are structured to match the expected flow once implementation is complete.

mod acp_tests {
    use llama_agent::acp::AcpServer;
    use llama_agent::types::{ModelConfig, ModelSource};
    use llama_agent::AgentServer;
    use serial_test::serial;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_server() -> Result<Arc<AcpServer>, Box<dyn std::error::Error>> {
        use llama_agent::types::{ParallelConfig, QueueConfig, RetryConfig, SessionConfig};

        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Create a temporary directory for the test model
        let temp_dir = TempDir::new()?;

        // Create minimal agent config for testing
        let agent_config = llama_agent::types::AgentConfig {
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

        let acp_config = llama_agent::acp::config::AcpConfig::default();

        // Create the ACP server
        let server = AcpServer::new(agent_server, acp_config).0;
        Ok(Arc::new(server))
    }

    /// Test basic protocol initialization
    ///
    /// Tests the ACP protocol flow: initialize -> new_session
    #[tokio::test]
    #[serial]
    async fn test_basic_acp_protocol() {
        let server = create_test_server().await.expect("Failed to create server");

        // Test initialize
        let init_request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::schema::ClientCapabilities::new()
                .fs(agent_client_protocol::schema::FileSystemCapabilities::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true),
        );
        let _init_response = server
            .initialize(init_request)
            .await
            .expect("Initialize failed");

        // Test new_session
        let session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::path::PathBuf::from("/tmp"));
        let _session_response = server
            .new_session(session_request)
            .await
            .expect("New session failed");

        // TODO: Add more protocol flow tests once Agent trait is implemented
    }

    /// Test shutdown coordination with broadcast channel
    ///
    /// This test verifies that when the request handler completes (simulating client disconnect),
    /// it signals the notification handler to shut down gracefully via a broadcast channel.
    ///
    /// The implementation follows the same pattern as claude-agent/src/server.rs:
    /// - Request handler signals on completion
    /// - Notification handler monitors via tokio::select!
    /// - Both handlers complete gracefully without hanging
    ///
    /// Test shutdown coordination without requiring a model
    ///
    /// This is a unit test that verifies the shutdown coordination mechanism works
    /// correctly without needing to initialize a full AgentServer with a model.
    #[tokio::test]
    async fn test_shutdown_coordination_without_model() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::sync::broadcast;
        use tokio::time::{timeout, Duration};

        // Create mock stdio channels
        let (client_writer, server_reader) = tokio::io::duplex(1024);
        let (server_writer, _client_reader) = tokio::io::duplex(1024);

        // Create broadcast channel for shutdown coordination (same as AcpServer::run_stdio)
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

        // Create notification channel
        let (notification_tx, mut notification_rx) = tokio::sync::mpsc::unbounded_channel::<
            agent_client_protocol::schema::SessionNotification,
        >();

        // Simulate request handler - reads until EOF then signals shutdown
        let request_handler = async move {
            let mut lines = BufReader::new(server_reader).lines();
            while let Ok(Some(_line)) = lines.next_line().await {
                // Process lines (we'll close the writer to trigger EOF)
            }
            // Signal shutdown when reader closes
            let _ = shutdown_tx.send(());
        };

        // Simulate notification handler - monitors both notification_rx and shutdown_rx
        let notification_handler = async move {
            let _writer = server_writer; // Keep writer alive
            loop {
                tokio::select! {
                    notification = notification_rx.recv() => {
                        if notification.is_none() {
                            break;
                        }
                        // Process notification
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Notification handler received shutdown signal");
                        break;
                    }
                }
            }
        };

        // Spawn both handlers
        let request_handle = tokio::spawn(request_handler);
        let notification_handle = tokio::spawn(notification_handler);

        // Simulate client disconnect - close the writer to trigger EOF
        drop(client_writer);
        drop(notification_tx);

        // Wait for both handlers to complete with timeout
        let result = timeout(Duration::from_secs(2), async {
            let req_result = request_handle.await;
            let notif_result = notification_handle.await;
            (req_result, notif_result)
        })
        .await;

        match result {
            Ok((Ok(()), Ok(()))) => {
                // Success - both handlers completed gracefully
            }
            Ok((Err(e), _)) => {
                panic!("Request handler task panicked: {:?}", e);
            }
            Ok((_, Err(e))) => {
                panic!("Notification handler task panicked: {:?}", e);
            }
            Err(_) => {
                panic!("Handlers did not shut down within timeout - broadcast channel coordination failed");
            }
        }
    }

    /// Test load_session with nonexistent session ID
    ///
    /// This test verifies that loading a nonexistent session returns an error.
    #[tokio::test]
    #[serial]
    async fn test_load_session_nonexistent() {
        use std::path::PathBuf;

        let server = create_test_server().await.expect("Failed to create server");

        // Try to load a nonexistent session
        let load_request = agent_client_protocol::schema::LoadSessionRequest::new(
            agent_client_protocol::schema::SessionId::new("01HZZZZZZZZZZZZZZZZZZZZZZ"),
            PathBuf::from("/tmp"),
        );

        let result = server.load_session(load_request).await;

        // Verify that an error is returned
        assert!(
            result.is_err(),
            "Loading nonexistent session should return error"
        );
    }

    /// Test load_session with invalid session ID format
    ///
    /// This test verifies that loading a session with an invalid ID format returns an error.
    #[tokio::test]
    #[serial]
    async fn test_load_session_invalid_id_format() {
        use std::path::PathBuf;

        let server = create_test_server().await.expect("Failed to create server");

        // Try to load with an invalid session ID format
        let load_request = agent_client_protocol::schema::LoadSessionRequest::new(
            agent_client_protocol::schema::SessionId::new("not-a-valid-ulid"),
            PathBuf::from("/tmp"),
        );

        let result = server.load_session(load_request).await;

        // Verify that an error is returned
        assert!(
            result.is_err(),
            "Loading session with invalid ID format should return error"
        );
    }

    /// Test session state preservation across save and load
    ///
    /// This test verifies that all session state (messages, tools, prompts, metadata)
    /// is correctly preserved when a session is saved to storage and then loaded back.
    #[tokio::test]
    async fn test_session_state_preservation() {
        use llama_agent::types::{Message, MessageRole, SessionConfig, ToolCallId};
        use llama_agent::SessionManager;
        use std::time::SystemTime;
        use tempfile::TempDir;

        // Helper to create messages with proper fields
        fn create_message(
            role: MessageRole,
            content: &str,
            tool_call_id: Option<ToolCallId>,
            tool_name: Option<String>,
        ) -> Message {
            Message {
                role,
                content: content.to_string(),
                tool_call_id,
                tool_name,
                timestamp: SystemTime::now(),
            }
        }

        // Create a session manager with persistence enabled
        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config.clone());

        // Create a session with comprehensive state
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Add multiple messages with different roles
        let messages = [
            create_message(
                MessageRole::System,
                "You are a helpful assistant.",
                None,
                None,
            ),
            create_message(MessageRole::User, "What is the weather today?", None, None),
            create_message(
                MessageRole::Assistant,
                "Let me check the weather for you.",
                None,
                None,
            ),
            create_message(
                MessageRole::Tool,
                r#"{"temperature": 72, "condition": "sunny"}"#,
                Some(ToolCallId::new()),
                Some("get_weather".to_string()),
            ),
            create_message(
                MessageRole::Assistant,
                "The weather is sunny with a temperature of 72°F.",
                None,
                None,
            ),
        ];

        for message in messages.iter() {
            manager
                .add_message(&session_id, message.clone())
                .await
                .unwrap();
        }

        // Get the original session to verify original state
        let original_session = manager.get_session(&session_id).await.unwrap().unwrap();

        // Save the session
        manager.save_session(&session_id).await.unwrap();

        // Create a new manager with the same storage directory to simulate a restart
        let manager2 = SessionManager::new(config);

        // Restore sessions from storage
        let restored_count = manager2.restore_sessions().await.unwrap();
        assert_eq!(restored_count, 1, "Should restore exactly one session");

        // Get the restored session
        let restored_session = manager2
            .get_session(&session_id)
            .await
            .unwrap()
            .expect("Session should exist after restoration");

        // Verify all session state is preserved

        // 1. Session ID
        assert_eq!(
            restored_session.id, session_id,
            "Session ID should be preserved"
        );

        // 2. Messages - count and content
        assert_eq!(
            restored_session.messages.len(),
            messages.len(),
            "All messages should be preserved"
        );

        for (i, (original, restored)) in messages
            .iter()
            .zip(restored_session.messages.iter())
            .enumerate()
        {
            assert_eq!(
                restored.role, original.role,
                "Message {} role should be preserved",
                i
            );
            assert_eq!(
                restored.content, original.content,
                "Message {} content should be preserved",
                i
            );
            assert_eq!(
                restored.tool_call_id, original.tool_call_id,
                "Message {} tool_call_id should be preserved",
                i
            );
            assert_eq!(
                restored.tool_name, original.tool_name,
                "Message {} tool_name should be preserved",
                i
            );
        }

        // 3. Tools (verify they match original session)
        assert_eq!(
            restored_session.available_tools.len(),
            original_session.available_tools.len(),
            "Tool count should be preserved"
        );

        for (i, (original, restored)) in original_session
            .available_tools
            .iter()
            .zip(restored_session.available_tools.iter())
            .enumerate()
        {
            assert_eq!(
                restored.name, original.name,
                "Tool {} name should be preserved",
                i
            );
            assert_eq!(
                restored.description, original.description,
                "Tool {} description should be preserved",
                i
            );
            assert_eq!(
                restored.parameters, original.parameters,
                "Tool {} parameters should be preserved",
                i
            );
        }

        // 4. Metadata timestamps
        assert!(
            restored_session.created_at <= SystemTime::now(),
            "Created timestamp should be valid"
        );
        assert!(
            restored_session.updated_at <= SystemTime::now(),
            "Updated timestamp should be valid"
        );

        // 5. Collections should be preserved
        assert_eq!(
            restored_session.mcp_servers.len(),
            original_session.mcp_servers.len(),
            "MCP servers list should be preserved"
        );
        assert_eq!(
            restored_session.available_prompts.len(),
            original_session.available_prompts.len(),
            "Prompts list should be preserved"
        );
        assert_eq!(
            restored_session.compaction_history.len(),
            original_session.compaction_history.len(),
            "Compaction history should be preserved"
        );

        // 6. Optional fields
        assert_eq!(
            restored_session.transcript_path, original_session.transcript_path,
            "transcript_path should be preserved"
        );

        // 7. ACP-specific fields (when feature is enabled)
        // Note: todos are now stored in the kanban board, not in the session
        {
            assert_eq!(
                restored_session.available_commands.len(),
                original_session.available_commands.len(),
                "Commands list should be preserved"
            );
            assert_eq!(
                restored_session.current_mode, original_session.current_mode,
                "current_mode should be preserved"
            );
        }
    }

    /// Test that file read operations require the fs.read_text_file capability
    ///
    /// This test verifies that the filesystem handler correctly checks the
    /// fs.read_text_file capability before allowing file read operations.
    #[tokio::test]
    async fn test_file_read_requires_capability() {
        use agent_client_protocol::schema::{
            ClientCapabilities, FileSystemCapabilities, ReadTextFileRequest,
        };
        use llama_agent::acp::config::FilesystemSettings;
        use llama_agent::acp::filesystem::FilesystemOperations;
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::types::ids::SessionId;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();

        // Create session state WITHOUT read_text_file capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(false) // Capability explicitly disabled
                .write_text_file(true)),
        );

        // Create filesystem handler
        let settings = FilesystemSettings {
            allowed_paths: vec![temp_dir.path().to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 10_000_000,
        };
        let handler = FilesystemOperations::new(&settings);

        // Attempt to read a file - this should fail because the capability is not available
        let read_request =
            ReadTextFileRequest::new(session.session_id.clone(), test_file.display().to_string());

        let result = handler.read_text_file(&session, read_request).await;

        // Verify that the operation failed with an appropriate error
        assert!(
            result.is_err(),
            "File read should fail when fs.read_text_file capability is not available"
        );

        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("support") && error_msg.contains("read_text_file"),
            "Error message should mention capability requirement. Got: {}",
            error_msg
        );
    }

    /// Test that file read operations succeed WITH the capability
    ///
    /// This test verifies that file read operations work correctly when
    /// the client has the required capability.
    #[tokio::test]
    async fn test_file_read_with_capability() {
        use agent_client_protocol::schema::{
            ClientCapabilities, FileSystemCapabilities, ReadTextFileRequest,
        };
        use llama_agent::acp::config::FilesystemSettings;
        use llama_agent::acp::filesystem::FilesystemOperations;
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::types::ids::SessionId;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let test_content = "test content";
        tokio::fs::write(&test_file, test_content).await.unwrap();

        // Create session state WITH read_text_file capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(true)),
        );

        // Create filesystem handler
        let settings = FilesystemSettings {
            allowed_paths: vec![temp_dir.path().to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 10_000_000,
        };
        let handler = FilesystemOperations::new(&settings);

        // Attempt to read a file - this should succeed
        let read_request =
            ReadTextFileRequest::new(session.session_id.clone(), test_file.display().to_string());

        let result = handler.read_text_file(&session, read_request).await;

        // Verify that the operation succeeded
        assert!(
            result.is_ok(),
            "File read should succeed with fs.read_text_file capability"
        );

        let response = result.unwrap();
        assert_eq!(response.content, test_content, "File content should match");
    }

    /// Test that file write operations require the fs.write_text_file capability
    ///
    /// This test verifies that the filesystem handler correctly checks the
    /// fs.write_text_file capability before allowing file write operations.
    #[tokio::test]
    async fn test_file_write_requires_capability() {
        use agent_client_protocol::schema::{
            ClientCapabilities, FileSystemCapabilities, WriteTextFileRequest,
        };
        use llama_agent::acp::config::FilesystemSettings;
        use llama_agent::acp::filesystem::FilesystemOperations;
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::types::ids::SessionId;
        use tempfile::TempDir;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Create session state WITHOUT write_text_file capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new().fs(
                FileSystemCapabilities::new()
                    .read_text_file(true)
                    .write_text_file(false), // Capability explicitly disabled
            ),
        );

        // Create filesystem handler
        let settings = FilesystemSettings {
            allowed_paths: vec![temp_dir.path().to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 10_000_000,
        };
        let handler = FilesystemOperations::new(&settings);

        // Attempt to write a file - this should fail because the capability is not available
        let write_request = WriteTextFileRequest::new(
            session.session_id.clone(),
            test_file.display().to_string(),
            "test content".to_string(),
        );

        let result = handler.write_text_file(&session, write_request).await;

        // Verify that the operation failed with an appropriate error
        assert!(
            result.is_err(),
            "File write should fail when fs.write_text_file capability is not available"
        );

        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("support") && error_msg.contains("write_text_file"),
            "Error message should mention capability requirement. Got: {}",
            error_msg
        );
    }

    /// Test that file write operations succeed WITH the capability
    ///
    /// This test verifies that file write operations work correctly when
    /// the client has the required capability.
    #[tokio::test]
    async fn test_file_write_with_capability() {
        use agent_client_protocol::schema::{
            ClientCapabilities, FileSystemCapabilities, WriteTextFileRequest,
        };
        use llama_agent::acp::config::FilesystemSettings;
        use llama_agent::acp::filesystem::FilesystemOperations;
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::types::ids::SessionId;
        use tempfile::TempDir;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Create session state WITH write_text_file capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(true)),
        );

        // Create filesystem handler
        let settings = FilesystemSettings {
            allowed_paths: vec![temp_dir.path().to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 10_000_000,
        };
        let handler = FilesystemOperations::new(&settings);

        // Attempt to write a file - this should succeed
        let test_content = "test content";
        let write_request = WriteTextFileRequest::new(
            session.session_id.clone(),
            test_file.display().to_string(),
            test_content.to_string(),
        );

        let result = handler.write_text_file(&session, write_request).await;

        // Verify that the operation succeeded
        assert!(
            result.is_ok(),
            "File write should succeed with fs.write_text_file capability"
        );

        // Verify the file was actually written
        let content = tokio::fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, test_content, "File content should match");
    }

    /// Test that terminal operations require the terminal capability
    ///
    /// This test verifies that the terminal manager correctly checks the
    /// terminal capability before allowing terminal creation.
    #[tokio::test]
    async fn test_terminal_create_requires_capability() {
        use agent_client_protocol::schema::{ClientCapabilities, FileSystemCapabilities};
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::acp::terminal::{CreateTerminalRequest, TerminalManager};
        use llama_agent::types::ids::SessionId;

        // Create session state WITHOUT terminal capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new()
                .fs(FileSystemCapabilities::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(false), // Capability explicitly disabled
        );

        // Create terminal manager
        let mut manager = TerminalManager::new();
        manager.set_client_capabilities(session.client_capabilities.clone());

        // Attempt to create a terminal - this should fail because the capability is not available
        let terminal_request = CreateTerminalRequest {
            command: "echo test".to_string(),
        };

        let result = manager.create_terminal(terminal_request).await;

        // Verify that the operation failed with an appropriate error
        assert!(
            result.is_err(),
            "Terminal creation should fail when terminal capability is not available"
        );

        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("capability") || error_msg.contains("support"),
            "Error message should mention capability requirement. Got: {}",
            error_msg
        );
    }

    /// Test that terminal operations succeed WITH the capability
    ///
    /// This test verifies that terminal creation works correctly when
    /// the client has the required capability.
    #[tokio::test]
    async fn test_terminal_create_with_capability() {
        use agent_client_protocol::schema::{ClientCapabilities, FileSystemCapabilities};
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::acp::terminal::{CreateTerminalRequest, TerminalManager};
        use llama_agent::types::ids::SessionId;

        // Create session state WITH terminal capability
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new()
                .fs(FileSystemCapabilities::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true), // Capability enabled
        );

        // Create terminal manager
        let mut manager = TerminalManager::new();
        manager.set_client_capabilities(session.client_capabilities.clone());

        // Attempt to create a terminal - this should succeed
        let terminal_request = CreateTerminalRequest {
            command: "echo test".to_string(),
        };

        let result = manager.create_terminal(terminal_request).await;

        // Verify that the operation succeeded
        assert!(
            result.is_ok(),
            "Terminal creation should succeed with terminal capability"
        );

        let response = result.unwrap();
        assert!(
            response.terminal_id.starts_with("term_"),
            "Terminal ID should have proper prefix"
        );
    }

    /// Test that all operations with capabilities disabled fail gracefully
    ///
    /// This test verifies that when a client has no filesystem or terminal
    /// capabilities, all related operations fail with appropriate errors.
    #[tokio::test]
    async fn test_no_capabilities_fails_all_operations() {
        use agent_client_protocol::schema::{
            ClientCapabilities, FileSystemCapabilities, ReadTextFileRequest, WriteTextFileRequest,
        };
        use llama_agent::acp::config::FilesystemSettings;
        use llama_agent::acp::filesystem::FilesystemOperations;
        use llama_agent::acp::session::AcpSessionState;
        use llama_agent::acp::terminal::{CreateTerminalRequest, TerminalManager};
        use llama_agent::types::ids::SessionId;
        use tempfile::TempDir;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();

        // Create session state with NO capabilities
        let session = AcpSessionState::with_capabilities(
            SessionId::new(),
            ClientCapabilities::new()
                .fs(FileSystemCapabilities::new()
                    .read_text_file(false)
                    .write_text_file(false))
                .terminal(false),
        );

        // Create handlers
        let settings = FilesystemSettings {
            allowed_paths: vec![temp_dir.path().to_path_buf()],
            blocked_paths: vec![],
            max_file_size: 10_000_000,
        };
        let fs_handler = FilesystemOperations::new(&settings);
        let mut terminal_manager = TerminalManager::new();
        terminal_manager.set_client_capabilities(session.client_capabilities.clone());

        // Test file read fails
        let read_request =
            ReadTextFileRequest::new(session.session_id.clone(), test_file.display().to_string());
        let read_result = fs_handler.read_text_file(&session, read_request).await;
        assert!(
            read_result.is_err(),
            "File read should fail without capability"
        );

        // Test file write fails
        let write_request = WriteTextFileRequest::new(
            session.session_id.clone(),
            test_file.display().to_string(),
            "content".to_string(),
        );
        let write_result = fs_handler.write_text_file(&session, write_request).await;
        assert!(
            write_result.is_err(),
            "File write should fail without capability"
        );

        // Test terminal create fails
        let terminal_request = CreateTerminalRequest {
            command: "echo test".to_string(),
        };
        let terminal_result = terminal_manager.create_terminal(terminal_request).await;
        assert!(
            terminal_result.is_err(),
            "Terminal creation should fail without capability"
        );

        // Verify all errors mention capability requirements
        assert!(
            read_result.unwrap_err().to_string().contains("support"),
            "File read error should mention capability"
        );
        assert!(
            write_result.unwrap_err().to_string().contains("support"),
            "File write error should mention capability"
        );
        assert!(
            terminal_result.unwrap_err().to_string().contains("support"),
            "Terminal error should mention capability"
        );
    }
}

/// Integration tests for `session/resume` and `session/load`.
///
/// These exercise the chat-template-re-render resume path end to end:
/// persist a [`SessionRecord`] to the shared [`SessionStore`], then drive
/// `session/resume` (restore only) and `session/load` (restore plus replay)
/// against it.
///
/// The server is built without loading a model — the chat-template re-render
/// inside `ResumeStrategy::restore` is best-effort and is skipped (with a
/// warning) when no model is loaded. The load-bearing step under test is the
/// reconstruction of the in-memory llama session from the durable record, and
/// the `session/load` history replay.
mod session_resume_tests {
    use agent_client_protocol::schema::{
        ContentBlock, ContentChunk, LoadSessionRequest, ResumeSessionRequest, SessionId,
        SessionUpdate, TextContent,
    };
    use agent_client_protocol_extras::{SessionRecord, SessionStore};
    use llama_agent::acp::config::AcpConfig;
    use llama_agent::acp::AcpServer;
    use llama_agent::types::ids::SessionId as LlamaSessionId;
    use llama_agent::types::{
        AgentConfig, MessageRole, ModelConfig, ModelSource, ParallelConfig, QueueConfig,
        RetryConfig, SessionConfig,
    };
    use llama_agent::AgentServer;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// RAII guard returned by [`with_temp_state`]: points `XDG_STATE_HOME` at a
    /// fresh temp directory and, on drop, restores the env var to whatever it
    /// was before (removing it if it was unset).
    struct TempState {
        /// The temp directory backing the isolated state tree; dropped last.
        _temp: TempDir,
        /// The `XDG_STATE_HOME` value captured before the override, restored on
        /// drop. `None` means the variable was unset.
        previous: Option<std::ffi::OsString>,
    }

    impl Drop for TempState {
        fn drop(&mut self) {
            // SAFETY: callers are `#[serial]`, so no other thread reads or
            // writes the env var concurrently.
            match self.previous.take() {
                Some(value) => std::env::set_var("XDG_STATE_HOME", value),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
        }
    }

    /// Point `XDG_STATE_HOME` at a fresh temp directory so the shared
    /// `SessionStore` resolves into isolated, disposable state.
    ///
    /// Returns a [`TempState`] RAII guard: the caller must bind it (e.g.
    /// `let _state = with_temp_state();`) so the temp directory stays alive for
    /// the test and the prior `XDG_STATE_HOME` value is restored on drop.
    ///
    /// Serialized at the call site with `#[serial]`: this mutates the
    /// process-global `XDG_STATE_HOME` env var.
    fn with_temp_state() -> TempState {
        let temp = TempDir::new().expect("temp dir for XDG_STATE_HOME");
        let previous = std::env::var_os("XDG_STATE_HOME");
        // SAFETY: callers are `#[serial]`, so no other thread reads or writes
        // the env var concurrently; the previous value is restored on drop.
        std::env::set_var("XDG_STATE_HOME", temp.path());
        TempState {
            _temp: temp,
            previous,
        }
    }

    /// Build an `AcpServer` for resume/load tests without loading a model.
    ///
    /// Returns the server and the broadcast receiver so a test can observe the
    /// `session/update` notifications a `session/load` replays.
    async fn build_server() -> (
        Arc<AcpServer>,
        tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
    ) {
        let temp_dir = TempDir::new().expect("temp dir for model folder");
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

        let model_manager = Arc::new(
            llama_agent::model::ModelManager::new(agent_config.model.clone())
                .expect("model manager"),
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

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            agent_config,
        ));

        let (server, rx) = AcpServer::new(agent_server, AcpConfig::default());
        (Arc::new(server), rx)
    }

    /// Persist a `SessionRecord` carrying a user/assistant exchange under a
    /// fresh llama ULID id, and return that id as a string.
    fn persist_record_with_exchange() -> String {
        let id = LlamaSessionId::new().to_string();
        let mut record = SessionRecord::new(&id, PathBuf::from("/tmp"), "2026-05-18T12:00:00Z");
        record.updates = vec![
            SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("remember the number is 42".to_string()),
            ))),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("understood, the number is 42".to_string()),
            ))),
        ];
        SessionStore::new()
            .persist(&record)
            .expect("persist session record");
        id
    }

    /// `session/resume` reconstructs the in-memory llama session from a durable
    /// record and returns — it must NOT replay history to the client.
    #[tokio::test]
    #[serial]
    async fn resume_restores_session_without_replay() {
        let _state = with_temp_state();
        let (server, mut rx) = build_server().await;
        let id = persist_record_with_exchange();

        let response = server
            .resume_session(ResumeSessionRequest::new(
                SessionId::new(id.clone()),
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("resume should succeed for a persisted record");

        // The resume response carries no replayed conversation.
        assert!(response.modes.is_none());

        // The in-memory llama session must now hold the restored conversation
        // so the next `session/prompt` can continue it.
        let llama_id: LlamaSessionId = id.parse().expect("record id is a llama ULID");
        let restored = server
            .agent_server()
            .session_manager()
            .get_session(&llama_id)
            .await
            .expect("session lookup")
            .expect("resumed session must be in the session manager");
        assert_eq!(restored.messages.len(), 2);
        assert_eq!(restored.messages[0].role, MessageRole::User);
        assert_eq!(restored.messages[0].content, "remember the number is 42");
        assert_eq!(restored.messages[1].role, MessageRole::Assistant);

        // No `session/update` notifications were emitted — resume does not replay.
        assert!(
            rx.try_recv().is_err(),
            "session/resume must not replay history to the client"
        );
    }

    /// `session/resume` for an id with no persisted record fails with an error
    /// — the opaque id is never rejected on format, only on absence.
    #[tokio::test]
    #[serial]
    async fn resume_missing_record_errors() {
        let _state = with_temp_state();
        let (server, _rx) = build_server().await;

        let id = LlamaSessionId::new().to_string();
        let result = server
            .resume_session(ResumeSessionRequest::new(
                SessionId::new(id),
                PathBuf::from("/tmp"),
            ))
            .await;

        assert!(
            result.is_err(),
            "resuming a session with no persisted record must error"
        );
    }

    /// `session/load` restores the session AND replays the recorded
    /// conversation to the client as `session/update` notifications.
    #[tokio::test]
    #[serial]
    async fn load_restores_and_replays_history() {
        let _state = with_temp_state();
        let (server, mut rx) = build_server().await;
        let id = persist_record_with_exchange();

        server
            .load_session(LoadSessionRequest::new(
                SessionId::new(id.clone()),
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("load should succeed for a persisted record");

        // The llama session is restored, exactly as for `session/resume`.
        let llama_id: LlamaSessionId = id.parse().expect("record id is a llama ULID");
        let restored = server
            .agent_server()
            .session_manager()
            .get_session(&llama_id)
            .await
            .expect("session lookup")
            .expect("loaded session must be in the session manager");
        assert_eq!(restored.messages.len(), 2);

        // Unlike resume, load replays the recorded conversation: two updates
        // (a user chunk, then an agent chunk) reach the client.
        let first = rx.try_recv().expect("first replayed update");
        assert_eq!(first.session_id.0.as_ref(), id.as_str());
        assert!(matches!(first.update, SessionUpdate::UserMessageChunk(_)));

        let second = rx.try_recv().expect("second replayed update");
        assert!(matches!(second.update, SessionUpdate::AgentMessageChunk(_)));

        assert!(
            rx.try_recv().is_err(),
            "load should replay exactly the two recorded updates"
        );

        // Each replayed notification carries `_meta` tagging it as historical
        // replay — identical to claude-agent — so a client can distinguish
        // replayed history from live updates. The shape matches claude's
        // `build_replay_notification`: `message_type`, `message_index`,
        // `total_messages`.
        for (index, notification) in [&first, &second].into_iter().enumerate() {
            let meta = notification
                .meta
                .as_ref()
                .expect("replayed notification must carry historical-replay _meta");
            assert_eq!(
                meta.get("message_type").and_then(|v| v.as_str()),
                Some("historical_replay"),
                "replayed notification must be tagged historical_replay"
            );
            assert_eq!(
                meta.get("message_index").and_then(|v| v.as_u64()),
                Some(index as u64),
                "replayed notification must carry its message_index"
            );
            assert_eq!(
                meta.get("total_messages").and_then(|v| v.as_u64()),
                Some(2),
                "replayed notification must carry the total message count"
            );
        }
    }

    /// `session/load` for an id with no persisted record fails with an error.
    #[tokio::test]
    #[serial]
    async fn load_missing_record_errors() {
        let _state = with_temp_state();
        let (server, _rx) = build_server().await;

        let id = LlamaSessionId::new().to_string();
        let result = server
            .load_session(LoadSessionRequest::new(
                SessionId::new(id),
                PathBuf::from("/tmp"),
            ))
            .await;

        assert!(
            result.is_err(),
            "loading a session with no persisted record must error"
        );
    }

    /// `initialize` advertises the `session/resume` capability so clients know
    /// the agent supports resuming sessions.
    #[tokio::test]
    #[serial]
    async fn initialize_advertises_resume_capability() {
        let _state = with_temp_state();
        let (server, _rx) = build_server().await;

        let response = server
            .initialize(agent_client_protocol::schema::InitializeRequest::new(
                agent_client_protocol::schema::ProtocolVersion::V1,
            ))
            .await
            .expect("initialize");

        assert!(
            response
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some(),
            "agent must advertise the session/resume capability"
        );
    }

    /// `session/set_mode` emits a `CurrentModeUpdate` notification on every
    /// successful call — parity with claude-agent, which also emits
    /// `CurrentModeUpdate` whenever the mode is successfully set.
    #[tokio::test]
    #[serial]
    async fn set_session_mode_emits_current_mode_update() {
        use agent_client_protocol::schema::{
            NewSessionRequest, SessionModeId, SetSessionModeRequest,
        };

        let _state = with_temp_state();
        let (server, mut rx) = build_server().await;

        let session = server
            .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
            .await
            .expect("new_session");

        // Drain any notifications emitted by session creation so the assertion
        // observes only the mode-change notification.
        while rx.try_recv().is_ok() {}

        server
            .set_session_mode(SetSessionModeRequest::new(
                session.session_id.clone(),
                SessionModeId::new("planning"),
            ))
            .await
            .expect("set_session_mode");

        let notification = rx
            .try_recv()
            .expect("set_session_mode must emit a notification");
        assert_eq!(notification.session_id, session.session_id);
        match notification.update {
            SessionUpdate::CurrentModeUpdate(update) => {
                assert_eq!(
                    update.current_mode_id.0.as_ref(),
                    "planning",
                    "CurrentModeUpdate must carry the newly set mode id"
                );
            }
            other => panic!("expected CurrentModeUpdate, got {other:?}"),
        }
    }

    /// `session/cancel` emits a final status update on the notification stream
    /// — parity with claude-agent, so a client cancelling a turn observes the
    /// same cancellation notification from both agents.
    #[tokio::test]
    #[serial]
    async fn cancel_emits_final_status_update() {
        use agent_client_protocol::schema::{CancelNotification, NewSessionRequest};

        let _state = with_temp_state();
        let (server, mut rx) = build_server().await;

        let session = server
            .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
            .await
            .expect("new_session");

        // Drain any notifications emitted by session creation.
        while rx.try_recv().is_ok() {}

        server
            .cancel(CancelNotification::new(session.session_id.clone()))
            .await
            .expect("cancel");

        let notification = rx
            .try_recv()
            .expect("cancel must emit a final status update");
        assert_eq!(notification.session_id, session.session_id);

        // The final update is an AgentMessageChunk tagged as a cancellation —
        // identical in shape to claude-agent's `send_final_cancellation_updates`.
        assert!(
            matches!(notification.update, SessionUpdate::AgentMessageChunk(_)),
            "cancellation final update must be an AgentMessageChunk"
        );
        let meta = notification
            .meta
            .as_ref()
            .expect("cancellation notification must carry _meta");
        assert_eq!(
            meta.get("final_update").and_then(|v| v.as_bool()),
            Some(true),
            "cancellation notification must be tagged final_update"
        );
        assert_eq!(
            meta.get("cancellation").and_then(|v| v.as_bool()),
            Some(true),
            "cancellation notification must be tagged cancellation"
        );
    }
}
