//! Prompt handling logic for Claude Agent
//!
//! This module contains the streaming and non-streaming prompt processing
//! logic extracted from the main agent module for maintainability.

use crate::content_capability_validator::ContentCapabilityValidator;
use agent_client_protocol::{
    ContentBlock, PromptRequest, PromptResponse, SessionId, SessionNotification, SessionUpdate,
    StopReason, TextContent,
};
use swissarmyhammer_common::Pretty;
use tokio_stream::StreamExt;

impl crate::agent::ClaudeAgent {
    /// Check if streaming is supported for this session
    pub(crate) fn should_stream(
        &self,
        session: &crate::session::Session,
        _request: &PromptRequest,
    ) -> bool {
        // Check if client supports streaming
        session
            .client_capabilities
            .as_ref()
            .and_then(|caps| caps.meta.as_ref())
            .and_then(|meta| meta.get("streaming"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Handle streaming prompt request
    pub(crate) async fn handle_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling streaming prompt for session: {}", session_id);

        // Validate content blocks against prompt capabilities before processing
        let content_validator =
            ContentCapabilityValidator::new(self.capabilities.prompt_capabilities.clone());
        if let Err(capability_error) = content_validator.validate_content_blocks(&request.prompt) {
            tracing::warn!(
                "Content capability validation failed for session {}: {}",
                session_id,
                capability_error
            );

            // Convert to ACP-compliant error response
            let acp_error_data = capability_error.to_acp_error();
            return Err(agent_client_protocol::Error::new(
                acp_error_data["code"].as_i64().unwrap_or(-32602) as i32,
                acp_error_data["message"]
                    .as_str()
                    .unwrap_or("Content capability validation failed")
                    .to_string(),
            )
            .data(acp_error_data["data"].clone()));
        }

        // Process all content blocks using the comprehensive processor
        let content_summary = self
            .content_block_processor
            .process_content_blocks(&request.prompt)
            .map_err(|e| {
                tracing::error!("Failed to process content blocks: {}", e);
                agent_client_protocol::Error::invalid_params()
            })?;

        let prompt_text = content_summary.combined_text;
        let has_binary_content = content_summary.has_binary_content;

        if has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for session: {}",
                session_id
            );
        }

        // ACP Compliance: Check turn request limit before making LM request
        // This mirrors the non-streaming path check (see handle_prompt around line 2833).
        // Currently each prompt() call is a new turn with only one LM request, but
        // when tool call loops are implemented, this will prevent infinite loops.
        let mut updated_session = session.clone();
        let current_requests = updated_session.increment_turn_requests();
        if current_requests > self.config.max_turn_requests {
            tracing::info!(
                "Turn request limit exceeded ({} > {}) for session: {} (streaming path)",
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
            meta_map.insert("streaming".to_string(), serde_json::json!(true));

            return Ok(PromptResponse::new(StopReason::MaxTurnRequests).meta(meta_map));
        }

        // Update session with incremented turn request counter
        self.session_manager
            .update_session(session_id, |s| {
                s.turn_request_count = updated_session.turn_request_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let context: crate::claude::SessionContext = session.into();

        // Mode handling happens in set_session_mode, not here
        // Just send the prompt to the existing Claude process
        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create streaming query: {}", e);
                agent_client_protocol::Error::internal_error()
            })?;

        let session_id_str = session_id.to_string();
        let mut claude_stop_reason: Option<String> = None;
        let mut accumulated_content = String::new();

        while let Some(chunk) = stream.next().await {
            // Check for cancellation
            if self
                .cancellation_manager
                .is_cancelled(&session_id_str)
                .await
            {
                tracing::info!("Streaming cancelled for session {}", session_id);

                // CRITICAL: Reset cancellation state for next turn
                self.cancellation_manager
                    .reset_for_new_turn(&session_id_str)
                    .await;

                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "cancelled_during_streaming".to_string(),
                    serde_json::json!(true),
                );
                return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
            }

            // Capture stop_reason
            if let Some(reason) = &chunk.stop_reason {
                claude_stop_reason = Some(reason.clone());
            }

            // Skip empty chunks
            if chunk.content.is_empty() && chunk.tool_call.is_none() {
                continue;
            }

            // Process this chunk
            if let Some(tool_call_info) = &chunk.tool_call {
                self.handle_streaming_tool_call(
                    session_id,
                    &session_id_str,
                    tool_call_info,
                    &mut accumulated_content,
                )
                .await?;
            } else if !chunk.content.is_empty() {
                // Accumulate content for logging
                accumulated_content.push_str(&chunk.content);

                // Create SessionUpdate for this chunk
                let update =
                    SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                        ContentBlock::Text(TextContent::new(chunk.content.clone())),
                    ));

                // Store in session
                let chunk_message = crate::session::Message::from_update(update.clone());

                self.session_manager
                    .update_session(session_id, |session| {
                        session.add_message(chunk_message);
                    })
                    .map_err(|_| agent_client_protocol::Error::internal_error())?;

                // Send chunk notification
                // Note: This may send redundantly with claude.rs notification_sender,
                // but the tracing layer handles deduplication for display purposes.
                let notification =
                    SessionNotification::new(SessionId::new(session_id_str.clone()), update);

                if let Err(e) = self.send_session_update(notification).await {
                    tracing::error!(
                        "Failed to send message chunk notification for session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        }

        // Check cancellation one final time
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!("Session {} cancelled after streaming", session_id);
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_after_streaming".to_string(),
                serde_json::json!(true),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        // Tool completions are emitted by protocol_translator when it detects
        // tool_result messages in Claude's stream

        // Map Claude's stop_reason to ACP StopReason
        let stop_reason = match claude_stop_reason.as_deref() {
            Some("max_tokens") => StopReason::MaxTokens,
            Some("end_turn") | None => StopReason::EndTurn,
            Some(other) => {
                tracing::debug!("Unknown stop_reason '{}', defaulting to EndTurn", other);
                StopReason::EndTurn
            }
        };

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("streaming".to_string(), serde_json::json!(true));
        Ok(PromptResponse::new(stop_reason).meta(meta_map))
    }

    /// Handle a tool call during streaming
    async fn handle_streaming_tool_call(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_info: &crate::claude::ToolCallInfo,
        _accumulated_content: &mut String,
    ) -> Result<(), agent_client_protocol::Error> {
        use agent_client_protocol::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

        let kind = if tool_call_info.name.to_lowercase().contains("read") {
            ToolKind::Read
        } else if tool_call_info.name.to_lowercase().contains("write")
            || tool_call_info.name.to_lowercase().contains("edit")
        {
            ToolKind::Edit
        } else if tool_call_info.name.to_lowercase().contains("bash")
            || tool_call_info.name.to_lowercase().contains("execute")
        {
            ToolKind::Execute
        } else {
            ToolKind::Other
        };

        let update = SessionUpdate::ToolCall(
            ToolCall::new(
                ToolCallId::new(std::sync::Arc::from(tool_call_info.id.clone())),
                tool_call_info.name.clone(),
            )
            .kind(kind)
            .status(ToolCallStatus::Pending)
            .raw_input(tool_call_info.parameters.clone()),
        );

        // Store in session context for history replay
        let tool_call_message = crate::session::Message::from_update(update.clone());
        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(tool_call_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let notification =
            SessionNotification::new(SessionId::new(session_id_str.to_string()), update);

        if let Err(e) = self.send_session_update(notification).await {
            tracing::error!(
                "Failed to send tool call notification for session {}: {}",
                session_id,
                e
            );
        }

        // Handle tool call with permission checks
        self.handle_tool_permission_check(session_id, session_id_str, tool_call_info)
            .await?;

        // Check if this is a TodoWrite tool call and send Plan notification
        if tool_call_info.name == "TodoWrite" {
            self.handle_todowrite_plan_notification(session_id, session_id_str, tool_call_info)
                .await;
        }

        Ok(())
    }

    /// Handle permission check for a tool call
    async fn handle_tool_permission_check(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_info: &crate::claude::ToolCallInfo,
    ) -> Result<(), agent_client_protocol::Error> {
        use crate::permissions::PolicyEvaluation;

        let tool_call_id = tool_call_info.id.clone();
        let tool_name = tool_call_info.name.clone();
        let tool_params = tool_call_info.parameters.clone();

        // Check permissions
        let policy_eval = self
            .permission_engine
            .evaluate_tool_call(&tool_name, &tool_params)
            .await
            .map_err(|e| {
                tracing::error!("Permission evaluation failed: {}", e);
                agent_client_protocol::Error::internal_error()
            })?;

        match policy_eval {
            PolicyEvaluation::Allowed => {
                tracing::debug!("Tool call '{}' allowed by policy, executing", tool_name);
                // Execute tool immediately
                // TODO: Call tool handler to execute the tool
            }
            PolicyEvaluation::Denied { reason } => {
                tracing::warn!("Tool call '{}' denied by policy: {}", tool_name, reason);
                // TODO: Send tool completion with error status
            }
            PolicyEvaluation::RequireUserConsent { options } => {
                tracing::info!("Tool call '{}' requires user consent", tool_name);

                // Check if there's a stored preference for this tool
                if let Some(stored_kind) = self.permission_storage.get_preference(&tool_name).await
                {
                    self.handle_stored_permission_preference(&tool_name, stored_kind);
                } else if let Some(ref client) = self.client {
                    self.request_user_permission(
                        session_id,
                        session_id_str,
                        &tool_call_id,
                        &tool_name,
                        client,
                        &options,
                    )
                    .await;
                } else {
                    tracing::warn!(
                        "Permission required for tool '{}' but no client connection available",
                        tool_name
                    );
                    // TODO: Send tool completion with error status
                }
            }
        }

        Ok(())
    }

    /// Handle a stored permission preference
    fn handle_stored_permission_preference(
        &self,
        tool_name: &str,
        stored_kind: crate::tools::PermissionOptionKind,
    ) {
        let should_allow = match stored_kind {
            crate::tools::PermissionOptionKind::AllowAlways => true,
            crate::tools::PermissionOptionKind::RejectAlways => false,
            _ => {
                tracing::warn!("Unexpected stored permission kind: {:?}", stored_kind);
                false
            }
        };

        if should_allow {
            tracing::info!("Using stored 'allow' preference for '{}'", tool_name);
            // TODO: Call tool handler to execute the tool
        } else {
            tracing::info!("Using stored 'reject' preference for '{}'", tool_name);
            // TODO: Send tool completion with error status
        }
    }

    /// Request user permission for a tool call
    async fn request_user_permission(
        &self,
        _session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_id: &str,
        tool_name: &str,
        client: &std::sync::Arc<dyn agent_client_protocol::Client + Send + Sync>,
        options: &[crate::tools::PermissionOption],
    ) {
        // Convert our internal types to ACP protocol types
        let acp_options: Vec<agent_client_protocol::PermissionOption> = options
            .iter()
            .map(|opt| {
                let kind = match opt.kind {
                    crate::tools::PermissionOptionKind::AllowOnce => {
                        agent_client_protocol::PermissionOptionKind::AllowOnce
                    }
                    crate::tools::PermissionOptionKind::AllowAlways => {
                        agent_client_protocol::PermissionOptionKind::AllowAlways
                    }
                    crate::tools::PermissionOptionKind::RejectOnce => {
                        agent_client_protocol::PermissionOptionKind::RejectOnce
                    }
                    crate::tools::PermissionOptionKind::RejectAlways => {
                        agent_client_protocol::PermissionOptionKind::RejectAlways
                    }
                };
                agent_client_protocol::PermissionOption::new(
                    agent_client_protocol::PermissionOptionId::new(opt.option_id.as_str()),
                    opt.name.clone(),
                    kind,
                )
            })
            .collect();

        let tool_call_update = agent_client_protocol::ToolCallUpdate::new(
            agent_client_protocol::ToolCallId::new(tool_call_id),
            agent_client_protocol::ToolCallUpdateFields::new(),
        );

        let acp_request = agent_client_protocol::RequestPermissionRequest::new(
            SessionId::new(session_id_str.to_string()),
            tool_call_update,
            acp_options,
        );

        match client.request_permission(acp_request).await {
            Ok(response) => {
                self.handle_permission_response(tool_name, response, options)
                    .await;
            }
            Err(e) => {
                tracing::error!("Failed to request permission from client: {}", e);
                // TODO: Send tool completion with error status
            }
        }
    }

    /// Handle the permission response from the client
    async fn handle_permission_response(
        &self,
        tool_name: &str,
        response: agent_client_protocol::RequestPermissionResponse,
        options: &[crate::tools::PermissionOption],
    ) {
        match response.outcome {
            agent_client_protocol::RequestPermissionOutcome::Cancelled => {
                tracing::info!("Permission request cancelled for '{}'", tool_name);
                // TODO: Send tool completion with cancelled status
            }
            agent_client_protocol::RequestPermissionOutcome::Selected(selected) => {
                let option_id_str = selected.option_id.0.to_string();

                // Store preference if it's an "always" decision
                if let Some(option) = options.iter().find(|opt| opt.option_id == option_id_str) {
                    self.permission_storage
                        .store_preference(tool_name, option.kind.clone())
                        .await;
                }

                // Check if the selected option allows execution
                let should_allow = option_id_str.starts_with("allow");

                if should_allow {
                    tracing::info!("Permission granted for '{}'", tool_name);
                    // TODO: Call tool handler to execute the tool
                } else {
                    tracing::info!("Permission denied for '{}'", tool_name);
                    // TODO: Send tool completion with error status
                }
            }
            _ => {
                tracing::warn!("Unknown permission outcome for '{}'", tool_name);
                // TODO: Send tool completion with error status
            }
        }
    }

    /// Handle TodoWrite tool call and send Plan notification
    async fn handle_todowrite_plan_notification(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_info: &crate::claude::ToolCallInfo,
    ) {
        match crate::plan::todowrite_to_agent_plan(&tool_call_info.parameters) {
            Ok(agent_plan) => {
                let acp_plan = agent_plan.to_acp_plan();
                let plan_update = SessionUpdate::Plan(acp_plan);

                // Store/update plan in PlanManager for status tracking
                // This preserves entry IDs when updating existing plans
                {
                    let mut plan_manager = self.plan_manager.write().await;
                    plan_manager.update_plan(&session_id.to_string(), agent_plan);
                }

                // Store in session context for history replay
                let plan_message = crate::session::Message::from_update(plan_update.clone());
                if self
                    .session_manager
                    .update_session(session_id, |session| {
                        session.add_message(plan_message);
                    })
                    .is_err()
                {
                    tracing::error!(
                        "Failed to store plan message in session {} context",
                        session_id
                    );
                }

                let plan_notification = SessionNotification::new(
                    SessionId::new(session_id_str.to_string()),
                    plan_update,
                );

                if let Err(e) = self.send_session_update(plan_notification).await {
                    tracing::error!(
                        "Failed to send Plan notification from TodoWrite for session {}: {}",
                        session_id,
                        e
                    );
                } else {
                    tracing::debug!(
                        "Sent Plan notification from TodoWrite for session {}",
                        session_id
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to convert TodoWrite to Plan for session {}: {}",
                    session_id,
                    e
                );
            }
        }
    }

    /// Handle non-streaming prompt request
    pub(crate) async fn handle_non_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling non-streaming prompt for session: {}", session_id);

        // Validate content blocks against prompt capabilities before processing
        let content_validator =
            ContentCapabilityValidator::new(self.capabilities.prompt_capabilities.clone());
        if let Err(capability_error) = content_validator.validate_content_blocks(&request.prompt) {
            tracing::warn!(
                "Content capability validation failed for session {}: {}",
                session_id,
                capability_error
            );

            // Convert to ACP-compliant error response
            let acp_error_data = capability_error.to_acp_error();
            return Err(agent_client_protocol::Error::new(
                acp_error_data["code"].as_i64().unwrap_or(-32602) as i32,
                acp_error_data["message"]
                    .as_str()
                    .unwrap_or("Content capability validation failed")
                    .to_string(),
            )
            .data(acp_error_data["data"].clone()));
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
                    // Process image data (already validated in validate_prompt_request)
                    let _decoded = self
                        .base64_processor
                        .decode_image_data(&image_content.data, &image_content.mime_type)
                        .map_err(|e| {
                            tracing::error!("Failed to decode image data: {}", e);
                            agent_client_protocol::Error::invalid_params()
                        })?;

                    // Add descriptive text for now until full multimodal support
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
                    // Process audio data (already validated in validate_prompt_request)
                    let _decoded = self
                        .base64_processor
                        .decode_audio_data(&audio_content.data, &audio_content.mime_type)
                        .map_err(|e| {
                            tracing::error!("Failed to decode audio data: {}", e);
                            agent_client_protocol::Error::invalid_params()
                        })?;

                    // Add descriptive text for now until full multimodal support
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
                "Processing prompt with binary content for session: {}",
                session_id
            );
        }

        let context: crate::claude::SessionContext = session.into();
        let session_id_str = session_id.to_string();

        // Check for cancellation before making Claude API request
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!("Session {} cancelled before Claude API request", session_id);
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_before_api_request".to_string(),
                serde_json::json!(true),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        tracing::info!("Calling Claude API for session: {}", session_id);

        // Mode handling happens in set_session_mode, not here
        // Just send the prompt to the existing Claude process
        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context)
            .await
            .map_err(|e| {
                tracing::error!("Claude API error: {}", Pretty(&e));
                agent_client_protocol::Error::internal_error()
            })?;

        let mut response_content = String::new();
        let mut chunk_count = 0;

        while let Some(chunk) = futures::StreamExt::next(&mut stream).await {
            // Check for cancellation during response
            if self
                .cancellation_manager
                .is_cancelled(&session_id_str)
                .await
            {
                tracing::info!(
                    "Session {} cancelled during Claude API response",
                    session_id
                );
                let mut meta_map = serde_json::Map::new();
                meta_map.insert(
                    "cancelled_during_api_response".to_string(),
                    serde_json::json!(true),
                );
                meta_map.insert(
                    "partial_response_length".to_string(),
                    serde_json::json!(response_content.len()),
                );
                return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
            }

            chunk_count += 1;
            response_content.push_str(&chunk.content);

            // Handle tool calls and send notifications
            let update = if let Some(tool_call_info) = &chunk.tool_call {
                use agent_client_protocol::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

                // Infer tool kind from name
                let kind = if tool_call_info.name.to_lowercase().contains("read") {
                    ToolKind::Read
                } else if tool_call_info.name.to_lowercase().contains("write")
                    || tool_call_info.name.to_lowercase().contains("edit")
                {
                    ToolKind::Edit
                } else if tool_call_info.name.to_lowercase().contains("bash")
                    || tool_call_info.name.to_lowercase().contains("execute")
                {
                    ToolKind::Execute
                } else {
                    ToolKind::Other
                };

                SessionUpdate::ToolCall(
                    ToolCall::new(
                        ToolCallId::new(format!("tool_{}", chunk_count)),
                        tool_call_info.name.clone(),
                    )
                    .kind(kind)
                    .status(ToolCallStatus::Pending)
                    .raw_input(tool_call_info.parameters.clone()),
                )
            } else if !chunk.content.is_empty() {
                // Send text chunk notification
                SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                    ContentBlock::Text(TextContent::new(chunk.content.clone())),
                ))
            } else {
                continue; // Skip empty chunks
            };

            // Store in session context for history replay
            let message = crate::session::Message::from_update(update.clone());
            self.session_manager
                .update_session(session_id, |session| {
                    session.add_message(message);
                })
                .map_err(|_| agent_client_protocol::Error::internal_error())?;

            let notification =
                SessionNotification::new(SessionId::new(session_id_str.clone()), update);

            // Send notification
            if let Err(e) = self.send_session_update(notification).await {
                tracing::error!(
                    "Failed to send notification for session {}: {}",
                    session_id,
                    e
                );
            }

            // Check if this is a TodoWrite tool call and send Plan notification
            if let Some(tool_call_info) = &chunk.tool_call {
                if tool_call_info.name == "TodoWrite" {
                    self.handle_todowrite_plan_notification(
                        session_id,
                        &session_id_str,
                        tool_call_info,
                    )
                    .await;
                }
            }
        }

        tracing::info!(
            "Received Claude API response ({} bytes, {} chunks) for session: {}",
            response_content.len(),
            chunk_count,
            session_id
        );

        // ACP requires specific stop reasons for all prompt turn completions:
        // Check for refusal patterns in Claude's response content
        if self.is_response_refusal(&response_content) {
            tracing::info!(
                "Claude refused to respond for session: {}. Response: {}",
                session_id,
                response_content
            );
            return Ok(self.create_refusal_response(&session_id.to_string(), false, None));
        }

        // Check for cancellation after Claude API request but before storing
        if self
            .cancellation_manager
            .is_cancelled(&session_id_str)
            .await
        {
            tracing::info!(
                "Session {} cancelled after Claude API response, not storing",
                session_id
            );
            let mut meta = serde_json::Map::new();
            meta.insert(
                "cancelled_after_api_response".to_string(),
                serde_json::json!(true),
            );
            meta.insert(
                "response_length".to_string(),
                serde_json::json!(response_content.len()),
            );

            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta));
        }

        // Store assistant response in session
        let assistant_message = crate::session::Message::new(
            crate::session::MessageRole::Assistant,
            response_content.clone(),
        );

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(assistant_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let mut meta = serde_json::Map::new();
        meta.insert("processed".to_string(), serde_json::json!(true));
        meta.insert("streaming".to_string(), serde_json::json!(false));
        meta.insert(
            "claude_response".to_string(),
            serde_json::json!(response_content),
        );
        meta.insert(
            "session_messages".to_string(),
            serde_json::json!(session.context.len() + 1),
        );

        Ok(PromptResponse::new(StopReason::EndTurn).meta(meta))
    }
}
