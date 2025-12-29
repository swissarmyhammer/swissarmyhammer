//! Test protocol version negotiation for ACP server
//!
//! These tests verify that the ACP server correctly negotiates protocol versions
//! according to the ACP specification:
//! - V0 and V1 are both supported
//! - Client's requested version is used if supported
//! - Agent responds with the requested version in InitializeResponse

#[cfg(feature = "acp")]
mod version_negotiation_tests {
    use agent_client_protocol::{Agent, ProtocolVersion};
    use llama_agent::acp::AcpServer;
    use llama_agent::types::{AgentConfig, ModelConfig, ModelSource};
    use llama_agent::{AgentAPI, AgentServer};
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper to create a minimal InitializeRequest for testing
    fn create_initialize_request(
        protocol_version: ProtocolVersion,
    ) -> agent_client_protocol::InitializeRequest {
        agent_client_protocol::InitializeRequest::new(protocol_version).client_capabilities(
            agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true),
        )
    }

    async fn create_test_server() -> Result<Arc<AcpServer>, Box<dyn std::error::Error>> {
        // Initialize tracing for test visibility
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        // Create a temporary directory for the test model
        let temp_dir = TempDir::new()?;

        // Create minimal agent config for testing
        let agent_config = AgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
                    filename: None,
                },
                batch_size: 512,
                n_seq_max: 1,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: llama_agent::types::RetryConfig::default(),
                debug: false,
            },
            ..Default::default()
        };

        // Create agent server - this will fail without an actual model
        // but that's OK for protocol testing
        let agent_server = Arc::new(AgentServer::initialize(agent_config).await?);

        // Create ACP server
        let acp_config = llama_agent::acp::config::AcpConfig::default();
        Ok(Arc::new(AcpServer::new(agent_server, acp_config)))
    }

    /// Test that V0 protocol version is accepted and echoed back
    #[tokio::test]
    #[ignore = "Requires valid model for AgentServer initialization"]
    async fn test_version_negotiation_v0() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        // Client requests V0
        let request = create_initialize_request(ProtocolVersion::V0);

        let response = server
            .initialize(request)
            .await
            .expect("Initialize should succeed with V0");

        // Agent should respond with V0 (client's requested version)
        assert_eq!(
            response.protocol_version,
            ProtocolVersion::V0,
            "Agent should respond with client's requested V0 version"
        );
    }

    /// Test that V1 protocol version is accepted and echoed back
    #[tokio::test]
    #[ignore = "Requires valid model for AgentServer initialization"]
    async fn test_version_negotiation_v1() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        // Client requests V1
        let request = create_initialize_request(ProtocolVersion::V1);

        let response = server
            .initialize(request)
            .await
            .expect("Initialize should succeed with V1");

        // Agent should respond with V1 (client's requested version)
        assert_eq!(
            response.protocol_version,
            ProtocolVersion::V1,
            "Agent should respond with client's requested V1 version"
        );
    }

    /// Test comprehensive version negotiation for both V0 and V1
    #[tokio::test]
    #[ignore = "Requires valid model for AgentServer initialization"]
    async fn test_version_negotiation_comprehensive() {
        let server = create_test_server()
            .await
            .expect("Failed to create test server");

        // Test V0
        let v0_request = create_initialize_request(ProtocolVersion::V0);
        let v0_response = server
            .initialize(v0_request)
            .await
            .expect("V0 should be supported");
        assert_eq!(v0_response.protocol_version, ProtocolVersion::V0);

        // Test V1
        let v1_request = create_initialize_request(ProtocolVersion::V1);
        let v1_response = server
            .initialize(v1_request)
            .await
            .expect("V1 should be supported");
        assert_eq!(v1_response.protocol_version, ProtocolVersion::V1);

        // Both versions should be supported for backward and forward compatibility
    }
}
