//! ACP protocol entry-points for ClaudeAgent.
//!
//! In ACP 0.10 these methods lived under `impl Agent for ClaudeAgent`, where
//! `Agent` was a trait the SDK invoked via dynamic dispatch. ACP 0.11 removes
//! that trait — `agent_client_protocol::Agent` is now a unit Role marker —
//! and replaces it with a typed builder/handler runtime. The connection
//! layer (`Agent.builder().on_receive_request(...).connect_with(...)`) is
//! wired up by the server module; this file holds the inherent methods
//! that the per-method handlers delegate to:
//! - `initialize` / `authenticate`
//! - `new_session` / `load_session` / `set_session_mode`
//! - `prompt` / `cancel`
//! - `ext_method` / `ext_notification`
//!
//! The method bodies are unchanged from the 0.10 trait impl — only the
//! wrapping syntax has flipped from `impl Agent for ClaudeAgent` to
//! `impl ClaudeAgent`.

use crate::agent::ClaudeAgent;
use crate::agent_file_operations::{ReadTextFileParams, WriteTextFileParams};
use agent_client_protocol::schema::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification, ExtRequest,
    ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest, LoadSessionResponse,
    NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse, RawValue,
    ResumeSessionRequest, ResumeSessionResponse, SessionId, SetSessionModeRequest,
    SetSessionModeResponse,
};
use std::sync::Arc;

// ACP protocol entry-points used by the SDK 0.11 builder/handler layer.
// Each method matches a JSON-RPC request handler registered on
// `Agent.builder().on_receive_request(...)` by the server module.
impl ClaudeAgent {
    // ACP AGENT PROTOCOL FLOW WITHOUT AUTHENTICATION:
    // 1. Client sends initialize request
    // 2. Agent responds with capabilities and authMethods: []
    // 3. Client can immediately call session/new (no auth step)
    // 4. Normal session operations proceed without authentication
    //
    // This is the correct flow for local development tools.

    /// Handle the ACP `initialize` request.
    ///
    /// `initialize` is a light, non-fatal handshake: it negotiates the
    /// protocol version, stores the client capabilities for capability gating,
    /// and returns the agent's advertised capabilities. Per the ACP
    /// specification it never hard-fails on a protocol-version mismatch — the
    /// negotiated version is returned and the client decides whether to
    /// proceed. There is no request-body validation beyond negotiation, and
    /// `llama-agent` follows the identical convention. Authentication methods
    /// are intentionally empty — see the architectural note in the body.
    pub async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, agent_client_protocol::Error> {
        self.log_request("initialize", &request);
        tracing::info!(
            "Initializing agent with client capabilities: {:?}",
            request.client_capabilities
        );

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
            agent_client_protocol::schema::Implementation::new("claude-agent", crate::VERSION)
                .title(format!("Claude Agent v{}", crate::VERSION));

        let response =
            InitializeResponse::new(Self::negotiate_protocol_version(&request.protocol_version))
                .agent_capabilities(self.capabilities.clone())
                .auth_methods(vec![])
                .agent_info(agent_info);

        self.log_response("initialize", &response);
        Ok(response)
    }

    /// Handle the ACP `authenticate` request.
    ///
    /// Always returns `method_not_found`: claude-agent declares no auth methods
    /// in `initialize`, so the client should never call this. If it does anyway,
    /// reject explicitly per the ACP spec.
    pub async fn authenticate(
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

        Err(crate::acp_error::method_not_found(
            "Authentication is not supported: claude-agent declares no auth methods in initialize.",
        ))
    }

    /// Handle the ACP `session/new` request.
    ///
    /// Validates MCP transport requirements, creates a new session, spawns the
    /// underlying Claude process, and returns the response (including any
    /// configured session modes).
    ///
    /// No `AvailableCommandsUpdate` notification is emitted at session creation.
    /// `AvailableCommandsUpdate` is a *change* notification — the agent emits it
    /// when the command set changes during a session (e.g. an MCP server
    /// connects and exposes prompts, via `refresh_commands_for_all_sessions`).
    /// An unsolicited update at `session/new` advertised only two
    /// non-dispatchable placeholder commands, and llama-agent emits nothing
    /// here; suppressing it makes both agents emit the same (empty) command
    /// stream on `session/new`.
    pub async fn new_session(
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

        // Connect any MCP servers supplied in the request so their tools are
        // exposed to the agent before the Claude process is spawned.
        self.connect_new_session_mcp_servers(&request).await;

        // Spawn Claude process and handle init. Skipped when
        // `spawn_claude_on_new_session` is disabled (test/headless seam); the
        // MCP servers are already connected above.
        if self.config.spawn_claude_on_new_session {
            self.spawn_claude_for_new_session(&session_id, &protocol_session_id, &request)
                .await;
        }

        // Build response with modes if applicable
        let response = self
            .build_new_session_response(&session_id, &protocol_session_id)
            .await;

        self.log_response("new_session", &response);
        Ok(response)
    }

    /// Handle the ACP `session/load` request.
    ///
    /// `session/load` is `session/resume` plus history replay. It loads the
    /// durable [`SessionRecord`](agent_client_protocol_extras::SessionRecord)
    /// from the shared `SessionStore`, restores agent state via
    /// [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore)
    /// (rehydrating the in-memory session and re-spawning the claude CLI with
    /// `--resume`), replays the recorded conversation as `session/update`
    /// notifications, and returns.
    ///
    /// A missing, expired, or corrupt record surfaces as a session-not-found
    /// error — the opaque session id is never rejected on format.
    pub async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);

        // Validate MCP transport requirements (capability gating).
        self.validate_load_session_mcp_config(&request)?;

        let session_id_str = request.session_id.0.to_string();
        let record = self
            .load_session_record(&session_id_str)
            .map_err(|e| self.restore_error_to_acp(&request.session_id, e))?;

        // Restore state: rehydrate the in-memory session and `claude --resume`.
        agent_client_protocol_extras::ResumeStrategy::restore(self, &record)
            .await
            .map_err(|e| self.session_restore_failed_error(&request.session_id, &e))?;

        // The replay step is the only thing `session/load` does beyond
        // `session/resume`: stream the recorded conversation back to the client.
        self.replay_record_updates(&record)
            .await
            .map_err(|e| self.restore_error_to_acp(&request.session_id, e))?;

        let response = self.build_load_session_response(&record);
        self.log_response("load_session", &response);
        Ok(response)
    }

    /// Handle the ACP `session/resume` request.
    ///
    /// `session/resume` restores agent state and returns — it MUST NOT replay
    /// history. It loads the durable
    /// [`SessionRecord`](agent_client_protocol_extras::SessionRecord) from the
    /// shared `SessionStore` and restores state via
    /// [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore),
    /// which rehydrates the in-memory session and re-spawns the claude CLI with
    /// `--resume` so the next `session/prompt` continues the conversation. The
    /// recorded conversation is *not* streamed back; that is `session/load`.
    ///
    /// A missing, expired, or corrupt record surfaces as a session-not-found
    /// error — the opaque session id is never rejected on format.
    pub async fn resume_session(
        &self,
        request: ResumeSessionRequest,
    ) -> Result<ResumeSessionResponse, agent_client_protocol::Error> {
        self.log_request("resume_session", &request);
        tracing::info!("Resuming session: {}", request.session_id);

        let session_id_str = request.session_id.0.to_string();
        let record = self
            .load_session_record(&session_id_str)
            .map_err(|e| self.restore_error_to_acp(&request.session_id, e))?;

        // Restore state only — no history replay, per the ACP resume contract.
        agent_client_protocol_extras::ResumeStrategy::restore(self, &record)
            .await
            .map_err(|e| self.session_restore_failed_error(&request.session_id, &e))?;

        let response = ResumeSessionResponse::new();
        self.log_response("resume_session", &response);
        Ok(response)
    }

    /// Handle the ACP `session/set_mode` request.
    ///
    /// Validates the requested mode against the configured set, updates the
    /// session's current mode, and (if the mode actually changed) replaces the
    /// underlying Claude process so the next prompt runs under the new mode.
    ///
    /// A `SessionUpdate::CurrentModeUpdate` notification is emitted on every
    /// successful call — exactly as llama-agent does — so a client tracking
    /// session mode observes the same notification stream from both agents.
    /// The process replacement is an internal, claude-specific concern gated on
    /// whether the mode actually changed; the client-facing notification is
    /// not.
    pub async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        self.log_request("set_session_mode", &request);

        // Resolve the opaque session id by existence — same not-found path and
        // same `invalid_params` error as `prompt` and `cancel`.
        let parsed_session_id = self.resolve_session(&request.session_id)?.id;
        let mode_id_string = request.mode_id.0.to_string();

        // Validate mode ID is in available modes
        self.validate_mode_exists(&mode_id_string).await?;

        // Check if mode will change and update session
        let mode_changed = self
            .check_and_update_session_mode(&parsed_session_id, &mode_id_string)
            .await?;

        // Handle process replacement if mode changed
        if mode_changed {
            self.handle_mode_change_process(&parsed_session_id, &mode_id_string)
                .await?;
        }

        // Emit the `CurrentModeUpdate` notification unconditionally — the mode
        // was successfully set, so confirm the active mode to the client
        // regardless of whether it differed from the previous mode. This keeps
        // claude's notification stream identical to llama's.
        self.send_mode_update_notification(&parsed_session_id, &request)
            .await?;

        let response = self.build_set_mode_response(mode_changed);
        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    /// Handle the ACP `session/prompt` request.
    ///
    /// Validates the request, sends user-message chunks back as session updates
    /// for transparency, honours any per-request `_meta.max_tokens` cap, and
    /// dispatches to the streaming or non-streaming prompt handler. Resets the
    /// per-session cancellation flag once the turn finishes so the next prompt
    /// starts fresh.
    pub async fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        self.log_prompt_debug(&request);

        self.validate_prompt_request(&request).await?;

        // Resolve the opaque session id by existence — this is the single
        // not-found path shared with `cancel` and `set_mode`. An unknown id
        // (non-ULID or simply absent) fails here with one `invalid_params`
        // error before any turn work begins.
        let session = self.resolve_session(&request.session_id)?;
        let session_id = session.id;

        // Send user message chunks for conversation transparency
        self.send_user_message_chunks(&request).await;

        // Check for pre-cancelled session
        if let Some(response) = self.check_cancelled_before_processing(&session_id).await {
            // `send_user_message_chunks` already grew the live session's
            // `context`; persist so a cancelled turn still records that state
            // durably rather than relying on the next successful turn.
            self.persist_session_record(&session_id);
            return Ok(response);
        }

        // Extract prompt content
        let prompt_text = self.extract_prompt_text(&request);

        // Prepare session for new turn
        self.prepare_session_for_turn(&session_id, &prompt_text)?;

        // Check turn limits
        if let Some(response) = self.check_turn_limits(&session_id, &prompt_text)? {
            // The user message has already been accumulated into the live
            // session's `context`; persist so a turn-limited turn still
            // records that state durably rather than relying on the next
            // successful turn.
            self.persist_session_record(&session_id);
            return Ok(response);
        }

        // Get session for prompt handling
        let updated_session = self.get_updated_session(&session_id)?;

        // Optional per-request generation cap. The ACP `_meta` map is the
        // documented extensibility channel — callers (e.g. the validator
        // runner) attach a `"max_tokens"` key here to defend against runaway
        // generation. The ACP spec lets agents ignore unknown `_meta` keys, so
        // honoring it is a deliberate opt-in: this agent enforces the cap at
        // the streaming layer below, narrowing — never widening — the
        // configured `max_tokens_per_turn`. Hitting the cap surfaces as
        // `StopReason::MaxTokens`.
        let requested_max_tokens =
            crate::agent_prompt_handling::extract_request_max_tokens(request.meta.as_ref());

        // Execute prompt (streaming or non-streaming)
        let response = if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(
                &session_id,
                &request,
                &updated_session,
                requested_max_tokens,
            )
            .await?
        } else {
            self.handle_non_streaming_prompt(
                &session_id,
                &request,
                &updated_session,
                requested_max_tokens,
            )
            .await?
        };

        // Reset cancellation for next turn
        self.cancellation_manager
            .reset_for_new_turn(&session_id.to_string())
            .await;

        // Persist a durable SessionRecord now that the turn's updates have been
        // accumulated into the live session. This is what makes the session
        // survive a process restart and answerable by `session/list`.
        self.persist_session_record(&session_id);

        // After the first meaningful exchange, generate a human-readable
        // session title and emit the built-in `SessionInfoUpdate`. This runs
        // off the turn's critical path and is a no-op once a title exists.
        self.maybe_generate_session_title(&session_id);

        self.log_response("prompt", &response);
        Ok(response)
    }

    /// Handle the ACP `session/cancel` notification.
    ///
    /// Marks the session cancelled, fans out cancellation to ongoing Claude
    /// requests, tool executions, and pending permission prompts, and emits
    /// final status updates. The original `session/prompt` call observes the
    /// cancellation flag and responds with the cancelled stop reason.
    pub async fn cancel(
        &self,
        notification: CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &notification);

        // Resolve the opaque session id by existence first — `cancel` uses the
        // same not-found path and the same `invalid_params` error as `prompt`
        // and `set_mode`. An unknown id is rejected before any cancellation
        // work begins.
        self.resolve_session(&notification.session_id)?;
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

    /// Handle extension method requests.
    ///
    /// Extension methods are JSON-RPC methods outside the core ACP request
    /// set. Claude Agent dispatches the `fs/*`, `terminal/*`, and
    /// `editor/update_buffers` extensions to dedicated handlers.
    ///
    /// ## Unknown methods
    ///
    /// A method that matches no handler is rejected with `method_not_found`
    /// (`-32601`) rather than answered with a success response. Reporting an
    /// unknown method as an error is the correct JSON-RPC behavior and matches
    /// `llama-agent`, so a client probing an unsupported extension observes
    /// the same failure from either agent.
    pub async fn ext_method(
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

    /// Handle ACP extension notifications.
    ///
    /// Extension notifications are fire-and-forget messages outside the core
    /// ACP protocol. Claude Agent currently logs and ignores them; this hook
    /// exists so future extensions can be wired in without churn at the
    /// builder layer.
    pub async fn ext_notification(
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

    /// Handle an unknown extension method.
    ///
    /// Rejects the call with `method_not_found` (`-32601`). An extension method
    /// the agent does not implement is genuinely "not found", so a JSON-RPC
    /// error — not a success response — is the correct answer. `llama-agent`
    /// rejects unknown extension methods the same way.
    fn handle_ext_unknown(
        &self,
        request: &ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        tracing::warn!("Unknown extension method: {}", request.method);
        Err(crate::acp_error::method_not_found(format!(
            "Extension method not found: {}",
            request.method
        )))
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
                Err(crate::acp_error::invalid_params(
                    "File system read capability not declared by client. Set client_capabilities.fs.read_text_file = true during initialization.",
                ))
            }
            None => {
                tracing::error!("No client capabilities for fs/read_text_file validation");
                Err(crate::acp_error::invalid_params(
                    "Client capabilities not initialized. Cannot perform file system operations without capability declaration.",
                ))
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
                Err(crate::acp_error::invalid_params(
                    "File system write capability not declared by client. Set client_capabilities.fs.write_text_file = true during initialization.",
                ))
            }
            None => {
                tracing::error!("No client capabilities for fs/write_text_file validation");
                Err(crate::acp_error::invalid_params(
                    "Client capabilities not initialized. Cannot perform file system operations without capability declaration.",
                ))
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
                Err(crate::acp_error::invalid_params(format!(
                    "Terminal capability not declared by client; {method} requires client_capabilities.terminal = true during initialization."
                )))
            }
            None => {
                tracing::error!("No client capabilities for {} validation", method);
                Err(crate::acp_error::invalid_params(format!(
                    "Client capabilities not initialized; cannot perform {method} without capability declaration."
                )))
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
                Err(crate::acp_error::invalid_params(
                    "Editor state capability not declared by client.",
                ))
            }
            None => {
                tracing::error!("No client capabilities for editor/update_buffers validation");
                Err(crate::acp_error::invalid_params(
                    "Client capabilities not initialized.",
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
                crate::acp_error::invalid_params(format!(
                    "Extension method {method} parameters are not valid JSON: {e}"
                ))
            })?;
        serde_json::from_value(params_value).map_err(|e| {
            tracing::error!("Failed to deserialize {} parameters: {}", method, e);
            crate::acp_error::invalid_params(format!(
                "Extension method {method} parameters do not match the expected schema: {e}"
            ))
        })
    }

    /// Convert response to ExtResponse.
    fn to_ext_response<T: serde::Serialize>(
        &self,
        response: T,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        let response_json = serde_json::to_value(response).map_err(|e| {
            tracing::error!("Failed to serialize extension method response: {}", e);
            crate::acp_error::internal_error(format!(
                "Failed to serialize extension method response: {e}"
            ))
        })?;
        let raw_value = RawValue::from_string(response_json.to_string()).map_err(|e| {
            tracing::error!(
                "Failed to build raw JSON for extension method response: {}",
                e
            );
            crate::acp_error::internal_error(format!(
                "Failed to build raw JSON for extension method response: {e}"
            ))
        })?;
        Ok(ExtResponse::new(Arc::from(raw_value)))
    }
}
