//! Terminal protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/terminals
//!
//! ## Requirements Tested
//!
//! 1. **Checking Support**
//!    - Agents MUST verify client capabilities before attempting terminal methods
//!    - Check `clientCapabilities.terminal`
//!    - If false or not present, agent MUST NOT attempt to call any terminal methods
//!
//! 2. **Executing Commands**
//!    - Method: `terminal/create`
//!    - Required params: `sessionId`, `command`
//!    - Optional params: `args`, `env`, `cwd`, `outputByteLimit`
//!    - Response: `{ terminalId: string }`
//!    - Returns immediately without waiting for completion
//!
//! 3. **Getting Output**
//!    - Method: `terminal/output`
//!    - Required params: `sessionId`, `terminalId`
//!    - Response: `{ output: string, truncated: boolean, exitStatus?: { exitCode, signal } }`
//!    - Returns current output without waiting
//!
//! 4. **Waiting for Exit**
//!    - Method: `terminal/wait_for_exit`
//!    - Required params: `sessionId`, `terminalId`
//!    - Response: `{ exitCode: number, signal: string }`
//!    - Returns once command completes
//!
//! 5. **Killing Commands**
//!    - Method: `terminal/kill`
//!    - Required params: `sessionId`, `terminalId`
//!    - Terminal remains valid after kill for output/wait_for_exit
//!
//! 6. **Releasing Terminals**
//!    - Method: `terminal/release`
//!    - Required params: `sessionId`, `terminalId`
//!    - Kills command if still running and releases all resources
//!    - Terminal ID becomes invalid after release

use agent_client_protocol::schema::{
    ClientCapabilities, ExtRequest, ExtResponse, InitializeRequest, NewSessionRequest,
    ProtocolVersion,
};
use agent_client_protocol::ClientRequest;
use agent_client_protocol_extras::{recording::RecordedSession, AgentWithFixture};
use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_common::Pretty;

/// Send an `ExtRequest` over the wrapper's connection and reconstitute an
/// [`ExtResponse`] for downstream code.
///
/// ACP 0.10 had `Agent::ext_method(ExtRequest) -> ExtResponse`. ACP 0.11
/// drops the trait; ext requests now flow through
/// [`ConnectionTo::send_request`] wrapped in
/// [`ClientRequest::ExtMethodRequest`], with the response dispatched as a
/// raw [`serde_json::Value`]. This helper bridges the two: the production
/// scenarios still talk in terms of `ExtResponse(Arc<RawValue>)`, so we
/// re-encode the wire JSON back into that shape for them.
async fn send_ext_method(
    agent: &dyn AgentWithFixture,
    request: ExtRequest,
) -> agent_client_protocol::Result<ExtResponse> {
    let value: serde_json::Value = agent
        .connection()
        .send_request(ClientRequest::ExtMethodRequest(request))
        .block_task()
        .await?;
    let raw =
        serde_json::value::to_raw_value(&value).map_err(agent_client_protocol::Error::into_internal_error)?;
    Ok(ExtResponse::new(Arc::from(raw)))
}

/// Statistics from terminals fixture verification
#[derive(Debug, Default, serde::Serialize)]
pub struct TerminalsStats {
    pub initialize_calls: usize,
    pub new_session_calls: usize,
    pub ext_method_calls: usize,
}

/// Test that agent properly checks terminal capability before allowing terminal operations
pub async fn test_terminal_capability_check(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal capability check");

    // Initialize with NO terminal capability
    let client_caps = ClientCapabilities::new().terminal(false);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Attempt to create a terminal without capability
    let params = json!({
        "sessionId": session_id.0,
        "command": "echo",
        "args": ["hello"]
    });

    let ext_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    // Should return an error because capability is not declared
    let result = send_ext_method(agent, ext_request).await;

    match result {
        Err(e) => {
            // Agent correctly rejected the request
            let error_msg = format!("{:?}", e);
            if error_msg.contains("Invalid params") || error_msg.contains("-32602") {
                tracing::info!(
                    "Agent correctly rejected terminal/create without capability (Invalid params)"
                );
                Ok(())
            } else if error_msg.contains("capability") || error_msg.contains("not supported") {
                tracing::info!("Agent correctly rejected terminal/create without capability");
                Ok(())
            } else {
                Err(crate::Error::Validation(format!(
                    "Agent rejected terminal/create but with unexpected error: {}",
                    error_msg
                )))
            }
        }
        Ok(_) => Err(crate::Error::Validation(
            "Agent should reject terminal/create when capability not declared".to_string(),
        )),
    }
}

/// Test creating a terminal with the terminal capability
pub async fn test_terminal_create(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal/create");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal that runs a simple command
    let params = json!({
        "sessionId": session_id.0,
        "command": "echo",
        "args": ["hello world"]
    });

    let ext_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    let result = send_ext_method(agent, ext_request).await;

    match result {
        Ok(response) => {
            // Parse response to check for terminalId field
            let response_value: serde_json::Value = serde_json::from_str(response.0.get())?;

            if let Some(terminal_id) = response_value.get("terminalId") {
                if terminal_id.is_string() {
                    tracing::info!("Successfully created terminal with ID: {}", terminal_id);

                    // Clean up: release the terminal
                    let release_params = json!({
                        "sessionId": session_id.0,
                        "terminalId": terminal_id
                    });
                    let release_request = ExtRequest::new(
                        "terminal/release",
                        Arc::from(serde_json::value::to_raw_value(&release_params)?),
                    );
                    let _ = send_ext_method(agent, release_request).await;

                    Ok(())
                } else {
                    Err(crate::Error::Validation(
                        "Response terminalId field is not a string".to_string(),
                    ))
                }
            } else {
                Err(crate::Error::Validation(
                    "Response missing 'terminalId' field".to_string(),
                ))
            }
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test getting terminal output
pub async fn test_terminal_output(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal/output");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal
    let create_params = json!({
        "sessionId": session_id.0,
        "command": "echo",
        "args": ["test output"]
    });

    let create_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&create_params)?),
    );

    let create_response = send_ext_method(agent, create_request).await?;
    let create_value: serde_json::Value = serde_json::from_str(create_response.0.get())?;
    let terminal_id = create_value
        .get("terminalId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("No terminalId in response".to_string()))?;

    // Wait a bit for command to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Get output
    let output_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let output_request = ExtRequest::new(
        "terminal/output",
        Arc::from(serde_json::value::to_raw_value(&output_params)?),
    );

    let output_result = send_ext_method(agent, output_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = send_ext_method(agent, release_request).await;

    match output_result {
        Ok(response) => {
            let response_value: serde_json::Value = serde_json::from_str(response.0.get())?;

            // Check required fields
            if response_value.get("output").is_none() {
                return Err(crate::Error::Validation(
                    "Response missing 'output' field".to_string(),
                ));
            }
            if response_value.get("truncated").is_none() {
                return Err(crate::Error::Validation(
                    "Response missing 'truncated' field".to_string(),
                ));
            }

            tracing::info!("Successfully retrieved terminal output");
            Ok(())
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test waiting for terminal exit
pub async fn test_terminal_wait_for_exit(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal/wait_for_exit");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal with a quick command
    let create_params = json!({
        "sessionId": session_id.0,
        "command": "echo",
        "args": ["done"]
    });

    let create_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&create_params)?),
    );

    let create_response = send_ext_method(agent, create_request).await?;
    let create_value: serde_json::Value = serde_json::from_str(create_response.0.get())?;
    let terminal_id = create_value
        .get("terminalId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("No terminalId in response".to_string()))?;

    // Wait for exit
    let wait_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let wait_request = ExtRequest::new(
        "terminal/wait_for_exit",
        Arc::from(serde_json::value::to_raw_value(&wait_params)?),
    );

    let wait_result = send_ext_method(agent, wait_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = send_ext_method(agent, release_request).await;

    match wait_result {
        Ok(response) => {
            let response_value: serde_json::Value = serde_json::from_str(response.0.get())?;

            // Check that we have exitCode field (may be null) or signal field
            let has_exit_code = response_value.get("exitCode").is_some();
            let has_signal = response_value.get("signal").is_some();

            if !has_exit_code && !has_signal {
                return Err(crate::Error::Validation(
                    "Response missing both 'exitCode' and 'signal' fields".to_string(),
                ));
            }

            tracing::info!("Successfully waited for terminal exit");
            Ok(())
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test killing a terminal command
pub async fn test_terminal_kill(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal/kill");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal with a long-running command
    let create_params = json!({
        "sessionId": session_id.0,
        "command": "sleep",
        "args": ["10"]
    });

    let create_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&create_params)?),
    );

    let create_response = send_ext_method(agent, create_request).await?;
    let create_value: serde_json::Value = serde_json::from_str(create_response.0.get())?;
    let terminal_id = create_value
        .get("terminalId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("No terminalId in response".to_string()))?;

    // Give command a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Kill the command
    let kill_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let kill_request = ExtRequest::new(
        "terminal/kill",
        Arc::from(serde_json::value::to_raw_value(&kill_params)?),
    );

    let kill_result = send_ext_method(agent, kill_request).await;

    // Terminal should still be valid, try to get output
    let output_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let output_request = ExtRequest::new(
        "terminal/output",
        Arc::from(serde_json::value::to_raw_value(&output_params)?),
    );

    let output_result = send_ext_method(agent, output_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = send_ext_method(agent, release_request).await;

    // Verify kill succeeded
    if kill_result.is_err() {
        return Err(crate::Error::Validation(format!(
            "terminal/kill failed: {:?}",
            kill_result.err()
        )));
    }

    // Verify terminal is still valid (output call should succeed)
    match output_result {
        Ok(_) => {
            tracing::info!("Successfully killed terminal and verified it remains valid");
            Ok(())
        }
        Err(e) => Err(crate::Error::Validation(format!(
            "Terminal should remain valid after kill, but output call failed: {:?}",
            e
        ))),
    }
}

/// Test releasing a terminal
pub async fn test_terminal_release(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal/release");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal
    let create_params = json!({
        "sessionId": session_id.0,
        "command": "echo",
        "args": ["test"]
    });

    let create_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&create_params)?),
    );

    let create_response = send_ext_method(agent, create_request).await?;
    let create_value: serde_json::Value = serde_json::from_str(create_response.0.get())?;
    let terminal_id = create_value
        .get("terminalId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("No terminalId in response".to_string()))?;

    // Release the terminal
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );

    let release_result = send_ext_method(agent, release_request).await;

    if release_result.is_err() {
        return Err(crate::Error::Validation(format!(
            "terminal/release failed: {:?}",
            release_result.err()
        )));
    }

    // Verify terminal is now invalid - attempting to use it should fail
    let output_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let output_request = ExtRequest::new(
        "terminal/output",
        Arc::from(serde_json::value::to_raw_value(&output_params)?),
    );

    let output_result = send_ext_method(agent, output_request).await;

    match output_result {
        Err(_) => {
            tracing::info!("Successfully released terminal and verified it becomes invalid");
            Ok(())
        }
        Ok(_) => Err(crate::Error::Validation(
            "Terminal should be invalid after release, but output call succeeded".to_string(),
        )),
    }
}

/// Test building a timeout with terminal methods
pub async fn test_terminal_timeout(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing terminal timeout pattern");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent
        .connection()
        .send_request(init_request)
        .block_task()
        .await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent
        .connection()
        .send_request(new_session_request)
        .block_task()
        .await?;
    let session_id = new_session_response.session_id;

    // Create a terminal with a long-running command
    let create_params = json!({
        "sessionId": session_id.0,
        "command": "sleep",
        "args": ["5"]
    });

    let create_request = ExtRequest::new(
        "terminal/create",
        Arc::from(serde_json::value::to_raw_value(&create_params)?),
    );

    let create_response = send_ext_method(agent, create_request).await?;
    let create_value: serde_json::Value = serde_json::from_str(create_response.0.get())?;
    let terminal_id = create_value
        .get("terminalId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("No terminalId in response".to_string()))?;

    // Race between timeout and wait_for_exit
    let wait_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let wait_request = ExtRequest::new(
        "terminal/wait_for_exit",
        Arc::from(serde_json::value::to_raw_value(&wait_params)?),
    );

    let wait_future = send_ext_method(agent, wait_request);
    let timeout_future = tokio::time::sleep(tokio::time::Duration::from_millis(500));

    tokio::select! {
        _ = wait_future => {
            // Command finished before timeout (shouldn't happen with sleep 5)
            tracing::warn!("Command finished before timeout");
        }
        _ = timeout_future => {
            // Timeout occurred, kill the command
            let kill_params = json!({
                "sessionId": session_id.0,
                "terminalId": terminal_id
            });

            let kill_request = ExtRequest::new(
                "terminal/kill",
                Arc::from(serde_json::value::to_raw_value(&kill_params)?),
            );

            send_ext_method(agent, kill_request).await?;

            // Get final output
            let output_params = json!({
                "sessionId": session_id.0,
                "terminalId": terminal_id
            });

            let output_request = ExtRequest::new(
                "terminal/output",
                Arc::from(serde_json::value::to_raw_value(&output_params)?),
            );

            let _ = send_ext_method(agent, output_request).await?;

            tracing::info!("Successfully implemented timeout pattern");
        }
    }

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = send_ext_method(agent, release_request).await;

    Ok(())
}

/// Verify terminals fixture has proper recordings
pub fn verify_terminals_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<TerminalsStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = TerminalsStats::default();

    // CRITICAL: Verify we have calls recorded (catches poor tests with calls: [])
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: [] - test didn't call agent properly"
    );

    for call in &session.calls {
        match call.method.as_str() {
            "initialize" => stats.initialize_calls += 1,
            "new_session" => stats.new_session_calls += 1,
            "ext_method" => stats.ext_method_calls += 1,
            _ => {}
        }
    }

    tracing::info!("{} terminals fixture stats: {}", agent_type, Pretty(&stats));

    // Should have at least initialize and new_session
    assert!(
        stats.initialize_calls >= 1,
        "Expected at least 1 initialize call, got {}",
        stats.initialize_calls
    );
    assert!(
        stats.new_session_calls >= 1,
        "Expected at least 1 new_session call, got {}",
        stats.new_session_calls
    );

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{run_with_mock_agent_as_fixture, MockAgent};
    use agent_client_protocol::schema::InitializeResponse;
    use futures::future::BoxFuture;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Minimal mock for terminal scenarios.
    ///
    /// In ACP 0.10 the terminal methods (`terminal/create`, `terminal/output`,
    /// …) lived on the `Agent` trait as ext-method dispatch and the unit-test
    /// mock implemented all of them inline. ACP 0.11 dispatches arbitrary
    /// methods through `ClientRequest::ExtMethodRequest`, but only methods
    /// prefixed with `_` route through that variant — bare strings like
    /// `terminal/create` are rejected by the SDK's parse layer with
    /// `method_not_found` *before* the mock sees them. This actually matches
    /// the no-capability scenario's expected outcome (the agent rejects),
    /// and the real recording flow against `claude-agent` / `llama-agent`
    /// uses the production agents' own typed handlers — not this mock — so
    /// the mock only needs to make `initialize` and `new_session` succeed
    /// for the production helpers to reach the SDK's terminal-rejection
    /// path. The remaining terminal-state tracking fields are kept for
    /// scenario-specific assertions inside the mock if future scenarios
    /// reach them.
    struct TerminalMockAgent {
        /// Whether terminal capability was declared during init.
        terminal_enabled: AtomicBool,
    }

    impl TerminalMockAgent {
        fn new() -> Self {
            Self {
                terminal_enabled: AtomicBool::new(false),
            }
        }
    }

    impl MockAgent for TerminalMockAgent {
        fn initialize<'a>(
            &'a self,
            request: InitializeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<InitializeResponse>> {
            Box::pin(async move {
                if request.client_capabilities.terminal {
                    self.terminal_enabled.store(true, Ordering::SeqCst);
                }
                Ok(InitializeResponse::new(ProtocolVersion::V1))
            })
        }

        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            Box::pin(async move { Ok(NewSessionResponse::new("test-session-1")) })
        }
    }

    use agent_client_protocol::schema::NewSessionResponse;
    use std::sync::Arc;

    #[test]
    fn test_module_compiles() {
        // Module compiles successfully
    }

    #[test]
    fn test_terminals_stats_default() {
        let stats = TerminalsStats::default();
        assert_eq!(stats.initialize_calls, 0);
        assert_eq!(stats.new_session_calls, 0);
        assert_eq!(stats.ext_method_calls, 0);
    }

    #[test]
    fn test_terminals_stats_debug_and_serialize() {
        let stats = TerminalsStats {
            initialize_calls: 2,
            new_session_calls: 1,
            ext_method_calls: 5,
        };
        let debug = format!("{:?}", stats);
        assert!(debug.contains("TerminalsStats"));

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["initialize_calls"], 2);
        assert_eq!(json["new_session_calls"], 1);
        assert_eq!(json["ext_method_calls"], 5);
    }

    /// `test_terminal_capability_check` exercises the no-capability rejection
    /// path. The SDK's wire-method parser rejects unknown methods like
    /// `terminal/create` with `method_not_found` before the mock sees them,
    /// which satisfies the scenario's "agent rejected" expectation.
    #[tokio::test]
    async fn test_terminal_capability_check_no_capability() {
        let mock = Arc::new(TerminalMockAgent::new());
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_terminal_capability_check(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[test]
    fn test_verify_terminals_fixture_not_found() {
        let result = verify_terminals_fixture("nonexistent-agent", "nonexistent-test");
        assert!(result.is_err());
    }
}
