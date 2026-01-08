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
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
    ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, RawValue, SessionId, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason,
};
use std::sync::Arc;
use swissarmyhammer_common::Pretty;

use crate::agent_raw_messages::RawMessageManager;

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

        // ACP requires strict transport capability enforcement:
        // 1. stdio: Always supported (mandatory per spec)
        // 2. http: Only if mcpCapabilities.http: true was declared
        // 3. sse: Only if mcpCapabilities.sse: true was declared
        //
        // This prevents protocol violations and ensures capability negotiation contract.

        // Convert ACP MCP server configs to internal types for validation
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        // Validate transport requirements against agent capabilities
        if let Err(validation_error) = crate::capability_validation::CapabilityRequirementChecker::check_new_session_requirements(
            &self.capabilities,
            &internal_mcp_servers,
        ) {
            tracing::error!("Session creation failed: Transport validation error - {}", validation_error);
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }

        let client_caps = {
            let guard = self.client_capabilities.read().await;
            guard.clone()
        };

        let session_id = self
            .session_manager
            .create_session(request.cwd.clone(), client_caps)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        // Register RawMessageManager for this session so subagents can find it
        if let Some(ref manager) = self.raw_message_manager {
            RawMessageManager::register(session_id.to_string(), manager.clone());
            tracing::debug!("Registered RawMessageManager for session {}", session_id);
        }

        // Store MCP servers in the session if provided
        if !request.mcp_servers.is_empty() {
            self.session_manager
                .update_session(&session_id, |session| {
                    // Store the actual MCP server info from the request as JSON strings
                    session.mcp_servers = request
                        .mcp_servers
                        .iter()
                        .map(|server| {
                            serde_json::to_string(server)
                                .unwrap_or_else(|_| format!("{:?}", server))
                        })
                        .collect();
                })
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        }

        tracing::info!("Created session: {}", session_id);

        let protocol_session_id = SessionId::new(session_id.to_string());

        // Spawn Claude process immediately and read init message with slash_commands and available_agents
        // Pass agent's configured MCP servers (self.config.mcp_servers) to Claude CLI
        // These are the MCP servers configured during agent creation, not from the request
        tracing::info!("Spawning Claude process for session: {}", session_id);
        match self
            .claude_client
            .spawn_process_and_consume_init(
                &session_id,
                &protocol_session_id,
                &request.cwd,
                self.config.mcp_servers.clone(),
                None, // No agent mode at initial session creation
                None, // No system prompt at initial session creation
            )
            .await
        {
            Ok((Some(agents), current_agent)) => {
                tracing::info!(
                    "Storing {} available agents from Claude CLI init",
                    agents.len()
                );
                self.set_available_agents(agents).await;

                // Set initial mode if Claude CLI specified current_agent
                if let Some(mode) = current_agent {
                    tracing::info!("Setting initial mode from Claude CLI: {}", mode);
                    self.session_manager
                        .update_session(&session_id, |session| {
                            session.current_mode = Some(mode.clone());
                        })
                        .map_err(|_| {
                            tracing::warn!("Failed to set initial mode");
                        })
                        .ok();
                } else {
                    tracing::debug!(
                        "No current_agent in init - session starts without mode (no --agent flag)"
                    );
                }
            }
            Ok((None, _)) => {
                tracing::debug!("No available agents in Claude CLI init message");
                // Still load SwissArmyHammer modes even if Claude CLI didn't provide agents
                self.set_available_agents(vec![]).await;
            }
            Err(e) => {
                tracing::error!("Failed to spawn Claude process and read init: {}", e);
                // Still load SwissArmyHammer modes even if Claude CLI failed
                self.set_available_agents(vec![]).await;
            }
        }

        // Send initial available commands after session creation (core + tool_handler commands)
        let initial_commands = self
            .get_available_commands_for_session(&protocol_session_id)
            .await;
        if let Err(e) = self
            .update_session_available_commands(&protocol_session_id, initial_commands)
            .await
        {
            tracing::warn!(
                "Failed to send initial available commands for session {}: {}",
                session_id,
                e
            );
        }

        let mut response = NewSessionResponse::new(SessionId::new(session_id.to_string()));

        // Add available modes only if the session has a mode explicitly set
        // Per user requirement: don't assume any default mode - no mode means no --agent flag
        if let Some(available_modes) = self.get_available_modes().await {
            // Only include modes in response if session has current_mode set
            if let Some(current_mode_id) = self.get_session_mode(&session_id).await {
                let mode_state = agent_client_protocol::SessionModeState::new(
                    agent_client_protocol::SessionModeId::new(current_mode_id.as_str()),
                    available_modes,
                );
                response = response.modes(mode_state);
                tracing::info!("Session created with mode: {}", current_mode_id);
            } else {
                // Modes are available but not set - don't include in response
                // This allows sessions to run without --agent flag until mode is explicitly set
                tracing::debug!(
                    "Session created without mode (available modes: {}, will not use --agent flag)",
                    available_modes.len()
                );
            }
        }

        self.log_response("new_session", &response);
        Ok(response)
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);

        // ACP requires complete conversation history replay during session loading:
        // 1. Validate loadSession capability before allowing session/load
        // 2. Stream ALL historical messages via session/update notifications
        // 3. Maintain exact chronological order of original conversation
        // 4. Only respond to session/load AFTER all history is streamed
        // 5. Client can then continue conversation seamlessly

        // ACP requires strict transport capability enforcement for session loading:
        // Convert ACP MCP server configs to internal types for validation
        let internal_mcp_servers: Vec<crate::config::McpServerConfig> = request
            .mcp_servers
            .iter()
            .filter_map(|server| self.convert_acp_to_internal_mcp_config(server))
            .collect();

        // Validate transport requirements and loadSession capability
        if let Err(validation_error) = crate::capability_validation::CapabilityRequirementChecker::check_load_session_requirements(
            &self.capabilities,
            &internal_mcp_servers,
        ) {
            tracing::error!("Session loading failed: Transport/capability validation error - {}", validation_error);
            return Err(self.convert_session_setup_error_to_acp_error(validation_error));
        }

        let session_id = self.parse_session_id(&request.session_id)?;

        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        match session {
            Some(session) => {
                tracing::info!(
                    "Loaded session: {} with {} historical messages",
                    session_id,
                    session.context.len()
                );

                // Step 2-3: Stream ALL historical messages via session/update notifications
                // Maintain exact chronological order using message timestamps
                if !session.context.is_empty() {
                    tracing::info!(
                        "Replaying {} historical messages for session {}",
                        session.context.len(),
                        session_id
                    );

                    for message in &session.context {
                        // Use the SessionUpdate stored in the message directly
                        let mut meta_map = serde_json::Map::new();
                        meta_map.insert(
                            "timestamp".to_string(),
                            serde_json::json!(message
                                .timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()),
                        );
                        meta_map.insert(
                            "message_type".to_string(),
                            serde_json::json!("historical_replay"),
                        );

                        let notification = SessionNotification::new(
                            SessionId::new(session.id.to_string()),
                            message.update.clone(),
                        )
                        .meta(meta_map);

                        // Stream historical message via session/update notification
                        // Note: send_update() queues the notification in the broadcast channel
                        // The notification_handler task processes these concurrently
                        if let Err(e) = self.notification_sender.send_update(notification).await {
                            tracing::error!(
                                "Failed to send historical message notification: {}",
                                e
                            );
                            // Continue with other messages even if one fails
                        }
                    }

                    tracing::info!(
                        "Completed queueing {} history notifications for session {}",
                        session.context.len(),
                        session_id
                    );
                }

                // Step 4: Return LoadSessionResponse after all history notifications are queued
                // The notifications are processed by the notification_handler task concurrently.
                // The broadcast channel and shared writer Mutex ensure notifications are delivered
                // to the client before this response due to:
                // 1. FIFO ordering in the broadcast channel
                // 2. Notification handler actively polling the channel
                // 3. Serialized writes through the shared Mutex-protected writer
                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "session_id".to_string(),
                    serde_json::json!(session.id.to_string()),
                );
                meta_map.insert(
                    "created_at".to_string(),
                    serde_json::json!(session
                        .created_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()),
                );
                meta_map.insert(
                    "message_count".to_string(),
                    serde_json::json!(session.context.len()),
                );
                meta_map.insert(
                    "history_replayed".to_string(),
                    serde_json::json!(session.context.len()),
                );

                let response = LoadSessionResponse::new().meta(meta_map);
                self.log_response("load_session", &response);
                Ok(response)
            }
            None => {
                tracing::warn!("Session not found: {}", session_id);
                Err(agent_client_protocol::Error::new(
                    -32602,
                    "Session not found: sessionId does not exist or has expired".to_string(),
                )
                .data(serde_json::json!({
                    "sessionId": request.session_id,
                    "error": "session_not_found"
                })))
            }
        }
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        self.log_request("set_session_mode", &request);

        let parsed_session_id = match crate::session::SessionId::parse(&request.session_id.0) {
            Ok(id) => id,
            Err(_) => {
                return Err(agent_client_protocol::Error::invalid_request());
            }
        };

        let mode_id_string = request.mode_id.0.to_string();

        // Validate mode ID is in available modes
        let available_agents = self.available_agents.read().await;
        if let Some(agents) = available_agents.as_ref() {
            let mode_exists = agents.iter().any(|(id, _, _)| id == &mode_id_string);
            if !mode_exists {
                tracing::error!(
                    "Invalid mode '{}' requested. Available modes: {:?}",
                    mode_id_string,
                    agents
                        .iter()
                        .map(|(id, name, _)| format!("{}:{}", id, name))
                        .collect::<Vec<_>>()
                );
                return Err(agent_client_protocol::Error::invalid_params());
            }
        } else {
            // No available modes - shouldn't happen but reject to be safe
            tracing::warn!("set_session_mode called but no available modes configured");
            return Err(agent_client_protocol::Error::invalid_params());
        }
        drop(available_agents);

        // Get the current mode to check if it will change
        let current_mode = self
            .session_manager
            .get_session(&parsed_session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .map(|session| session.current_mode.clone())
            .unwrap_or(None);

        let mode_changed = current_mode != Some(mode_id_string.clone());

        // Update session with new mode
        self.session_manager
            .update_session(&parsed_session_id, |session| {
                session.current_mode = Some(mode_id_string.clone());
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        tracing::info!("Session mode set to: {}", mode_id_string);

        // When mode changes, terminate the existing process and spawn a new one
        // This is effectively a powerful "clear" - the new process starts fresh
        if mode_changed {
            // Get session cwd for spawning new process
            let session = self
                .session_manager
                .get_session(&parsed_session_id)
                .map_err(|_| agent_client_protocol::Error::internal_error())?
                .ok_or_else(|| agent_client_protocol::Error::internal_error())?;
            let cwd = session.cwd.clone();

            // Terminate the existing Claude process
            tracing::info!(
                "Mode changed for session {}, terminating process and spawning new one with mode '{}'",
                parsed_session_id,
                mode_id_string
            );

            if let Err(e) = self
                .claude_client
                .terminate_session(&parsed_session_id)
                .await
            {
                tracing::warn!(
                    "Failed to terminate Claude process for session {}: {}",
                    parsed_session_id,
                    e
                );
            }

            // Determine spawn flags based on mode type
            let (agent_mode, system_prompt) =
                if let Some(prompt) = self.get_sah_mode_system_prompt(&mode_id_string).await {
                    // SAH mode: use --system-prompt
                    tracing::info!(
                    "Spawning new Claude process with --system-prompt ({} chars) for SAH mode '{}'",
                    prompt.len(),
                    mode_id_string
                );
                    (None, Some(prompt))
                } else {
                    // Claude CLI mode: use --agent
                    tracing::info!(
                        "Spawning new Claude process with --agent '{}' for Claude CLI mode",
                        mode_id_string
                    );
                    (Some(mode_id_string.clone()), None)
                };

            // Spawn new process with appropriate flags
            let protocol_session_id = SessionId::new(parsed_session_id.to_string());
            if let Err(e) = self
                .claude_client
                .spawn_process_and_consume_init(
                    &parsed_session_id,
                    &protocol_session_id,
                    &cwd,
                    self.config.mcp_servers.clone(),
                    agent_mode,
                    system_prompt,
                )
                .await
            {
                tracing::error!("Failed to spawn new Claude process for mode change: {}", e);
                return Err(agent_client_protocol::Error::internal_error());
            }

            let current_mode_update =
                agent_client_protocol::CurrentModeUpdate::new(request.mode_id.clone());
            let update = SessionUpdate::CurrentModeUpdate(current_mode_update);

            // Store in session context for history replay
            let mode_message = crate::session::Message::from_update(update.clone());
            self.session_manager
                .update_session(&parsed_session_id, |session| {
                    session.add_message(mode_message);
                })
                .map_err(|_| agent_client_protocol::Error::internal_error())?;

            if let Err(e) = self
                .send_session_update(SessionNotification::new(request.session_id.clone(), update))
                .await
            {
                tracing::warn!("Failed to send current mode update notification: {}", e);
            }
        }

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("mode_set".to_string(), serde_json::json!(true));
        meta_map.insert(
            "message".to_string(),
            serde_json::json!("Session mode updated"),
        );
        meta_map.insert("mode_changed".to_string(), serde_json::json!(mode_changed));
        if mode_changed {
            meta_map.insert(
                "process_action".to_string(),
                serde_json::json!("process_replaced"),
            );
        }

        let response = SetSessionModeResponse::new().meta(meta_map);

        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        tracing::info!(
            "Processing prompt request for session: {}",
            request.session_id
        );

        // 🚨 DEBUG: Log exactly what prompt text we're receiving
        tracing::debug!("📨 PROMPT REQUEST DEBUG:");
        tracing::debug!("  Session: {}", request.session_id);
        tracing::debug!("  Content blocks: {}", request.prompt.len());
        for (i, block) in request.prompt.iter().enumerate() {
            match block {
                agent_client_protocol::ContentBlock::Text(text) => {
                    tracing::debug!("  Block {}: TEXT ({} chars)", i + 1, text.text.len());
                    tracing::debug!(
                        "  Text preview: {}",
                        if text.text.len() > 200 {
                            format!("{}...", &text.text[..200])
                        } else {
                            text.text.clone()
                        }
                    );
                }
                _ => {
                    tracing::debug!("  Block {}: {}", i + 1, Pretty(block));
                }
            }
        }

        // Validate the request
        self.validate_prompt_request(&request).await?;

        // Parse session ID
        let session_id = self.parse_session_id(&request.session_id)?;

        // ACP requires user message chunk updates for conversation transparency:
        // 1. Echo user input via session/update with user_message_chunk
        // 2. Send before agent processing begins
        // 3. Include all content blocks from user prompt
        // 4. Maintain conversation flow visibility for clients
        // 5. Support conversation history reconstruction
        //
        // User message chunks provide consistent conversation reporting.
        for content_block in &request.prompt {
            let content_chunk = agent_client_protocol::ContentChunk::new(content_block.clone());
            let notification = SessionNotification::new(
                request.session_id.clone(),
                SessionUpdate::UserMessageChunk(content_chunk),
            );

            if let Err(e) = self.send_session_update(notification).await {
                tracing::warn!(
                    "Failed to send user message chunk for session {}: {}",
                    request.session_id,
                    e
                );
            }
        }

        // Check if session is already cancelled before processing
        if self
            .cancellation_manager
            .is_cancelled(&session_id.to_string())
            .await
        {
            tracing::info!(
                "Session {} is cancelled, returning cancelled response",
                session_id
            );

            // CRITICAL: Reset cancellation state for next turn
            self.cancellation_manager
                .reset_for_new_turn(&session_id.to_string())
                .await;

            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_before_processing".to_string(),
                serde_json::json!(true),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        // Extract and process all content from the prompt
        let mut prompt_text = String::new();
        let mut has_binary_content = false;

        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                ContentBlock::Image(image_content) => {
                    // Add descriptive text for plan analysis
                    prompt_text.push_str(&format!(
                        "\n[Image content: {} ({})]",
                        image_content.mime_type,
                        if let Some(ref uri) = image_content.uri {
                            uri
                        } else {
                            "embedded data"
                        }
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Audio(audio_content) => {
                    // Add descriptive text for plan analysis
                    prompt_text.push_str(&format!(
                        "\n[Audio content: {} (embedded data)]",
                        audio_content.mime_type
                    ));
                    has_binary_content = true;
                }
                ContentBlock::Resource(_resource_content) => {
                    // Add descriptive text for the resource
                    prompt_text.push_str("\n[Embedded Resource]");
                    has_binary_content = true;
                }
                ContentBlock::ResourceLink(resource_link) => {
                    // Add descriptive text for the resource link
                    prompt_text.push_str(&format!("\n[Resource Link: {}]", resource_link.uri));
                    // ResourceLink is just a URI reference, not binary content
                }
                _ => {
                    // Unknown content block type, skip it
                    tracing::warn!("Unknown content block type, skipping");
                }
            }
        }

        if has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for plan analysis in session: {}",
                session_id
            );
        }

        // Validate session exists and get it
        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::invalid_params)?;

        // Reset turn counters at the start of each new turn.
        // ACP defines a turn as: a single user prompt and all subsequent LM requests
        // until the final response. This prevents unbounded counter growth across turns.
        self.session_manager
            .update_session(&session_id, |session| {
                session.reset_turn_counters();
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Add user message to session
        let user_message =
            crate::session::Message::new(crate::session::MessageRole::User, prompt_text.clone());

        self.session_manager
            .update_session(&session_id, |session| {
                session.add_message(user_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Get updated session for context
        let mut updated_session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::internal_error)?;

        // ACP requires specific stop reasons for all prompt turn completions:
        // 1. max_tokens: Token limit exceeded (configurable)
        // 2. max_turn_requests: Too many LM requests in single turn
        // Check limits before making Claude API calls

        // Check turn request limit
        let current_requests = updated_session.increment_turn_requests();
        if current_requests > self.config.max_turn_requests {
            tracing::info!(
                "Turn request limit exceeded ({} > {}) for session: {}",
                current_requests,
                self.config.max_turn_requests,
                session_id
            );
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "turn_requests".to_string(),
                serde_json::json!(current_requests),
            );
            meta_map.insert(
                "max_turn_requests".to_string(),
                serde_json::json!(self.config.max_turn_requests),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::MaxTurnRequests).meta(meta_map));
        }

        // Estimate token usage for the prompt (rough approximation: 4 chars per token)
        let estimated_tokens = (prompt_text.len() as u64) / 4;
        let current_tokens = updated_session.add_turn_tokens(estimated_tokens);
        if current_tokens > self.config.max_tokens_per_turn {
            tracing::info!(
                "Token limit exceeded ({} > {}) for session: {}",
                current_tokens,
                self.config.max_tokens_per_turn,
                session_id
            );
            let mut meta_map = serde_json::Map::new();
            meta_map.insert("turn_tokens".to_string(), serde_json::json!(current_tokens));
            meta_map.insert(
                "max_tokens_per_turn".to_string(),
                serde_json::json!(self.config.max_tokens_per_turn),
            );
            meta_map.insert(
                "session_id".to_string(),
                serde_json::json!(session_id.to_string()),
            );
            return Ok(PromptResponse::new(StopReason::MaxTokens).meta(meta_map));
        }

        // Update session with incremented counters
        self.session_manager
            .update_session(&session_id, |session| {
                session.turn_request_count = updated_session.turn_request_count;
                session.turn_token_count = updated_session.turn_token_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Check if streaming is supported and requested
        let response = if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        } else {
            self.handle_non_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        };

        // Clear cancellation state after turn completes successfully
        // This prepares for the next turn
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

        // Handle fs/read_text_file extension method
        if request.method == "fs/read_text_file".into() {
            // Validate client capabilities for file system read operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.fs.read_text_file => {
                        tracing::debug!("File system read capability validated");
                    }
                    Some(_) => {
                        tracing::error!("fs/read_text_file capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for fs/read_text_file validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse fs/read_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: ReadTextFileParams = serde_json::from_value(params_value).map_err(|e| {
                tracing::error!("Failed to deserialize fs/read_text_file parameters: {}", e);
                agent_client_protocol::Error::invalid_params()
            })?;

            // Handle the file reading request
            let response = self.handle_read_text_file(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle fs/write_text_file extension method
        if request.method == "fs/write_text_file".into() {
            // Validate client capabilities for file system write operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.fs.write_text_file => {
                        tracing::debug!("File system write capability validated");
                    }
                    Some(_) => {
                        tracing::error!("fs/write_text_file capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for fs/write_text_file validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse fs/write_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: WriteTextFileParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize fs/write_text_file parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the file writing request
            let response = self.handle_write_text_file(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/output extension method
        if request.method == "terminal/output".into() {
            // Validate client capabilities for terminal operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated");
                    }
                    Some(_) => {
                        tracing::error!("terminal/output capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/output validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/output parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/output parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal output request
            let response = self.handle_terminal_output(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/release extension method
        if request.method == "terminal/release".into() {
            // Validate client capabilities for terminal operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated");
                    }
                    Some(_) => {
                        tracing::error!("terminal/release capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/release validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/release parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalReleaseParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/release parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal release request
            let response = self.handle_terminal_release(params).await?;

            // Convert response to RawValue (should be null per ACP spec)
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/wait_for_exit extension method
        if request.method == "terminal/wait_for_exit".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for wait_for_exit");
                    }
                    Some(_) => {
                        tracing::error!("terminal/wait_for_exit capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/wait_for_exit validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/wait_for_exit parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!(
                        "Failed to deserialize terminal/wait_for_exit parameters: {}",
                        e
                    );
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the wait for exit request
            let response = self.handle_terminal_wait_for_exit(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/kill extension method
        if request.method == "terminal/kill".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for kill");
                    }
                    Some(_) => {
                        tracing::error!("terminal/kill capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/kill validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/kill parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalOutputParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/kill parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the kill request
            self.handle_terminal_kill(params).await?;

            // Return null result per ACP specification
            let response_json = serde_json::Value::Null;
            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle terminal/create extension method
        if request.method == "terminal/create".into() {
            // Validate terminal capability
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if caps.terminal => {
                        tracing::debug!("Terminal capability validated for create");
                    }
                    Some(_) => {
                        tracing::error!("terminal/create capability not declared by client");
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for terminal/create validation"
                        );
                        return Err(agent_client_protocol::Error::invalid_params());
                    }
                }
            }

            // Parse and validate parameters
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse terminal/create parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let params: crate::terminal_manager::TerminalCreateParams =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!("Failed to deserialize terminal/create parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Handle the terminal create request
            let response = self.handle_terminal_create(params).await?;

            // Convert response to RawValue
            let response_json = serde_json::to_value(response)
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Handle editor/update_buffers extension method
        //
        // This extension method allows clients to push editor buffer state to the agent,
        // enabling the agent to access unsaved file content when executing tools that read files.
        //
        // ## Protocol Integration
        //
        // This implements the ACP (Agent-Client Protocol) requirement for editor state management.
        // Clients should proactively push editor state updates when buffers change, allowing the
        // agent to work with current content rather than stale disk versions.
        //
        // ## Parameters
        //
        // Expects an `EditorBufferResponse` containing:
        // - `buffers`: HashMap of absolute file paths to EditorBuffer objects with content and metadata
        // - `unavailable_paths`: List of paths that don't have active editor buffers
        //
        // ## Returns
        //
        // Returns null on success per ACP specification for notifications.
        //
        // ## Client Usage Example
        //
        // ```typescript
        // await agent.ext_method({
        //   method: "editor/update_buffers",
        //   params: {
        //     buffers: {
        //       "/path/to/file.rs": {
        //         path: "/path/to/file.rs",
        //         content: "fn main() { ... }",
        //         modified: true,
        //         last_modified: { secs_since_epoch: 1234567890, nanos_since_epoch: 0 },
        //         encoding: "UTF-8"
        //       }
        //     },
        //     unavailable_paths: []
        //   }
        // });
        // ```
        if request.method == "editor/update_buffers".into() {
            // Validate client capabilities for editor state operations
            {
                let client_caps = self.client_capabilities.read().await;
                match &*client_caps {
                    Some(caps) if crate::editor_state::supports_editor_state(caps) => {
                        tracing::debug!("Editor state capability declared and validated");
                    }
                    Some(_) => {
                        tracing::error!("editor/update_buffers capability not declared by client");
                        return Err(agent_client_protocol::Error::new(
                            -32602,
                            "Editor state capability not declared by client. This feature requires client to support editor buffer synchronization.".to_string(),
                        ));
                    }
                    None => {
                        tracing::error!(
                            "No client capabilities available for editor/update_buffers validation"
                        );
                        return Err(agent_client_protocol::Error::new(
                            -32602,
                            "Client capabilities not initialized. Cannot perform editor operations without capability declaration.".to_string(),
                        ));
                    }
                }
            }

            // Parse the request parameters from RawValue
            let params_value: serde_json::Value = serde_json::from_str(request.params.get())
                .map_err(|e| {
                    tracing::error!("Failed to parse editor/update_buffers parameters: {}", e);
                    agent_client_protocol::Error::invalid_params()
                })?;

            let response: crate::editor_state::EditorBufferResponse =
                serde_json::from_value(params_value).map_err(|e| {
                    tracing::error!(
                        "Failed to deserialize editor/update_buffers parameters: {}",
                        e
                    );
                    agent_client_protocol::Error::invalid_params()
                })?;

            // Update the editor state manager cache
            tracing::info!(
                "Updating editor buffers cache with {} buffers",
                response.buffers.len()
            );
            self.editor_state_manager
                .update_buffers_from_response(response)
                .await;

            // Return success with null result
            let response_json = serde_json::Value::Null;
            let raw_value = RawValue::from_string(response_json.to_string())
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;

            return Ok(ExtResponse::new(Arc::from(raw_value)));
        }

        // Return a structured response indicating no other extensions are implemented
        // This maintains ACP compliance while clearly communicating capability limitations
        let response = serde_json::json!({
            "method": request.method,
            "result": "Extension method not implemented"
        });

        let raw_value = RawValue::from_string(response.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        Ok(ExtResponse::new(Arc::from(raw_value)))
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
