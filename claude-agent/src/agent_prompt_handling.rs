//! Prompt handling logic for Claude Agent
//!
//! This module contains the streaming and non-streaming prompt processing
//! logic extracted from the main agent module for maintainability.

use crate::content_capability_validator::ContentCapabilityValidator;
use agent_client_protocol::schema::{
    ContentBlock, PromptRequest, PromptResponse, SessionId, SessionNotification, SessionUpdate,
    StopReason, TextContent,
};
use swissarmyhammer_common::Pretty;
use tokio_stream::StreamExt;

/// Approximate generation tokens contributed by a streamed text chunk.
///
/// Claude Code's stream-json protocol does not annotate text deltas with a
/// per-chunk token count (only the final `result` message carries an
/// authoritative `usage.output_tokens`). To enforce a per-turn output cap as
/// the response streams in, we estimate tokens from byte length using the
/// same `len / 4` heuristic that the rest of `claude-agent` uses for input
/// estimation (see [`crate::agent::ClaudeAgent::check_turn_limits`]).
///
/// The estimate is intentionally pessimistic at the top of the function:
/// a chunk shorter than one estimated token still counts as one, so very
/// chatty whitespace-heavy streams cannot slip past the cap by emitting many
/// sub-token chunks.
///
/// # Arguments
///
/// * `text` - The chunk content as emitted by the Claude CLI.
fn estimate_chunk_tokens(text: &str) -> u64 {
    let bytes = text.len() as u64;
    if bytes == 0 {
        0
    } else {
        // `len / 4` underestimates for short non-empty chunks. Floor at 1 so
        // a stream of single-byte deltas still accumulates against the cap.
        std::cmp::max(1, bytes / 4)
    }
}

/// Extract the caller-supplied per-turn generation cap from a `PromptRequest`'s
/// `_meta` map.
///
/// The ACP `PromptRequest` schema has no first-class `max_tokens` field; the
/// validator runner attaches its per-rule defense-in-depth cap via the
/// extensibility `_meta` map under the key `"max_tokens"`. This function
/// returns `Some(n)` only when the value is a positive integer that fits in
/// `u64`. Returns `None` for all other cases (key missing, value not an
/// integer, value zero, value negative, or meta itself is `None`). Callers
/// treat `None` as "no caller-supplied cap" and fall back to the agent's own
/// `max_tokens_per_turn` config.
///
/// # Why a free function
///
/// Pulled out of the prompt loop so the parsing logic is unit-testable
/// without standing up a real `ClaudeAgent` (which spawns a subprocess). The
/// behavior is pure JSON inspection — no I/O, no async. This mirrors the
/// twin helper in `llama-agent::acp::server::extract_request_max_tokens`.
pub(crate) fn extract_request_max_tokens(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<u64> {
    let value = meta?.get("max_tokens")?;
    let raw = value.as_u64()?;
    if raw == 0 {
        return None;
    }
    Some(raw)
}

/// Compute the effective per-turn generation cap by intersecting the agent's
/// configured `max_tokens_per_turn` with any caller-supplied cap from
/// `request.meta`.
///
/// The caller can only *narrow* the cap, never widen it. This guarantees that
/// honoring `_meta.max_tokens` cannot be used to bypass operator-configured
/// safety limits.
///
/// # Arguments
///
/// * `config_max` - The agent's configured `max_tokens_per_turn`. Always wins
///   when no caller cap is present.
/// * `requested` - The caller-supplied cap from `request.meta.max_tokens`, if
///   any.
///
/// Returns the effective cap that applies to streamed output tokens.
fn effective_generation_cap(config_max: u64, requested: Option<u64>) -> u64 {
    match requested {
        Some(n) => std::cmp::min(config_max, n),
        None => config_max,
    }
}

/// Per-turn output-cap parameters shared by both prompt-dispatch paths.
///
/// Bundled into a `Copy` struct so [`ClaudeAgent::check_output_token_cap`]
/// stays under clippy's argument count without losing the named fields at the
/// call sites. The values are turn-scoped invariants — they don't change as
/// chunks stream in, so passing the same struct through every chunk iteration
/// is free.
///
/// * `effective` — The cap that actually applies to this turn (intersection
///   of the agent's configured cap and any caller-supplied cap).
/// * `caller_supplied` — The raw `request.meta.max_tokens` value, if any.
///   Recorded in the response meta so callers can distinguish their cap
///   firing from the agent's configured cap firing.
/// * `streaming` — Which dispatch path is enforcing the cap. Forwarded into
///   the response meta on the `streaming` key for symmetry with the per-path
///   success responses.
#[derive(Debug, Clone, Copy)]
struct OutputCap {
    effective: u64,
    caller_supplied: Option<u64>,
    streaming: bool,
}

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

    /// Handle streaming prompt request.
    ///
    /// `requested_max_tokens` carries the optional per-request generation cap
    /// from `PromptRequest.meta.max_tokens`. When present, it narrows (but
    /// never widens) the agent's configured `max_tokens_per_turn`. Hitting the
    /// resulting effective cap aborts streaming and yields
    /// `StopReason::MaxTokens`.
    pub(crate) async fn handle_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
        requested_max_tokens: Option<u64>,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling streaming prompt for session: {}", session_id);

        self.validate_streaming_content(session_id, request)?;
        let prompt_text = self.process_streaming_content(session_id, request)?;

        if let Some(response) = self.check_turn_request_limit_streaming(session_id, session)? {
            return Ok(response);
        }

        let context: crate::claude::SessionContext = session.into();
        let mut stream = self.create_streaming_query(&prompt_text, &context).await?;

        let effective_cap =
            effective_generation_cap(self.config.max_tokens_per_turn, requested_max_tokens);
        self.process_stream_chunks(session_id, &mut stream, effective_cap, requested_max_tokens)
            .await
    }

    /// Validate content blocks for streaming prompt.
    fn validate_streaming_content(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        let content_validator =
            ContentCapabilityValidator::new(self.capabilities.prompt_capabilities.clone());

        if let Err(capability_error) = content_validator.validate_content_blocks(&request.prompt) {
            tracing::warn!(
                "Content capability validation failed for session {}: {}",
                session_id,
                capability_error
            );
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
        Ok(())
    }

    /// Process content blocks and extract prompt text.
    fn process_streaming_content(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
    ) -> Result<String, agent_client_protocol::Error> {
        let content_summary = self
            .content_block_processor
            .process_content_blocks(&request.prompt)
            .map_err(|e| {
                tracing::error!("Failed to process content blocks: {}", e);
                agent_client_protocol::Error::invalid_params()
            })?;

        if content_summary.has_binary_content {
            tracing::info!(
                "Processing prompt with binary content for session: {}",
                session_id
            );
        }

        Ok(content_summary.combined_text)
    }

    /// Check turn request limit for streaming path.
    fn check_turn_request_limit_streaming(
        &self,
        session_id: &crate::session::SessionId,
        session: &crate::session::Session,
    ) -> Result<Option<PromptResponse>, agent_client_protocol::Error> {
        let mut updated_session = session.clone();
        let current_requests = updated_session.increment_turn_requests();

        if current_requests > self.config.max_turn_requests {
            tracing::info!(
                "Turn request limit exceeded ({} > {}) for session: {} (streaming path)",
                current_requests,
                self.config.max_turn_requests,
                session_id
            );
            return Ok(Some(PromptResponse::new(StopReason::MaxTurnRequests).meta(
                self.build_turn_limit_meta(session_id, current_requests, true),
            )));
        }

        self.session_manager
            .update_session(session_id, |s| {
                s.turn_request_count = updated_session.turn_request_count;
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        Ok(None)
    }

    /// Build metadata for turn limit exceeded response.
    fn build_turn_limit_meta(
        &self,
        session_id: &crate::session::SessionId,
        current_requests: u64,
        streaming: bool,
    ) -> serde_json::Map<String, serde_json::Value> {
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
        meta_map.insert("streaming".to_string(), serde_json::json!(streaming));
        meta_map
    }

    /// Create a streaming query to the Claude process.
    async fn create_streaming_query(
        &self,
        prompt_text: &str,
        context: &crate::claude::SessionContext,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = crate::claude::MessageChunk> + Send>>,
        agent_client_protocol::Error,
    > {
        self.claude_client
            .query_stream_with_context(prompt_text, context)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create streaming query: {}", e);
                agent_client_protocol::Error::internal_error()
            })
    }

    /// Process stream chunks and build response.
    ///
    /// Enforces a per-turn output cap by accumulating an estimated token count
    /// across streamed text deltas (see [`estimate_chunk_tokens`]) and aborting
    /// the stream when `effective_cap` is exceeded. Hitting the cap returns
    /// `StopReason::MaxTokens` with `meta` describing the trigger; this is the
    /// signal the validator runner converts into a loud rule failure.
    ///
    /// `caller_supplied_cap` is the raw `request.meta.max_tokens` value (if
    /// any) — recorded in the response meta so callers can distinguish their
    /// cap firing from the agent's configured cap firing.
    pub(crate) async fn process_stream_chunks(
        &self,
        session_id: &crate::session::SessionId,
        stream: &mut std::pin::Pin<
            Box<dyn futures::Stream<Item = crate::claude::MessageChunk> + Send>,
        >,
        effective_cap: u64,
        caller_supplied_cap: Option<u64>,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        let session_id_str = session_id.to_string();
        let mut claude_stop_reason: Option<String> = None;
        let mut accumulated_content = String::new();
        let mut output_tokens: u64 = 0;

        while let Some(chunk) = stream.next().await {
            if let Some(response) = self
                .check_streaming_cancellation(session_id, &session_id_str)
                .await
            {
                return Ok(response);
            }

            if let Some(reason) = &chunk.stop_reason {
                claude_stop_reason = Some(reason.clone());
            }

            if chunk.content.is_empty() && chunk.tool_call.is_none() {
                continue;
            }

            self.process_single_chunk(
                session_id,
                &session_id_str,
                &chunk,
                &mut accumulated_content,
            )
            .await?;

            // Accumulate output tokens against the effective cap. We count
            // text deltas only (tool-call chunks are protocol metadata, not
            // generated content). If the cap fires, abort the underlying
            // claude subprocess and return `MaxTokens` immediately so the
            // model can't keep generating off-screen.
            if let Some(response) = self
                .check_output_token_cap(
                    session_id,
                    &session_id_str,
                    &mut output_tokens,
                    &chunk.content,
                    OutputCap {
                        effective: effective_cap,
                        caller_supplied: caller_supplied_cap,
                        streaming: true,
                    },
                )
                .await
            {
                return Ok(response);
            }
        }

        self.build_streaming_response(&session_id_str, claude_stop_reason)
            .await
    }

    /// Abort the in-flight generation when the per-turn output cap fires.
    ///
    /// Terminating the underlying claude subprocess is the only reliable way
    /// to stop generation — the CLI does not accept a per-turn `--max-tokens`
    /// flag, so we cannot ask the model to stop politely. The session's
    /// process is dropped; the next prompt on this session will spawn a fresh
    /// process via the normal session lifecycle.
    ///
    /// Path-agnostic: invoked from both the streaming path
    /// (`process_stream_chunks`) and the non-streaming path
    /// (`handle_non_streaming_prompt`). The underlying termination mechanics
    /// are identical regardless of which path observed the chunk that pushed
    /// `output_tokens` past the cap.
    async fn abort_for_output_max_tokens(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
    ) {
        // Mark cancellation state so any concurrent observers see the abort
        // and don't keep producing output for this turn.
        if let Err(e) = self
            .cancellation_manager
            .mark_cancelled(session_id_str, "max_tokens cap exceeded")
            .await
        {
            tracing::warn!(
                "Failed to mark session {} cancelled on max_tokens abort: {}",
                session_id,
                e
            );
        }

        // Kill the claude subprocess so it stops streaming output we're not
        // going to consume. Errors here are non-fatal — even if termination
        // fails, the channel-drop below will still stop us from reading.
        let process_manager = self.claude_client.process_manager();
        if let Err(e) = process_manager.terminate_session(session_id).await {
            tracing::debug!(
                "Could not terminate claude process for session {} after max_tokens abort: {}",
                session_id,
                e
            );
        }
    }

    /// Accumulate `chunk_content` against the per-turn output cap and, if the
    /// cap fires, abort the underlying generation and build the
    /// `StopReason::MaxTokens` response.
    ///
    /// This is the shared cap-enforcement step used by both
    /// `process_stream_chunks` (streaming path) and the chunk loop in
    /// `handle_non_streaming_prompt` (non-streaming path). Both call sites
    /// count text deltas only — tool-call chunks are protocol metadata, not
    /// generated content, and pass an empty `chunk_content` here implicitly
    /// (callers gate on `chunk.content`).
    ///
    /// Returns `Some(response)` when the cap fired and the caller should
    /// short-circuit with that `MaxTokens` response. Returns `None` when the
    /// running total still fits under `cap.effective` and the caller should
    /// continue consuming chunks.
    async fn check_output_token_cap(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        output_tokens: &mut u64,
        chunk_content: &str,
        cap: OutputCap,
    ) -> Option<PromptResponse> {
        *output_tokens = output_tokens.saturating_add(estimate_chunk_tokens(chunk_content));
        if *output_tokens <= cap.effective {
            return None;
        }
        tracing::warn!(
            "{} generation hit max_tokens cap ({} > {}) for session {}; aborting",
            if cap.streaming {
                "Streaming"
            } else {
                "Non-streaming"
            },
            output_tokens,
            cap.effective,
            session_id
        );
        self.abort_for_output_max_tokens(session_id, session_id_str)
            .await;
        Some(self.build_output_max_tokens_response(
            session_id_str,
            *output_tokens,
            cap.effective,
            cap.caller_supplied,
            cap.streaming,
        ))
    }

    /// Build a `StopReason::MaxTokens` response for generation that hit the
    /// per-turn output-token cap.
    ///
    /// `caller_supplied_cap` is recorded only when the caller sent one via
    /// `_meta.max_tokens` — so the validator runner can distinguish its
    /// defense-in-depth cap firing from the agent's own configured limit.
    ///
    /// `streaming` records which dispatch path observed the cap firing so
    /// callers can correlate with the per-path success responses (which set
    /// `streaming: true`/`false` on the same key). Path-agnostic in behavior
    /// — only the metadata distinguishes the two call sites.
    fn build_output_max_tokens_response(
        &self,
        session_id_str: &str,
        output_tokens: u64,
        effective_cap: u64,
        caller_supplied_cap: Option<u64>,
        streaming: bool,
    ) -> PromptResponse {
        let mut meta = serde_json::Map::new();
        meta.insert("streaming".to_string(), serde_json::json!(streaming));
        meta.insert(
            "session_id".to_string(),
            serde_json::json!(session_id_str.to_string()),
        );
        meta.insert(
            "output_tokens".to_string(),
            serde_json::json!(output_tokens),
        );
        meta.insert(
            "effective_max_tokens".to_string(),
            serde_json::json!(effective_cap),
        );
        meta.insert(
            "max_tokens_per_turn".to_string(),
            serde_json::json!(self.config.max_tokens_per_turn),
        );
        if let Some(requested) = caller_supplied_cap {
            meta.insert(
                "requested_max_tokens".to_string(),
                serde_json::json!(requested),
            );
        }
        PromptResponse::new(StopReason::MaxTokens).meta(meta)
    }

    /// Check for cancellation during streaming.
    async fn check_streaming_cancellation(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
    ) -> Option<PromptResponse> {
        if self.cancellation_manager.is_cancelled(session_id_str).await {
            tracing::info!("Streaming cancelled for session {}", session_id);
            self.cancellation_manager
                .reset_for_new_turn(session_id_str)
                .await;

            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_during_streaming".to_string(),
                serde_json::json!(true),
            );
            return Some(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }
        None
    }

    /// Process a single stream chunk.
    async fn process_single_chunk(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        chunk: &crate::claude::MessageChunk,
        accumulated_content: &mut String,
    ) -> Result<(), agent_client_protocol::Error> {
        if let Some(tool_call_info) = &chunk.tool_call {
            self.handle_streaming_tool_call(
                session_id,
                session_id_str,
                tool_call_info,
                accumulated_content,
            )
            .await?;
        } else if !chunk.content.is_empty() {
            self.handle_streaming_text_chunk(
                session_id,
                session_id_str,
                chunk,
                accumulated_content,
            )
            .await?;
        }
        Ok(())
    }

    /// Handle a text chunk during streaming.
    async fn handle_streaming_text_chunk(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        chunk: &crate::claude::MessageChunk,
        accumulated_content: &mut String,
    ) -> Result<(), agent_client_protocol::Error> {
        accumulated_content.push_str(&chunk.content);

        let update =
            SessionUpdate::AgentMessageChunk(agent_client_protocol::schema::ContentChunk::new(
                ContentBlock::Text(TextContent::new(chunk.content.clone())),
            ));

        let chunk_message = crate::session::Message::from_update(update.clone());

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(chunk_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let notification =
            SessionNotification::new(SessionId::new(session_id_str.to_string()), update);

        if let Err(e) = self.send_session_update(notification).await {
            tracing::error!(
                "Failed to send message chunk notification for session {}: {}",
                session_id,
                e
            );
        }
        Ok(())
    }

    /// Build final streaming response with stop reason.
    async fn build_streaming_response(
        &self,
        session_id_str: &str,
        claude_stop_reason: Option<String>,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        if self.cancellation_manager.is_cancelled(session_id_str).await {
            tracing::info!("Session {} cancelled after streaming", session_id_str);
            let mut meta_map = serde_json::Map::new();
            meta_map.insert(
                "cancelled_after_streaming".to_string(),
                serde_json::json!(true),
            );
            return Ok(PromptResponse::new(StopReason::Cancelled).meta(meta_map));
        }

        let stop_reason = Self::map_claude_stop_reason(claude_stop_reason);
        let mut meta_map = serde_json::Map::new();
        meta_map.insert("streaming".to_string(), serde_json::json!(true));
        Ok(PromptResponse::new(stop_reason).meta(meta_map))
    }

    /// Map Claude's stop_reason to ACP StopReason.
    fn map_claude_stop_reason(claude_stop_reason: Option<String>) -> StopReason {
        match claude_stop_reason.as_deref() {
            Some("max_tokens") => StopReason::MaxTokens,
            Some("end_turn") | None => StopReason::EndTurn,
            Some(other) => {
                tracing::debug!("Unknown stop_reason '{}', defaulting to EndTurn", other);
                StopReason::EndTurn
            }
        }
    }

    /// Handle a tool call during streaming
    async fn handle_streaming_tool_call(
        &self,
        session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_info: &crate::claude::ToolCallInfo,
        _accumulated_content: &mut String,
    ) -> Result<(), agent_client_protocol::Error> {
        use agent_client_protocol::schema::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

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
    ///
    /// Sends an ACP `session/request_permission` request back to the client over
    /// the provided connection, awaits the response without blocking the event
    /// loop, and dispatches the outcome to [`Self::handle_permission_response`].
    ///
    /// The `client` parameter is a `ConnectionTo<Client>`: the agent-side handle
    /// to the JSON-RPC connection whose counterpart is the `Client` role.
    /// Requests flow through `ConnectionTo::send_request`.
    ///
    /// # Arguments
    ///
    /// * `_session_id` - Internal session identifier (currently unused; retained
    ///   for symmetry with other permission helpers and future tool-completion
    ///   wiring).
    /// * `session_id_str` - Wire-format ACP session id to attach to the
    ///   permission request.
    /// * `tool_call_id` - The ACP tool call id this permission gates.
    /// * `tool_name` - Human-readable tool name used for logging and stored
    ///   preference lookups.
    /// * `client` - Agent-side connection used to dispatch the permission
    ///   request to the client.
    /// * `options` - Caller-supplied permission options to render to the user.
    async fn request_user_permission(
        &self,
        _session_id: &crate::session::SessionId,
        session_id_str: &str,
        tool_call_id: &str,
        tool_name: &str,
        client: &agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
        options: &[crate::tools::PermissionOption],
    ) {
        // Convert our internal types to ACP protocol types
        let acp_options: Vec<agent_client_protocol::schema::PermissionOption> = options
            .iter()
            .map(|opt| {
                let kind = match opt.kind {
                    crate::tools::PermissionOptionKind::AllowOnce => {
                        agent_client_protocol::schema::PermissionOptionKind::AllowOnce
                    }
                    crate::tools::PermissionOptionKind::AllowAlways => {
                        agent_client_protocol::schema::PermissionOptionKind::AllowAlways
                    }
                    crate::tools::PermissionOptionKind::RejectOnce => {
                        agent_client_protocol::schema::PermissionOptionKind::RejectOnce
                    }
                    crate::tools::PermissionOptionKind::RejectAlways => {
                        agent_client_protocol::schema::PermissionOptionKind::RejectAlways
                    }
                };
                agent_client_protocol::schema::PermissionOption::new(
                    agent_client_protocol::schema::PermissionOptionId::new(opt.option_id.as_str()),
                    opt.name.clone(),
                    kind,
                )
            })
            .collect();

        let tool_call_update = agent_client_protocol::schema::ToolCallUpdate::new(
            agent_client_protocol::schema::ToolCallId::new(tool_call_id),
            agent_client_protocol::schema::ToolCallUpdateFields::new(),
        );

        let acp_request = agent_client_protocol::schema::RequestPermissionRequest::new(
            SessionId::new(session_id_str.to_string()),
            tool_call_update,
            acp_options,
        );

        // ACP 0.11: dispatch to the counterpart Client role over the
        // ConnectionTo handle. Calling `block_task` here is safe iff the caller
        // has already spawned this work off the JSON-RPC event loop (e.g. via
        // `cx.spawn(...)` from the `prompt` dispatch handler) — invoking it
        // directly inside an `on_receive_request` handler would deadlock. The
        // dispatch-layer wiring landing in B5/B6/B9 must uphold this contract.
        match client.send_request(acp_request).block_task().await {
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
        response: agent_client_protocol::schema::RequestPermissionResponse,
        options: &[crate::tools::PermissionOption],
    ) {
        match response.outcome {
            agent_client_protocol::schema::RequestPermissionOutcome::Cancelled => {
                tracing::info!("Permission request cancelled for '{}'", tool_name);
                // TODO: Send tool completion with cancelled status
            }
            agent_client_protocol::schema::RequestPermissionOutcome::Selected(selected) => {
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

    /// Handle non-streaming prompt request.
    ///
    /// `requested_max_tokens` carries the optional per-request generation cap
    /// from `PromptRequest.meta.max_tokens`. When present, it narrows (but
    /// never widens) the agent's configured `max_tokens_per_turn`. Hitting the
    /// resulting effective cap aborts the in-flight generation and yields
    /// `StopReason::MaxTokens`.
    pub(crate) async fn handle_non_streaming_prompt(
        &self,
        session_id: &crate::session::SessionId,
        request: &PromptRequest,
        session: &crate::session::Session,
        requested_max_tokens: Option<u64>,
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

        let effective_cap =
            effective_generation_cap(self.config.max_tokens_per_turn, requested_max_tokens);

        let mut response_content = String::new();
        let mut chunk_count = 0;
        let mut output_tokens: u64 = 0;

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
                use agent_client_protocol::schema::{
                    ToolCall, ToolCallId, ToolCallStatus, ToolKind,
                };

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
                SessionUpdate::AgentMessageChunk(agent_client_protocol::schema::ContentChunk::new(
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

            // Accumulate output tokens against the effective cap. We count
            // text deltas only (tool-call chunks are protocol metadata, not
            // generated content). If the cap fires, abort the underlying
            // claude subprocess and return `MaxTokens` immediately so the
            // model can't keep generating off-screen.
            if let Some(response) = self
                .check_output_token_cap(
                    session_id,
                    &session_id_str,
                    &mut output_tokens,
                    &chunk.content,
                    OutputCap {
                        effective: effective_cap,
                        caller_supplied: requested_max_tokens,
                        streaming: false,
                    },
                )
                .await
            {
                return Ok(response);
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // extract_request_max_tokens — caller-supplied generation cap from `_meta`
    // =========================================================================

    /// Returns `None` when no meta map is provided.
    ///
    /// The validator runner only attaches `max_tokens` for rule executions;
    /// other callers leave `request.meta` as `None` and must keep the
    /// existing (uncapped) behavior.
    #[test]
    fn test_extract_request_max_tokens_none_when_meta_missing() {
        assert_eq!(extract_request_max_tokens(None), None);
    }

    /// Returns `None` when meta is present but doesn't contain `max_tokens`.
    ///
    /// This guards the "generic ACP client that uses `_meta` for something
    /// else" case — we must not interpret unrelated `_meta` keys.
    #[test]
    fn test_extract_request_max_tokens_none_when_key_missing() {
        let mut meta = serde_json::Map::new();
        meta.insert("other_key".to_string(), serde_json::json!(42));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// Returns `Some(n)` for the canonical case the validator runner produces:
    /// `max_tokens` set to a positive `u64`. This is the contract we share
    /// with `avp-common::validator::runner::build_rule_prompt_request`.
    #[test]
    fn test_extract_request_max_tokens_positive_integer() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(4096_u64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), Some(4096));
    }

    /// Returns `None` for `max_tokens: 0` — a zero cap would be useless and
    /// almost certainly indicates a bug at the caller. Treating it as "no
    /// cap" matches the runner's intent (defense-in-depth, not a hard
    /// requirement).
    #[test]
    fn test_extract_request_max_tokens_zero_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(0));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// Returns `None` for non-integer types. Strings, floats, and booleans
    /// under the `max_tokens` key are all treated as "no cap" — we never
    /// coerce or guess.
    #[test]
    fn test_extract_request_max_tokens_string_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!("4096"));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    #[test]
    fn test_extract_request_max_tokens_float_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(4096.5));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    #[test]
    fn test_extract_request_max_tokens_bool_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(true));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// `i64`-formatted integers (signed positives) round-trip through
    /// `as_u64`: signed positives parse, negatives don't. We accept the
    /// positive case since `serde_json` may serialize positive ints as
    /// either `Number::U64` or `Number::I64` depending on source.
    #[test]
    fn test_extract_request_max_tokens_signed_positive_accepted() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(8192_i64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), Some(8192));
    }

    /// Negative integers do not satisfy `as_u64` and are treated as "no
    /// cap". The runner never sends negatives — this guards against
    /// accidentally widening (turning a negative into a huge `u64`).
    #[test]
    fn test_extract_request_max_tokens_negative_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(-1_i64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    // =========================================================================
    // effective_generation_cap — caller can only narrow, never widen
    // =========================================================================

    /// With no caller-supplied cap, the agent's configured per-turn cap
    /// applies unchanged.
    #[test]
    fn test_effective_cap_no_request_uses_config() {
        assert_eq!(effective_generation_cap(100_000, None), 100_000);
    }

    /// A caller-supplied cap below the config wins — the caller is allowed
    /// to narrow.
    #[test]
    fn test_effective_cap_caller_narrows() {
        assert_eq!(effective_generation_cap(100_000, Some(16_384)), 16_384);
    }

    /// A caller-supplied cap above the config is clamped down to the
    /// config — the caller cannot widen the agent's safety limit.
    #[test]
    fn test_effective_cap_caller_cannot_widen() {
        assert_eq!(effective_generation_cap(16_384, Some(100_000)), 16_384);
    }

    /// Caller-supplied cap equal to config produces config (no change).
    #[test]
    fn test_effective_cap_caller_equal_to_config() {
        assert_eq!(effective_generation_cap(16_384, Some(16_384)), 16_384);
    }

    // =========================================================================
    // estimate_chunk_tokens — output-token estimator for streaming chunks
    // =========================================================================

    /// Empty content contributes zero tokens.
    #[test]
    fn test_estimate_chunk_tokens_empty_is_zero() {
        assert_eq!(estimate_chunk_tokens(""), 0);
    }

    /// Short non-empty chunks are floored at one token so a stream of
    /// single-byte deltas still accumulates against the cap.
    #[test]
    fn test_estimate_chunk_tokens_short_floors_at_one() {
        assert_eq!(estimate_chunk_tokens("a"), 1);
        assert_eq!(estimate_chunk_tokens("abc"), 1);
    }

    /// Longer chunks use the `len/4` heuristic. 20 bytes → 5 tokens.
    #[test]
    fn test_estimate_chunk_tokens_long_uses_len_over_four() {
        assert_eq!(estimate_chunk_tokens("12345678901234567890"), 5);
    }

    // =========================================================================
    // process_stream_chunks integration test — cap fires → MaxTokens
    // =========================================================================

    use crate::claude::{ChunkType, MessageChunk};
    use crate::config::AgentConfig;
    use agent_client_protocol::schema::StopReason;
    use futures::Stream;

    /// Build a stream of `MessageChunk`s that emit fixed-size text deltas.
    ///
    /// Each chunk is `delta_size` bytes of `'a'`. Producing a long enough
    /// stream lets us drive the per-turn output token cap without standing
    /// up the real claude CLI subprocess.
    fn fixed_text_stream(
        chunk_count: usize,
        delta_size: usize,
    ) -> std::pin::Pin<Box<dyn Stream<Item = MessageChunk> + Send>> {
        let chunks: Vec<MessageChunk> = (0..chunk_count)
            .map(|_| MessageChunk {
                content: "a".repeat(delta_size),
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
                stop_reason: None,
            })
            .collect();
        Box::pin(tokio_stream::iter(chunks))
    }

    /// `process_stream_chunks` returns `StopReason::MaxTokens` and stops
    /// reading the stream when the accumulated estimated output tokens
    /// exceed the effective cap.
    ///
    /// This is the contract the validator runner depends on: a runaway
    /// generation must surface as `MaxTokens` (not `EndTurn`) so the
    /// runner's `build_max_tokens_failure_message` path produces a loud
    /// rule failure instead of a silent pass.
    #[tokio::test]
    async fn test_process_stream_chunks_max_tokens_fires_on_cap() {
        let config = AgentConfig::default();
        let (agent, _rx) = crate::agent::ClaudeAgent::new(config)
            .await
            .expect("agent construction must succeed");

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = agent
            .session_manager
            .create_session(cwd, None)
            .expect("session creation must succeed");

        // 100 chunks × 16 bytes ≈ 100 × 4 = 400 estimated output tokens.
        // Set the cap at 50 tokens so the cap fires partway through.
        let mut stream = fixed_text_stream(100, 16);
        let effective_cap = 50_u64;
        let caller_supplied_cap = Some(50_u64);

        let response = agent
            .process_stream_chunks(&session_id, &mut stream, effective_cap, caller_supplied_cap)
            .await
            .expect("process_stream_chunks must succeed");

        assert_eq!(
            response.stop_reason,
            StopReason::MaxTokens,
            "Hitting the per-turn output cap must surface as StopReason::MaxTokens"
        );

        let meta = response.meta.expect("MaxTokens response must include meta");
        assert_eq!(
            meta.get("effective_max_tokens").and_then(|v| v.as_u64()),
            Some(effective_cap),
            "meta must report the effective cap that fired"
        );
        assert_eq!(
            meta.get("requested_max_tokens").and_then(|v| v.as_u64()),
            Some(50),
            "meta must echo the caller-supplied cap so callers can distinguish their cap firing"
        );
        assert!(
            meta.get("output_tokens")
                .and_then(|v| v.as_u64())
                .map(|n| n > effective_cap)
                .unwrap_or(false),
            "meta.output_tokens must reflect a count above the cap"
        );
    }

    /// When the cap is generous enough that the stream finishes naturally,
    /// `process_stream_chunks` returns `StopReason::EndTurn` (not
    /// `MaxTokens`). This guards against the cap firing when it shouldn't —
    /// e.g. an off-by-one on the boundary check would silently degrade
    /// every short response into a fake max-tokens failure.
    #[tokio::test]
    async fn test_process_stream_chunks_no_cap_fire_when_under_limit() {
        let config = AgentConfig::default();
        let (agent, _rx) = crate::agent::ClaudeAgent::new(config)
            .await
            .expect("agent construction must succeed");

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = agent
            .session_manager
            .create_session(cwd, None)
            .expect("session creation must succeed");

        // 5 chunks × 4 bytes ≈ 5 × 1 = 5 estimated tokens. Cap at 100k.
        let mut stream = fixed_text_stream(5, 4);
        let response = agent
            .process_stream_chunks(&session_id, &mut stream, 100_000, None)
            .await
            .expect("process_stream_chunks must succeed");

        assert_ne!(
            response.stop_reason,
            StopReason::MaxTokens,
            "A short stream must not trigger the max_tokens cap"
        );
    }
}
