//! Initialization protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/initialization
//!
//! ## Requirements Tested
//!
//! 1. **Protocol Version Negotiation**
//!    - Client sends latest supported version
//!    - Agent responds with compatible version
//!    - Version must be a single integer
//!
//! 2. **Client Capabilities**
//!    - `fs.readTextFile` / `fs.writeTextFile`: File system access (optional)
//!    - `terminal`: Shell command execution (optional)
//!    - All omitted capabilities treated as unsupported
//!
//! 3. **Agent Capabilities**
//!    - `loadSession`: Session loading support (optional)
//!    - `promptCapabilities`: Content types - image, audio, embeddedContext (optional)
//!    - `mcp`: MCP transport - http, sse (optional)
//!    - All omitted capabilities treated as unsupported
//!
//! 4. **Authentication Methods**
//!    - Agent must return array of auth methods (required, may be empty)
//!
//! 5. **Implementation Info**
//!    - clientInfo / agentInfo recommended but optional
//!    - Must include name, version if present

use agent_client_protocol::{
    Agent, ClientCapabilities, FileSystemCapability, InitializeRequest, ProtocolVersion,
};
use agent_client_protocol_extras::recording::RecordedSession;

/// Statistics from initialization fixture verification
#[derive(Debug, Default)]
pub struct InitializationStats {
    pub initialize_calls: usize,
    pub protocol_version: Option<i64>,
    pub has_agent_info: bool,
    pub has_agent_capabilities: bool,
}

/// Test basic initialization with minimal capabilities
pub async fn test_minimal_initialization<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing minimal initialization");

    // Create minimal request
    let request = InitializeRequest::new(ProtocolVersion::V1);

    // Send initialize request
    let response = agent.initialize(request).await?;

    // Validate response has required fields
    validate_protocol_version(&response.protocol_version, ProtocolVersion::V1)?;
    validate_agent_capabilities_present(&response.agent_capabilities)?;
    validate_auth_methods(&response.auth_methods)?;
    validate_agent_info(&response.agent_info)?;

    Ok(())
}

/// Test initialization with full client capabilities
pub async fn test_full_capabilities_initialization<A: Agent + ?Sized>(
    agent: &A,
) -> crate::Result<()> {
    tracing::info!("Testing initialization with full client capabilities");

    // Create request with all client capabilities
    let client_caps = ClientCapabilities::new()
        .fs(FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);

    let request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);

    // Send initialize request
    let response = agent.initialize(request).await?;

    // Validate protocol version
    validate_protocol_version(&response.protocol_version, ProtocolVersion::V1)?;

    // Log agent capabilities
    log_agent_capabilities(&response.agent_capabilities);

    Ok(())
}

/// Test that agent handles missing optional fields gracefully
pub async fn test_minimal_client_capabilities<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing with minimal client capabilities (none declared)");

    // Create request with no client capabilities
    let request = InitializeRequest::new(ProtocolVersion::V1);

    // Send initialize request
    let _response = agent.initialize(request).await?;

    // Agent should not assume any client capabilities
    // This test just verifies the agent doesn't crash or error

    tracing::info!("Agent handled missing client capabilities correctly");

    Ok(())
}

/// Test protocol version negotiation
pub async fn test_protocol_version_negotiation<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing protocol version negotiation");

    // Test with V1
    let request = InitializeRequest::new(ProtocolVersion::V1);
    let response = agent.initialize(request).await?;

    validate_protocol_version(&response.protocol_version, ProtocolVersion::V1)?;

    tracing::info!("Protocol version V1 negotiated successfully");

    Ok(())
}

/// Test that initialize can be called multiple times (idempotent)
pub async fn test_initialize_idempotent<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing initialize idempotency");

    let request = InitializeRequest::new(ProtocolVersion::V1);

    // First initialize
    let response1 = agent.initialize(request.clone()).await?;

    // Second initialize - should work the same way
    let response2 = agent.initialize(request).await?;

    // Responses should be consistent
    if response1.protocol_version != response2.protocol_version {
        return Err(crate::Error::Validation(
            "Protocol version changed between initialize calls".to_string(),
        ));
    }

    tracing::info!("Initialize is idempotent");

    Ok(())
}

/// Test initialization with client info
pub async fn test_with_client_info<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing initialization with client info");

    let client_info = agent_client_protocol::Implementation::new("test-client", "1.0.0")
        .title("Test Client for Conformance");

    let request = InitializeRequest::new(ProtocolVersion::V1).client_info(client_info);

    let response = agent.initialize(request).await?;

    // Agent should accept client info without error
    validate_protocol_version(&response.protocol_version, ProtocolVersion::V1)?;

    tracing::info!("Agent accepted client info");

    Ok(())
}

// Validation helper functions

fn validate_protocol_version(
    actual: &ProtocolVersion,
    expected: ProtocolVersion,
) -> crate::Result<()> {
    if *actual != expected {
        return Err(crate::Error::Validation(format!(
            "Expected protocol version {:?}, got {:?}",
            expected, actual
        )));
    }
    Ok(())
}

fn validate_agent_capabilities_present(
    caps: &agent_client_protocol::AgentCapabilities,
) -> crate::Result<()> {
    // Agent capabilities are always present (required struct)
    tracing::debug!("Agent capabilities: {:?}", caps);
    Ok(())
}

fn validate_auth_methods(methods: &[agent_client_protocol::AuthMethod]) -> crate::Result<()> {
    // Auth methods must be present (may be empty)
    if methods.is_empty() {
        tracing::info!("Agent declares no authentication methods");
    } else {
        tracing::info!("Agent declares {} auth method(s)", methods.len());
        for method in methods {
            tracing::info!("  - Auth method: {:?}", method.id);
        }
    }
    Ok(())
}

fn validate_agent_info(info: &Option<agent_client_protocol::Implementation>) -> crate::Result<()> {
    // Agent info is recommended
    if let Some(ref info) = info {
        tracing::info!("Agent info: {} version {}", info.name, info.version);

        // Validate name is not empty
        if info.name.is_empty() {
            return Err(crate::Error::Validation(
                "Agent info name must not be empty".to_string(),
            ));
        }

        // Validate version is not empty
        if info.version.is_empty() {
            return Err(crate::Error::Validation(
                "Agent info version must not be empty".to_string(),
            ));
        }
    } else {
        tracing::warn!("Agent info not provided (recommended but optional)");
    }
    Ok(())
}

fn log_agent_capabilities(caps: &agent_client_protocol::AgentCapabilities) {
    tracing::info!("Agent capabilities:");

    // Check capabilities
    if caps.load_session {
        tracing::info!("  ✓ Session loading supported");
    } else {
        tracing::info!("  ✗ Session loading not supported");
    }

    // Prompt capabilities
    let prompt_caps = &caps.prompt_capabilities;
    tracing::info!("  Prompt capabilities:");
    if prompt_caps.image {
        tracing::info!("    ✓ Image content");
    }
    if prompt_caps.audio {
        tracing::info!("    ✓ Audio content");
    }
    if prompt_caps.embedded_context {
        tracing::info!("    ✓ Embedded context");
    }

    // MCP capabilities
    let mcp_caps = &caps.mcp_capabilities;
    tracing::info!("  MCP capabilities:");
    if mcp_caps.http {
        tracing::info!("    ✓ HTTP transport");
    }
    if mcp_caps.sse {
        tracing::info!("    ✓ SSE transport");
    }
}

/// Validate initialization response structure
pub fn validate_initialization_response(
    response: &agent_client_protocol::InitializeResponse,
) -> crate::Result<()> {
    // Protocol version is required
    tracing::debug!("Protocol version: {:?}", response.protocol_version);

    // Agent capabilities are always present (required struct)
    tracing::debug!("Agent capabilities: {:?}", response.agent_capabilities);

    // Auth methods array is required (but may be empty)
    tracing::debug!("Auth methods: {:?}", response.auth_methods);

    // Agent info is optional but recommended
    if response.agent_info.is_none() {
        tracing::debug!("Agent info not provided (recommended but optional)");
    }

    Ok(())
}

/// Verify initialization fixture has proper recordings
///
/// This function reads the fixture and verifies:
/// 1. The fixture has recorded calls (not calls: [])
/// 2. An initialize method call was recorded
/// 3. Response has required fields (protocol_version, agent_capabilities, auth_methods)
pub fn verify_initialization_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<InitializationStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = InitializationStats::default();

    // CRITICAL: Verify we have calls recorded (catches poor tests with calls: [])
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: [] - test didn't call agent properly"
    );

    for call in &session.calls {
        if call.method == "initialize" {
            stats.initialize_calls += 1;

            // Check response has required fields
            let response_json = &call.response;

            // Validate protocol_version
            if let Some(version) = response_json
                .get("protocolVersion")
                .and_then(|v| v.as_i64())
            {
                stats.protocol_version = Some(version);
            }

            // Validate agent_capabilities
            if response_json.get("agentCapabilities").is_some() {
                stats.has_agent_capabilities = true;
            }

            // Validate auth_methods (required, may be empty)
            assert!(
                response_json.get("authMethods").is_some(),
                "Initialize response missing required 'authMethods' field"
            );

            // Check for agent_info (optional but recommended)
            if response_json.get("agentInfo").is_some() {
                stats.has_agent_info = true;
            }
        }
    }

    tracing::info!("{} initialization fixture stats: {:?}", agent_type, stats);

    // At least one initialize call
    assert!(
        stats.initialize_calls > 0,
        "Expected at least one initialize call, got {}",
        stats.initialize_calls
    );

    // Protocol version should be set
    assert!(
        stats.protocol_version.is_some(),
        "Expected protocolVersion in response"
    );

    // Agent capabilities should be present
    assert!(
        stats.has_agent_capabilities,
        "Expected agentCapabilities in response"
    );

    Ok(stats)
}

#[cfg(test)]
mod tests {
    /// Dummy test to verify module compiles
    #[test]
    fn test_module_compiles() {
        // This ensures the module compiles correctly
    }
}
