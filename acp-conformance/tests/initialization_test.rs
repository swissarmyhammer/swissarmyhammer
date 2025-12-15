//! Integration tests for ACP initialization protocol
//!
//! These tests verify that both llama-agent and claude-agent correctly implement
//! the ACP initialization protocol per https://agentclientprotocol.com/protocol/initialization

use acp_conformance::initialization::*;
use agent_client_protocol::{Agent, Implementation, InitializeRequest};
use std::env;
use std::path::PathBuf;

/// Helper to get the llama-agent binary path from environment or default
fn get_llama_agent_path() -> Option<PathBuf> {
    env::var("LLAMA_AGENT_PATH")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Try workspace target directory
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // Go to workspace root
            path.push("target/debug/llama-agent");
            if path.exists() {
                Some(path)
            } else {
                None
            }
        })
}

/// Helper to get the claude-agent binary path from environment or default
fn get_claude_agent_path() -> Option<PathBuf> {
    env::var("CLAUDE_AGENT_PATH")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Try workspace claude-agent directory
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // Go to workspace root
            path.push("claude-agent/target/debug/claude-agent");
            if path.exists() {
                Some(path)
            } else {
                None
            }
        })
}

/// Mock agent for testing without requiring real binary
struct MockAgent {
    capabilities: agent_client_protocol::AgentCapabilities,
}

impl MockAgent {
    fn new() -> Self {
        let prompt_caps = agent_client_protocol::PromptCapabilities::new()
            .image(true)
            .audio(true)
            .embedded_context(true);

        let mcp_caps = agent_client_protocol::McpCapabilities::new()
            .http(true)
            .sse(false);

        let capabilities = agent_client_protocol::AgentCapabilities::new()
            .load_session(true)
            .prompt_capabilities(prompt_caps)
            .mcp_capabilities(mcp_caps);

        Self { capabilities }
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for MockAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::InitializeResponse> {
        // Simple mock: echo back the protocol version
        Ok(
            agent_client_protocol::InitializeResponse::new(request.protocol_version)
                .agent_capabilities(self.capabilities.clone())
                .auth_methods(vec![])
                .agent_info(Implementation::new("mock-agent", "1.0.0")),
        )
    }

    async fn authenticate(
        &self,
        _request: agent_client_protocol::AuthenticateRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::AuthenticateResponse> {
        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn new_session(
        &self,
        _request: agent_client_protocol::NewSessionRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::NewSessionResponse> {
        unimplemented!("new_session not needed for initialization tests")
    }

    async fn load_session(
        &self,
        _request: agent_client_protocol::LoadSessionRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::LoadSessionResponse> {
        unimplemented!("load_session not needed for initialization tests")
    }

    async fn set_session_mode(
        &self,
        _request: agent_client_protocol::SetSessionModeRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::SetSessionModeResponse> {
        unimplemented!("set_session_mode not needed for initialization tests")
    }

    async fn prompt(
        &self,
        _request: agent_client_protocol::PromptRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::PromptResponse> {
        unimplemented!("prompt not needed for initialization tests")
    }

    async fn cancel(
        &self,
        _notification: agent_client_protocol::CancelNotification,
    ) -> agent_client_protocol::Result<()> {
        unimplemented!("cancel not needed for initialization tests")
    }

    async fn ext_method(
        &self,
        _request: agent_client_protocol::ExtRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::ExtResponse> {
        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn ext_notification(
        &self,
        _notification: agent_client_protocol::ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        Ok(())
    }
}

#[test_log::test(tokio::test)]
async fn test_mock_agent_minimal_initialization() {
    let agent = MockAgent::new();
    test_minimal_initialization(&agent)
        .await
        .expect("Minimal initialization should succeed");
}

#[test_log::test(tokio::test)]
async fn test_mock_agent_full_capabilities() {
    let agent = MockAgent::new();
    test_full_capabilities_initialization(&agent)
        .await
        .expect("Full capabilities initialization should succeed");
}

#[test_log::test(tokio::test)]
async fn test_mock_agent_protocol_version() {
    let agent = MockAgent::new();
    test_protocol_version_negotiation(&agent)
        .await
        .expect("Protocol version negotiation should succeed");
}

#[test_log::test(tokio::test)]
async fn test_mock_agent_minimal_client_caps() {
    let agent = MockAgent::new();
    test_minimal_client_capabilities(&agent)
        .await
        .expect("Minimal client capabilities should succeed");
}

// Real agent tests - these require the agent binaries to be built

#[test_log::test(tokio::test)]
#[ignore = "Requires llama-agent binary to be built"]
async fn test_llama_agent_initialization() {
    let _agent_path = get_llama_agent_path()
        .expect("LLAMA_AGENT_PATH not set and binary not found in target/debug");

    // TODO: Spawn agent using TestClient and run tests
    // This requires implementing TestClient::spawn to work with the agent binary
    tracing::warn!("Test not yet implemented - need to spawn agent process");
}

#[test_log::test(tokio::test)]
#[ignore = "Requires claude-agent binary to be built"]
async fn test_claude_agent_initialization() {
    let _agent_path = get_claude_agent_path()
        .expect("CLAUDE_AGENT_PATH not set and binary not found in claude-agent/target/debug");

    // TODO: Spawn agent using TestClient and run tests
    tracing::warn!("Test not yet implemented - need to spawn agent process");
}
