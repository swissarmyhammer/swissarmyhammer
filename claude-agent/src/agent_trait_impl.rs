//! Agent trait implementation for ClaudeAgent
//!
//! This module contains the implementation of the Agent trait from agent_client_protocol
//! for the ClaudeAgent struct. It handles the core ACP protocol methods including:
//! - initialize/authenticate
//! - new_session/load_session/set_session_mode
//! - prompt/cancel
//! - ext_method/ext_notification

use crate::agent::ClaudeAgent;
use crate::agent_file_operations::{ReadTextFileParams, WriteTextFileParams};
use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    RawValue, SessionId, SetSessionModeRequest, SetSessionModeResponse,
};
use std::sync::Arc;

#[async_trait::async_trait(?Send)]
impl Agent for ClaudeAgent {
    // ACP AGENT PROTOCOL FLOW WITHOUT AUTHENTICATION:
    // 1. Client sends initialize request
    // 2. Agent responds with capabilities and authMethods: []
    // 3. Client can immediately call session/new (no auth step)
    // 4. Normal session operations proceed without authentication
    //
    // This is the correct flow for local development tools.

    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, agent_client_protocol::Error> {
        self.log_request("initialize", &request);
        tracing::info!(
            "Initializing agent with client capabilities: {:?}",
            request.client_capabilities
        );

        // Validate initialization request structure
        if let Err(e) = self.validate_initialization_request(&request) {
            tracing::error!(
                "Initialization failed: Invalid request structure - {}",
                e.message
            );
            return Err(e);
        }

        // Validate protocol version
        if let Err(e) = self.validate_protocol_version(&request.protocol_version) {
            let fatal_error = self.handle_fatal_initialization_error(e).await;
            tracing::error!(
                "Initialization failed: Protocol version validation error - {}",
                fatal_error.message
            );
            return Err(fatal_error);
        }

        // Validate client capabilities
        if let Err(e) = self.validate_client_capabilities(&request.client_capabilities) {
            tracing::error!(
                "Initialization failed: Client capability validation error - {}",
                e.message
            );
            return Err(e);
        }

        tracing::info!("Agent initialization validation completed successfully");

        // Store client capabilities for ACP compliance - required for capability gating
        {
            let mut client_caps = self.client_capabilities.write().await;
            *client_caps = Some(request.client_capabilities.clone());
        }

        // Pass client capabilities to tool handler for capability validation
        {
            let mut tool_handler = self.tool_handler.write().await;
            tool_handler.set_client_capabilities(request.client_capabilities.clone());
        }

        tracing::info!("Stored client capabilities for ACP compliance");

        // AUTHENTICATION ARCHITECTURE DECISION:
        // Claude Code is a local development tool that runs entirely on the user's machine.
        // It does not require authentication because:
        // 1. It operates within the user's own development environment
        // 2. It does not connect to external services requiring credentials
        // 3. It has no multi-user access control requirements
        // 4. All operations are performed with the user's existing local permissions
        //
        // Therefore, we intentionally declare NO authentication methods (empty array).
        // This is an architectural decision - do not add authentication methods.
        // If remote authentication is needed in the future, it should be a separate feature.

        let agent_info =
            agent_client_protocol::Implementation::new("claude-agent", env!("CARGO_PKG_VERSION"))
                .title(format!("Claude Agent v{}", env!("CARGO_PKG_VERSION")));

        let response =
            InitializeResponse::new(self.negotiate_protocol_version(&request.protocol_version))
                .agent_capabilities(self.capabilities.clone())
                .auth_methods(vec![])
                .agent_info(agent_info);

        self.log_response("initialize", &response);
        Ok(response)
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, agent_client_protocol::Error> {
        self.log_request("authenticate", &request);

        // AUTHENTICATION ARCHITECTURE DECISION:
        // Claude Code declares NO authentication methods in initialize().
        // According to ACP spec, clients should not call authenticate when no methods are declared.
        // If they do call authenticate anyway, we reject it with a clear error.
        tracing::warn!(
            "Authentication attempt rejected - no auth methods declared: {:?}",
            request.method_id
        );

        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.log_request("new_session", &request);
        tracing::info!("Creating new session");

        // Validate MCP transport requirements
        self.validate_new_session_mcp_config(&request)?;

        // Create the session
        let session_id = self.create_new_session_internal(&request).await?;
        let protocol_session_id = SessionId::new(session_id.to_string());

        // Spawn Claude process and handle init
        self.spawn_claude_for_new_session(&session_id, &protocol_session_id, &request)
            .await;

        // Send initial commands
        self.send_initial_session_commands(&session_id, &protocol_session_id)
            .await;

        // Build response with modes if applicable
        let response = self
            .build_new_session_response(&session_id, &protocol_session_id)
            .await;

        self.log_response("new_session", &response);
        Ok(response)
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);

        // Validate MCP transport requirements
        self.validate_load_session_mcp_config(&request)?;

        let session_id = self.parse_session_id(&request.session_id)?;
        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        match session {
            Some(session) => {
                let response = self.handle_session_found(&session).await;
                self.log_response("load_session", &response);
                Ok(response)
            }
            None => Err(self.session_not_found_error(&request.session_id)),
        }
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        self.log_request("set_session_mode", &request);

        let parsed_session_id = self.parse_mode_session_id(&request.session_id)?;
        let mode_id_string = request.mode_id.0.to_string();

        // Validate mode ID is in available modes
        self.validate_mode_exists(&mode_id_string).await?;

        // Check if mode will change and update session
        let mode_changed = self
            .check_and_update_session_mode(&parsed_session_id, &mode_id_string)
            .await?;

        // Handle process replacement if mode changed
        if mode_changed {
            self.handle_mode_change_process(&parsed_session_id, &mode_id_string, &request)
                .await?;
        }

        let response = self.build_set_mode_response(mode_changed);
        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        self.log_prompt_debug(&request);

        self.validate_prompt_request(&request).await?;
        let session_id = self.parse_session_id(&request.session_id)?;

        // Send user message chunks for conversation transparency
        self.send_user_message_chunks(&request).await;

        // Check for pre-cancelled session
        if let Some(response) = self.check_cancelled_before_processing(&session_id).await {
            return Ok(response);
        }

        // Extract prompt content and validate session
        let prompt_text = self.extract_prompt_text(&request);
        let session = self.get_validated_session(&session_id)?;

        // Prepare session for new turn
        self.prepare_session_for_turn(&session_id, &prompt_text)?;

        // Check turn limits
        if let Some(response) = self.check_turn_limits(&session_id, &prompt_text)? {
            return Ok(response);
        }

        // Get session for prompt handling
        let updated_session = self.get_updated_session(&session_id)?;

        // Execute prompt (streaming or non-streaming)
        let response = if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        } else {
            self.handle_non_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        };

        // Reset cancellation for next turn
        self.cancellation_manager
            .reset_for_new_turn(&session_id.to_string())
            .await;

        self.log_response("prompt", &response);
        Ok(response)
    }

    async fn cancel(
        &self,
        notification: CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &notification);
        let session_id = &notification.session_id.0;

        tracing::info!("Processing cancellation for session: {}", session_id);

        // ACP requires immediate and comprehensive cancellation handling:
        // 1. Process session/cancel notifications immediately
        // 2. Cancel ALL ongoing operations (LM, tools, permissions)
        // 3. Send final status updates before responding
        // 4. Respond to original session/prompt with cancelled stop reason
        // 5. Clean up all resources and prevent orphaned operations
        //
        // Cancellation must be fast and reliable to maintain responsiveness.

        // 1. Immediately mark session as cancelled
        if let Err(e) = self
            .cancellation_manager
            .mark_cancelled(session_id, "Client sent session/cancel notification")
            .await
        {
            tracing::error!("Failed to mark session {} as cancelled: {}", session_id, e);
            // Continue with cancellation despite state update failure
        }

        // 2. Cancel all ongoing operations for this session
        tokio::join!(
            self.cancel_claude_requests(session_id),
            self.cancel_tool_executions(session_id),
            self.cancel_permission_requests(session_id)
        );

        // 3. Send final status updates for any pending operations
        if let Err(e) = self.send_final_cancellation_updates(session_id).await {
            tracing::warn!(
                "Failed to send final cancellation updates for session {}: {}",
                session_id,
                e
            );
            // Don't fail cancellation due to notification issues
        }

        // 4. The original session/prompt will respond with cancelled stop reason
        // when it detects the cancellation state - this happens automatically
        // in the prompt method implementation

        tracing::info!(
            "Cancellation processing completed for session: {}",
            session_id
        );
        Ok(())
    }

    /// Handle extension method requests
    ///
    /// Extension methods allow clients to call custom methods not defined in the core
    /// Agent Client Protocol specification. This implementation returns a placeholder
    /// response indicating that extension methods are not currently supported.
    ///
    /// ## Design Decision
    ///
    /// Claude Agent currently does not require any extension methods beyond the standard
    /// ACP specification. The core protocol provides sufficient capabilities for:
    /// - Session management (new_session, load_session, set_session_mode)
    /// - Authentication (handled via empty auth_methods)
    /// - Tool execution (via prompt requests)
    /// - Session updates and notifications
    ///
    /// If future requirements emerge for custom extension methods, this implementation
    /// can be enhanced to dispatch to specific handlers based on the method name.
    ///
    /// ## Protocol Compliance
    ///
    /// This implementation satisfies the ACP requirement that agents must respond to
    /// extension method calls, even if they don't implement any specific extensions.
    /// Returning a structured response (rather than an error) maintains client compatibility.
    async fn ext_method(
        &self,
        request: ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.log_request("ext_method", &request);
        tracing::info!("Extension method called: {}", request.method);

        let method_str: &str = request.method.as_ref();
        match method_str {
            "fs/read_text_file" => self.handle_ext_read_text_file(&request).await,
            "fs/write_text_file" => self.handle_ext_write_text_file(&request).await,
            "terminal/output" => self.handle_ext_terminal_output(&request).await,
            "terminal/release" => self.handle_ext_terminal_release(&request).await,
            "terminal/wait_for_exit" => self.handle_ext_terminal_wait_for_exit(&request).await,
            "terminal/kill" => self.handle_ext_terminal_kill(&request).await,
            "terminal/create" => self.handle_ext_terminal_create(&request).await,
            "editor/update_buffers" => self.handle_ext_editor_update_buffers(&request).await,
            _ => self.handle_ext_unknown(&request),
        }
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("ext_notification", &notification);
        tracing::info!("Extension notification received: {}", notification.method);

        // Handle extension notifications
        Ok(())
    }
}

/// Extension method helpers for ClaudeAgent.
impl ClaudeAgent {
    /// Handle fs/read_text_file extension method.
    async fn handle_ext_read_text_file(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_fs_read_capability().await?;
        let params: ReadTextFileParams = self.parse_ext_params(request, "fs/read_text_file")?;
        let response = self.handle_read_text_file(params).await?;
        self.to_ext_response(response)
    }

    /// Handle fs/write_text_file extension method.
    async fn handle_ext_write_text_file(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_fs_write_capability().await?;
        let params: WriteTextFileParams = self.parse_ext_params(request, "fs/write_text_file")?;
        let response = self.handle_write_text_file(params).await?;
        self.to_ext_response(response)
    }

    /// Handle terminal/output extension method.
    async fn handle_ext_terminal_output(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_ext_terminal_capability("terminal/output")
            .await?;
        let params: crate::terminal_manager::TerminalOutputParams =
            self.parse_ext_params(request, "terminal/output")?;
        let response = self.handle_terminal_output(params).await?;
        self.to_ext_response(response)
    }

    /// Handle terminal/release extension method.
    async fn handle_ext_terminal_release(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_ext_terminal_capability("terminal/release")
            .await?;
        let params: crate::terminal_manager::TerminalReleaseParams =
            self.parse_ext_params(request, "terminal/release")?;
        let response = self.handle_terminal_release(params).await?;
        self.to_ext_response(response)
    }

    /// Handle terminal/wait_for_exit extension method.
    async fn handle_ext_terminal_wait_for_exit(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_ext_terminal_capability("terminal/wait_for_exit")
            .await?;
        let params: crate::terminal_manager::TerminalOutputParams =
            self.parse_ext_params(request, "terminal/wait_for_exit")?;
        let response = self.handle_terminal_wait_for_exit(params).await?;
        self.to_ext_response(response)
    }

    /// Handle terminal/kill extension method.
    async fn handle_ext_terminal_kill(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_ext_terminal_capability("terminal/kill")
            .await?;
        let params: crate::terminal_manager::TerminalOutputParams =
            self.parse_ext_params(request, "terminal/kill")?;
        self.handle_terminal_kill(params).await?;
        self.to_ext_response(serde_json::Value::Null)
    }

    /// Handle terminal/create extension method.
    async fn handle_ext_terminal_create(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_ext_terminal_capability("terminal/create")
            .await?;
        let params: crate::terminal_manager::TerminalCreateParams =
            self.parse_ext_params(request, "terminal/create")?;
        let response = self.handle_terminal_create(params).await?;
        self.to_ext_response(response)
    }

    /// Handle editor/update_buffers extension method.
    async fn handle_ext_editor_update_buffers(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.validate_editor_capability().await?;
        let response: crate::editor_state::EditorBufferResponse =
            self.parse_ext_params(request, "editor/update_buffers")?;
        tracing::info!(
            "Updating editor buffers cache with {} buffers",
            response.buffers.len()
        );
        self.editor_state_manager
            .update_buffers_from_response(response)
            .await;
        self.to_ext_response(serde_json::Value::Null)
    }

    /// Handle unknown extension method.
    fn handle_ext_unknown(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        let response = serde_json::json!({
            "method": request.method,
            "result": "Extension method not implemented"
        });
        let raw_value = RawValue::from_string(response.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        Ok(ExtResponse::new(Arc::from(raw_value)))
    }

    /// Validate file system read capability.
    async fn validate_fs_read_capability(&self) -> Result<(), agent_client_protocol::Error> {
        let client_caps = self.client_capabilities.read().await;
        match &*client_caps {
            Some(caps) if caps.fs.read_text_file => {
                tracing::debug!("File system read capability validated");
                Ok(())
            }
            Some(_) => {
                tracing::error!("fs/read_text_file capability not declared by client");
                Err(agent_client_protocol::Error::invalid_params())
            }
            None => {
                tracing::error!("No client capabilities for fs/read_text_file validation");
                Err(agent_client_protocol::Error::invalid_params())
            }
        }
    }

    /// Validate file system write capability.
    async fn validate_fs_write_capability(&self) -> Result<(), agent_client_protocol::Error> {
        let client_caps = self.client_capabilities.read().await;
        match &*client_caps {
            Some(caps) if caps.fs.write_text_file => {
                tracing::debug!("File system write capability validated");
                Ok(())
            }
            Some(_) => {
                tracing::error!("fs/write_text_file capability not declared by client");
                Err(agent_client_protocol::Error::invalid_params())
            }
            None => {
                tracing::error!("No client capabilities for fs/write_text_file validation");
                Err(agent_client_protocol::Error::invalid_params())
            }
        }
    }

    /// Validate terminal capability.
    async fn validate_ext_terminal_capability(
        &self,
        method: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let client_caps = self.client_capabilities.read().await;
        match &*client_caps {
            Some(caps) if caps.terminal => {
                tracing::debug!("Terminal capability validated for {}", method);
                Ok(())
            }
            Some(_) => {
                tracing::error!("{} capability not declared by client", method);
                Err(agent_client_protocol::Error::invalid_params())
            }
            None => {
                tracing::error!("No client capabilities for {} validation", method);
                Err(agent_client_protocol::Error::invalid_params())
            }
        }
    }

    /// Validate editor state capability.
    async fn validate_editor_capability(&self) -> Result<(), agent_client_protocol::Error> {
        let client_caps = self.client_capabilities.read().await;
        match &*client_caps {
            Some(caps) if crate::editor_state::supports_editor_state(caps) => {
                tracing::debug!("Editor state capability declared and validated");
                Ok(())
            }
            Some(_) => {
                tracing::error!("editor/update_buffers capability not declared by client");
                Err(agent_client_protocol::Error::new(
                    -32602,
                    "Editor state capability not declared by client.".to_string(),
                ))
            }
            None => {
                tracing::error!("No client capabilities for editor/update_buffers validation");
                Err(agent_client_protocol::Error::new(
                    -32602,
                    "Client capabilities not initialized.".to_string(),
                ))
            }
        }
    }

    /// Parse extension method parameters.
    fn parse_ext_params<T: serde::de::DeserializeOwned>(
        &self,
        request: &ExtRequest,
        method: &str,
    ) -> Result<T, agent_client_protocol::Error> {
        let params_value: serde_json::Value =
            serde_json::from_str(request.params.get()).map_err(|e| {
                tracing::error!("Failed to parse {} parameters: {}", method, e);
                agent_client_protocol::Error::invalid_params()
            })?;
        serde_json::from_value(params_value).map_err(|e| {
            tracing::error!("Failed to deserialize {} parameters: {}", method, e);
            agent_client_protocol::Error::invalid_params()
        })
    }

    /// Convert response to ExtResponse.
    fn to_ext_response<T: serde::Serialize>(
        &self,
        response: T,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        let response_json = serde_json::to_value(response)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        let raw_value = RawValue::from_string(response_json.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        Ok(ExtResponse::new(Arc::from(raw_value)))
    }
}
