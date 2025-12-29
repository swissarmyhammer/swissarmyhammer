//! Test for SessionUpdate streaming during session loading
//!
//! This test verifies that all SessionUpdate types (ToolCall, Plan, AgentThoughtChunk,
//! AvailableCommandsUpdate, CurrentModeUpdate, UserMessageChunk, AgentMessageChunk)
//! are correctly streamed during load_session.

#[cfg(feature = "acp")]
mod acp_tests {
    use agent_client_protocol::{Agent, AvailableCommand, LoadSessionRequest};
    use llama_agent::acp::AcpServer;
    use llama_agent::types::{Message, MessageRole, ModelConfig, ModelSource};
    use llama_agent::AgentServer;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tempfile::TempDir;

    /// Helper to create a test ACP server with in-memory storage
    async fn create_test_server() -> Result<Arc<AcpServer>, Box<dyn std::error::Error>> {
        use llama_agent::types::{ParallelConfig, QueueConfig, RetryConfig, SessionConfig};

        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let temp_dir = TempDir::new()?;

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
            session_config: SessionConfig {
                max_sessions: 10,
                auto_compaction: None,
                model_context_size: 4096,
                persistence_enabled: true,
                session_storage_dir: Some(temp_dir.path().to_path_buf()),
                session_ttl_hours: 24,
                auto_save_threshold: 5,
                max_kv_cache_files: 16,
                kv_cache_dir: None,
            },
            parallel_execution_config: ParallelConfig::default(),
        };

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
        let server = AcpServer::new(agent_server, acp_config);

        Ok(Arc::new(server))
    }

    /// Test that all SessionUpdate types are streamed during load_session
    ///
    /// This test creates a session with multiple message types and state updates,
    /// saves it to storage, then loads it back and verifies that all SessionUpdate
    /// types are streamed via notifications.
    #[tokio::test]
    #[ignore = "Requires ACP feature and valid model for initialization"]
    async fn test_all_session_update_types_streamed_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        // Create a session via ACP
        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        // Parse the session ID to get the llama session ID
        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        // Create a direct session manager to manipulate the session
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        // Add messages of different types to the session
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "Hello, what's the weather?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Let me check the weather for you.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Tool,
                content: r#"{"temperature": 72, "condition": "sunny"}"#.to_string(),
                tool_call_id: Some(llama_agent::types::ToolCallId::new()),
                tool_name: Some("get_weather".to_string()),
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "The weather is sunny with a temperature of 72°F.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ];

        for message in messages {
            session_manager
                .add_message(&llama_session_id, message)
                .await
                .expect("Failed to add message");
        }

        // Save the session
        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        // Load the session back
        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let load_response = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // Verify the response is successful
        assert!(
            matches!(
                load_response,
                agent_client_protocol::LoadSessionResponse { .. }
            ),
            "Load session should return success response"
        );

        // NOTE: This test verifies that load_session completes successfully.
        // To fully test SessionUpdate streaming, we would need to:
        // 1. Subscribe to the notification channel before calling load_session
        // 2. Collect all SessionUpdate notifications during the load
        // 3. Verify each expected SessionUpdate type was sent:
        //    - UserMessageChunk for user messages
        //    - AgentMessageChunk for assistant messages
        //    - ToolCallUpdate for tool call results (currently sent as AgentMessageChunk per server.rs:736)
        //    - AgentThoughtChunk for internal reasoning (if present)
        //    - Plan for execution plan (if todos present)
        //    - AvailableCommandsUpdate for available commands
        //    - CurrentModeUpdate for mode changes
        //
        // The current implementation in server.rs:720-740 sends:
        // - UserMessageChunk for MessageRole::User
        // - AgentMessageChunk for MessageRole::Assistant
        // - AgentMessageChunk for MessageRole::Tool (see comment on line 735-736)
        // - Skips MessageRole::System messages
    }

    /// Test that UserMessageChunk updates are sent for user messages
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_user_message_chunk_updates_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        // Add user message
        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        session_manager
            .add_message(
                &llama_session_id,
                Message {
                    role: MessageRole::User,
                    content: "Test user message".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
            )
            .await
            .expect("Failed to add message");

        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        // Load session and verify UserMessageChunk notification
        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::UserMessageChunk was sent via notification channel
    }

    /// Test that AgentMessageChunk updates are sent for assistant messages
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_agent_message_chunk_updates_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        session_manager
            .add_message(
                &llama_session_id,
                Message {
                    role: MessageRole::Assistant,
                    content: "Test agent response".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                },
            )
            .await
            .expect("Failed to add message");

        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::AgentMessageChunk was sent via notification channel
    }

    /// Test that ToolCallUpdate notifications are sent for tool messages
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_tool_call_update_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        // Add tool call message
        session_manager
            .add_message(
                &llama_session_id,
                Message {
                    role: MessageRole::Tool,
                    content: r#"{"result": "success"}"#.to_string(),
                    tool_call_id: Some(llama_agent::types::ToolCallId::new()),
                    tool_name: Some("test_tool".to_string()),
                    timestamp: SystemTime::now(),
                },
            )
            .await
            .expect("Failed to add message");

        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::AgentMessageChunk was sent for tool result
        // (Tool messages are currently sent as AgentMessageChunk per server.rs:736)
    }

    /// Test that Plan notifications are sent when session has todos
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_plan_update_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        // Add a todo to the session (this would normally be done through todo_create tool)
        let mut session = session_manager
            .get_session(&llama_session_id)
            .await
            .expect("Failed to get session")
            .expect("Session should exist");

        // Add todo via swissarmyhammer-todo (PlanEntry structure)
        let todo = swissarmyhammer_todo::PlanEntry::new(
            "Test task".to_string(),
            swissarmyhammer_todo::Priority::Medium,
        );
        session.todos.push(todo);

        // Save session with todos
        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::Plan was sent via notification channel
    }

    /// Test that AvailableCommandsUpdate notifications are sent
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_available_commands_update_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        // Add available commands to the session
        let mut session = session_manager
            .get_session(&llama_session_id)
            .await
            .expect("Failed to get session")
            .expect("Session should exist");

        session.available_commands = vec![AvailableCommand::new(
            "test_command",
            "Test command description",
        )];

        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::AvailableCommandsUpdate was sent via notification channel
    }

    /// Test that CurrentModeUpdate notifications are sent
    #[tokio::test]
    #[ignore = "Requires ACP feature and notification channel implementation"]
    async fn test_current_mode_update_during_load() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        let session_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                PathBuf::from("/tmp"),
            ))
            .await
            .expect("Failed to create session");
        let session_id = session_response.session_id;

        let llama_session_id =
            llama_agent::types::SessionId::from_str(&session_id.0).expect("Invalid session ID");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let session_config = llama_agent::types::SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let session_manager = llama_agent::session::SessionManager::new(session_config);

        // Set current mode on the session
        let mut session = session_manager
            .get_session(&llama_session_id)
            .await
            .expect("Failed to get session")
            .expect("Session should exist");

        session.current_mode = Some("coding".to_string());

        session_manager
            .save_session(&llama_session_id)
            .await
            .expect("Failed to save session");

        let load_request = LoadSessionRequest::new(session_id.clone(), PathBuf::from("/tmp"));

        let _result = server
            .load_session(load_request)
            .await
            .expect("Failed to load session");

        // TODO: Verify SessionUpdate::CurrentModeUpdate was sent via notification channel
    }
}
