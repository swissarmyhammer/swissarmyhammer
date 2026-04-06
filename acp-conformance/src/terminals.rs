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

use agent_client_protocol::{
    Agent, ClientCapabilities, ExtRequest, InitializeRequest, ProtocolVersion,
};
use agent_client_protocol_extras::recording::RecordedSession;
use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_common::Pretty;

/// Statistics from terminals fixture verification
#[derive(Debug, Default, serde::Serialize)]
pub struct TerminalsStats {
    pub initialize_calls: usize,
    pub new_session_calls: usize,
    pub ext_method_calls: usize,
}

/// Test that agent properly checks terminal capability before allowing terminal operations
pub async fn test_terminal_capability_check<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal capability check");

    // Initialize with NO terminal capability
    let client_caps = ClientCapabilities::new().terminal(false);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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
    let result = agent.ext_method(ext_request).await;

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
pub async fn test_terminal_create<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal/create");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let result = agent.ext_method(ext_request).await;

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
                    let _ = agent.ext_method(release_request).await;

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
pub async fn test_terminal_output<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal/output");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let create_response = agent.ext_method(create_request).await?;
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

    let output_result = agent.ext_method(output_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = agent.ext_method(release_request).await;

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
pub async fn test_terminal_wait_for_exit<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal/wait_for_exit");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let create_response = agent.ext_method(create_request).await?;
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

    let wait_result = agent.ext_method(wait_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = agent.ext_method(release_request).await;

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
pub async fn test_terminal_kill<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal/kill");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let create_response = agent.ext_method(create_request).await?;
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

    let kill_result = agent.ext_method(kill_request).await;

    // Terminal should still be valid, try to get output
    let output_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });

    let output_request = ExtRequest::new(
        "terminal/output",
        Arc::from(serde_json::value::to_raw_value(&output_params)?),
    );

    let output_result = agent.ext_method(output_request).await;

    // Clean up
    let release_params = json!({
        "sessionId": session_id.0,
        "terminalId": terminal_id
    });
    let release_request = ExtRequest::new(
        "terminal/release",
        Arc::from(serde_json::value::to_raw_value(&release_params)?),
    );
    let _ = agent.ext_method(release_request).await;

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
pub async fn test_terminal_release<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal/release");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let create_response = agent.ext_method(create_request).await?;
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

    let release_result = agent.ext_method(release_request).await;

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

    let output_result = agent.ext_method(output_request).await;

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
pub async fn test_terminal_timeout<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing terminal timeout pattern");

    // Initialize with terminal capability
    let client_caps = ClientCapabilities::new().terminal(true);

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
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

    let create_response = agent.ext_method(create_request).await?;
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

    let wait_future = agent.ext_method(wait_request);
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

            agent.ext_method(kill_request).await?;

            // Get final output
            let output_params = json!({
                "sessionId": session_id.0,
                "terminalId": terminal_id
            });

            let output_request = ExtRequest::new(
                "terminal/output",
                Arc::from(serde_json::value::to_raw_value(&output_params)?),
            );

            let _ = agent.ext_method(output_request).await?;

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
    let _ = agent.ext_method(release_request).await;

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
    use agent_client_protocol::{
        AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
        ExtResponse, InitializeResponse, LoadSessionRequest, LoadSessionResponse,
        NewSessionResponse, PromptRequest, PromptResponse, SetSessionModeRequest,
        SetSessionModeResponse, StopReason,
    };
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock agent that supports terminal operations via ext_method
    struct TerminalMockAgent {
        /// Whether terminal capability was declared during init
        terminal_enabled: AtomicBool,
        /// Tracks created terminal IDs
        terminal_created: std::sync::Mutex<Vec<String>>,
        /// Tracks released terminal IDs
        terminal_released: std::sync::Mutex<Vec<String>>,
        /// Tracks killed terminal IDs
        terminal_killed: std::sync::Mutex<Vec<String>>,
    }

    impl TerminalMockAgent {
        fn new() -> Self {
            Self {
                terminal_enabled: AtomicBool::new(false),
                terminal_created: std::sync::Mutex::new(Vec::new()),
                terminal_released: std::sync::Mutex::new(Vec::new()),
                terminal_killed: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Agent for TerminalMockAgent {
        async fn initialize(
            &self,
            request: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            // Check if terminal capability was declared
            if request.client_capabilities.terminal {
                self.terminal_enabled.store(true, Ordering::SeqCst);
            }
            Ok(InitializeResponse::new(ProtocolVersion::V1))
        }

        async fn authenticate(
            &self,
            _request: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }

        async fn new_session(
            &self,
            _request: agent_client_protocol::NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session-1"))
        }

        async fn prompt(
            &self,
            _request: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            Ok(PromptResponse::new(StopReason::EndTurn))
        }

        async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }

        async fn load_session(
            &self,
            _request: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }

        async fn set_session_mode(
            &self,
            _request: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }

        async fn ext_method(
            &self,
            request: ExtRequest,
        ) -> agent_client_protocol::Result<ExtResponse> {
            let params: serde_json::Value =
                serde_json::from_str(request.params.get()).unwrap_or_default();

            if !self.terminal_enabled.load(Ordering::SeqCst) {
                return Err(agent_client_protocol::Error::invalid_params());
            }

            match &*request.method {
                "terminal/create" => {
                    let tid = format!("term-{}", uuid_like());
                    self.terminal_created.lock().unwrap().push(tid.clone());
                    let resp = json!({"terminalId": tid});
                    Ok(ExtResponse::new(Arc::from(
                        serde_json::value::to_raw_value(&resp).unwrap(),
                    )))
                }
                "terminal/output" => {
                    let tid = params
                        .get("terminalId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    // Check if released
                    if self
                        .terminal_released
                        .lock()
                        .unwrap()
                        .contains(&tid.to_string())
                    {
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    let resp = json!({"output": "hello world\n", "truncated": false});
                    Ok(ExtResponse::new(Arc::from(
                        serde_json::value::to_raw_value(&resp).unwrap(),
                    )))
                }
                "terminal/wait_for_exit" => {
                    let resp = json!({"exitCode": 0, "signal": ""});
                    Ok(ExtResponse::new(Arc::from(
                        serde_json::value::to_raw_value(&resp).unwrap(),
                    )))
                }
                "terminal/kill" => {
                    let tid = params
                        .get("terminalId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.terminal_killed.lock().unwrap().push(tid);
                    let resp = json!({"success": true});
                    Ok(ExtResponse::new(Arc::from(
                        serde_json::value::to_raw_value(&resp).unwrap(),
                    )))
                }
                "terminal/release" => {
                    let tid = params
                        .get("terminalId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.terminal_released.lock().unwrap().push(tid);
                    let resp = json!({"success": true});
                    Ok(ExtResponse::new(Arc::from(
                        serde_json::value::to_raw_value(&resp).unwrap(),
                    )))
                }
                _ => Err(agent_client_protocol::Error::method_not_found()),
            }
        }

        async fn ext_notification(
            &self,
            _notification: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    /// Simple counter for unique IDs
    fn uuid_like() -> String {
        use std::sync::atomic::AtomicU64;
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        format!("{}", COUNTER.fetch_add(1, Ordering::SeqCst))
    }

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

    #[tokio::test]
    async fn test_terminal_capability_check_no_capability() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_capability_check(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_create_with_capability() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_create(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_output_with_capability() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_output(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_wait_for_exit_flow() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_wait_for_exit(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_kill_flow() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_kill(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_release_flow() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_release(&agent).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminal_timeout_flow() {
        let agent = TerminalMockAgent::new();
        let result = test_terminal_timeout(&agent).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_terminals_fixture_not_found() {
        let result = verify_terminals_fixture("nonexistent-agent", "nonexistent-test");
        assert!(result.is_err());
    }
}
