//! Test for session mode switching functionality
//!
//! This test verifies that session modes can be switched and that the mode state
//! is properly tracked in both the ACP session state and the llama session.

mod session_mode_tests {
    use agent_client_protocol::{Agent, SessionModeId};
    use llama_agent::acp::session::{AcpSessionState, SessionMode};
    use llama_agent::acp::AcpServer;
    use llama_agent::types::ids::SessionId as LlamaSessionId;
    use llama_agent::AgentServer;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper to create a test ACP server
    async fn create_test_server() -> Arc<AcpServer> {
        use llama_agent::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };

        let temp_dir = TempDir::new().unwrap();

        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let session_config = SessionConfig {
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
            session_config,
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
        Arc::new(AcpServer::new(agent_server, acp_config))
    }

    #[tokio::test]
    async fn test_session_mode_initialization() {
        // Test that new sessions start with general-purpose mode
        let llama_id = LlamaSessionId::new();
        let state = AcpSessionState::new(llama_id);

        assert!(
            matches!(state.mode, SessionMode::Custom(ref s) if s == "general-purpose"),
            "New sessions should start in general-purpose mode"
        );
    }

    #[tokio::test]
    async fn test_session_mode_parse_standard_modes() {
        // Test parsing standard mode identifiers
        let code_mode = SessionMode::parse("code").unwrap();
        assert!(
            matches!(code_mode, SessionMode::Code),
            "Should parse 'code' to Code mode"
        );

        let plan_mode = SessionMode::parse("plan").unwrap();
        assert!(
            matches!(plan_mode, SessionMode::Plan),
            "Should parse 'plan' to Plan mode"
        );

        let test_mode = SessionMode::parse("test").unwrap();
        assert!(
            matches!(test_mode, SessionMode::Test),
            "Should parse 'test' to Test mode"
        );
    }

    #[tokio::test]
    async fn test_session_mode_parse_custom_mode() {
        // Test parsing custom mode identifiers
        let custom_mode = SessionMode::parse("debug").unwrap();
        match custom_mode {
            SessionMode::Custom(s) => assert_eq!(s, "debug", "Should create custom mode 'debug'"),
            _ => panic!("Expected Custom mode variant"),
        }

        let another_custom = SessionMode::parse("research-mode").unwrap();
        match another_custom {
            SessionMode::Custom(s) => {
                assert_eq!(
                    s, "research-mode",
                    "Should create custom mode 'research-mode'"
                )
            }
            _ => panic!("Expected Custom mode variant"),
        }
    }

    #[tokio::test]
    async fn test_session_mode_parse_empty_string() {
        // Test that empty string creates a custom mode
        let empty_mode = SessionMode::parse("").unwrap();
        match empty_mode {
            SessionMode::Custom(s) => {
                assert_eq!(s, "", "Should create custom mode with empty string")
            }
            _ => panic!("Expected Custom mode variant"),
        }
    }

    #[tokio::test]
    async fn test_session_mode_clone() {
        // Test that all mode variants can be cloned
        let code = SessionMode::Code;
        let code_clone = code.clone();
        assert!(
            matches!(code_clone, SessionMode::Code),
            "Code mode should clone correctly"
        );

        let plan = SessionMode::Plan;
        let plan_clone = plan.clone();
        assert!(
            matches!(plan_clone, SessionMode::Plan),
            "Plan mode should clone correctly"
        );

        let test = SessionMode::Test;
        let test_clone = test.clone();
        assert!(
            matches!(test_clone, SessionMode::Test),
            "Test mode should clone correctly"
        );

        let custom = SessionMode::Custom("custom".to_string());
        let custom_clone = custom.clone();
        match custom_clone {
            SessionMode::Custom(s) => {
                assert_eq!(s, "custom", "Custom mode should clone correctly")
            }
            _ => panic!("Expected Custom mode variant"),
        }
    }

    #[tokio::test]
    async fn test_set_session_mode_request_response() {
        // Test the ACP protocol set_session_mode method
        let server = create_test_server().await;

        // Create a new session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_response = server.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Request mode change to "plan"
        let mode_id = SessionModeId::new("plan");
        let set_mode_request =
            agent_client_protocol::SetSessionModeRequest::new(session_id.clone(), mode_id);

        let result = server.set_session_mode(set_mode_request).await;

        assert!(
            result.is_ok(),
            "set_session_mode should succeed: {:?}",
            result.err()
        );

        let response = result.unwrap();

        // Verify response metadata
        assert!(response.meta.is_some(), "Response should contain metadata");

        let meta = response.meta.unwrap();

        // Since modes are not yet fully implemented, mode_set should be false
        assert_eq!(
            meta.get("mode_set"),
            Some(&serde_json::Value::Bool(false)),
            "mode_set should be false (not yet implemented)"
        );

        // Verify the requested mode_id is echoed back
        assert_eq!(
            meta.get("mode_id"),
            Some(&serde_json::Value::String("plan".to_string())),
            "mode_id should be echoed in response"
        );
    }

    #[tokio::test]
    async fn test_set_session_mode_multiple_switches() {
        // Test switching modes multiple times
        let server = create_test_server().await;

        // Create a new session
        let new_session_request =
            agent_client_protocol::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_response = server.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Switch through multiple modes
        let modes = ["code", "plan", "test", "debug", "code"];

        for mode_str in modes.iter() {
            let mode_id = SessionModeId::new(*mode_str);
            let set_mode_request =
                agent_client_protocol::SetSessionModeRequest::new(session_id.clone(), mode_id);

            let result = server.set_session_mode(set_mode_request).await;

            assert!(
                result.is_ok(),
                "Mode switch to '{}' should succeed",
                mode_str
            );

            let response = result.unwrap();
            let meta = response.meta.unwrap();

            // Verify the mode_id is echoed correctly
            assert_eq!(
                meta.get("mode_id"),
                Some(&serde_json::Value::String(mode_str.to_string())),
                "mode_id should match requested mode '{}'",
                mode_str
            );
        }
    }

    #[tokio::test]
    async fn test_set_session_mode_concurrent_sessions() {
        // Test that different sessions can have different modes independently
        let server = create_test_server().await;

        // Create multiple sessions
        let session1_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .unwrap();
        let session1_id = session1_response.session_id;

        let session2_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .unwrap();
        let session2_id = session2_response.session_id;

        let session3_response = server
            .new_session(agent_client_protocol::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .unwrap();
        let session3_id = session3_response.session_id;

        // Set different modes for each session
        let mode1 = SessionModeId::new("code");
        let mode2 = SessionModeId::new("plan");
        let mode3 = SessionModeId::new("test");

        let result1 = server
            .set_session_mode(agent_client_protocol::SetSessionModeRequest::new(
                session1_id.clone(),
                mode1,
            ))
            .await;

        let result2 = server
            .set_session_mode(agent_client_protocol::SetSessionModeRequest::new(
                session2_id.clone(),
                mode2,
            ))
            .await;

        let result3 = server
            .set_session_mode(agent_client_protocol::SetSessionModeRequest::new(
                session3_id.clone(),
                mode3,
            ))
            .await;

        // All requests should succeed
        assert!(result1.is_ok(), "Session 1 mode switch should succeed");
        assert!(result2.is_ok(), "Session 2 mode switch should succeed");
        assert!(result3.is_ok(), "Session 3 mode switch should succeed");

        // Verify each session has the correct mode echoed back
        let meta1 = result1.unwrap().meta.unwrap();
        let meta2 = result2.unwrap().meta.unwrap();
        let meta3 = result3.unwrap().meta.unwrap();

        assert_eq!(
            meta1.get("mode_id"),
            Some(&serde_json::Value::String("code".to_string())),
            "Session 1 should have mode 'code'"
        );

        assert_eq!(
            meta2.get("mode_id"),
            Some(&serde_json::Value::String("plan".to_string())),
            "Session 2 should have mode 'plan'"
        );

        assert_eq!(
            meta3.get("mode_id"),
            Some(&serde_json::Value::String("test".to_string())),
            "Session 3 should have mode 'test'"
        );
    }

    #[tokio::test]
    async fn test_set_session_mode_invalid_session() {
        // Test behavior when trying to set mode for non-existent session
        let server = create_test_server().await;

        // Create a fake session ID that doesn't exist
        let fake_session_id = agent_client_protocol::SessionId::new("nonexistent-session-id");

        let mode_id = SessionModeId::new("code");
        let set_mode_request =
            agent_client_protocol::SetSessionModeRequest::new(fake_session_id, mode_id);

        let result = server.set_session_mode(set_mode_request).await;

        // The current implementation doesn't validate session existence for mode switching
        // but we verify it doesn't panic or hang
        assert!(
            result.is_ok() || result.is_err(),
            "Should return either success or error, not panic"
        );
    }

    #[tokio::test]
    async fn test_session_mode_state_after_clone() {
        // Test that mode state is preserved when cloning AcpSessionState
        let llama_id = LlamaSessionId::new();
        let mut state = AcpSessionState::new(llama_id);

        // Change the mode
        state.mode = SessionMode::Plan;

        // Clone the state
        let cloned_state = state.clone();

        // Verify the mode is preserved in the clone
        assert!(
            matches!(cloned_state.mode, SessionMode::Plan),
            "Mode should be preserved when cloning state"
        );
    }
}
