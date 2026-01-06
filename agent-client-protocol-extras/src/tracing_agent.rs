//! TracingAgent - Wrapper that logs all Agent method calls at INFO level
//!
//! Provides unified tracing for all ACP agent implementations.

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
    ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse,
};
use tokio::sync::broadcast;

/// Extract text content from ACP ContentBlocks for logging
fn extract_prompt_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text(text) = block {
                Some(text.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Log a single notification
fn log_notification(agent_name: &str, notification: &SessionNotification) {
    let session_id = &notification.session_id;
    match &notification.update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            if let ContentBlock::Text(text) = &chunk.content {
                tracing::info!(
                    "[{}] notification: session={}, AgentMessageChunk ({} chars): {}",
                    agent_name,
                    session_id,
                    text.text.len(),
                    text.text
                );
            } else {
                tracing::info!(
                    "[{}] notification: session={}, AgentMessageChunk (non-text)",
                    agent_name,
                    session_id
                );
            }
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            if let ContentBlock::Text(text) = &chunk.content {
                tracing::info!(
                    "[{}] notification: session={}, AgentThoughtChunk ({} chars): {}",
                    agent_name,
                    session_id,
                    text.text.len(),
                    text.text
                );
            }
        }
        SessionUpdate::ToolCall(tool_call) => {
            tracing::info!(
                "[{}] notification: session={}, ToolCall: {}",
                agent_name,
                session_id,
                tool_call.title
            );
        }
        SessionUpdate::ToolCallUpdate(update) => {
            tracing::debug!(
                "[{}] notification: session={}, ToolCallUpdate: {}",
                agent_name,
                session_id,
                update.tool_call_id
            );
        }
        SessionUpdate::CurrentModeUpdate(mode) => {
            tracing::info!(
                "[{}] notification: session={}, CurrentModeUpdate: {}",
                agent_name,
                session_id,
                mode.current_mode_id
            );
        }
        SessionUpdate::AvailableCommandsUpdate(update) => {
            tracing::info!(
                "[{}] notification: session={}, AvailableCommandsUpdate: {} commands",
                agent_name,
                session_id,
                update.available_commands.len()
            );
        }
        SessionUpdate::Plan(plan) => {
            tracing::info!(
                "[{}] notification: session={}, Plan: {} entries",
                agent_name,
                session_id,
                plan.entries.len()
            );
        }
        _ => {
            tracing::debug!(
                "[{}] notification: session={}, other update type",
                agent_name,
                session_id
            );
        }
    }
}

/// Spawn a task that logs all notifications from the receiver
///
/// Returns a new receiver that can be used by consumers (the original is consumed by the logger).
pub fn trace_notifications(
    agent_name: String,
    receiver: broadcast::Receiver<SessionNotification>,
) -> broadcast::Receiver<SessionNotification> {
    // Create a new channel to forward notifications after logging
    let (tx, rx) = broadcast::channel(256);

    let mut recv = receiver;
    tokio::spawn(async move {
        loop {
            match recv.recv().await {
                Ok(notification) => {
                    log_notification(&agent_name, &notification);
                    // Forward to consumers (ignore send errors if no receivers)
                    let _ = tx.send(notification);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("[{}] notification receiver lagged by {}", agent_name, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!("[{}] notification channel closed", agent_name);
                    break;
                }
            }
        }
    });

    rx
}

/// TracingAgent wraps any Agent and logs all method calls
///
/// Uses Arc<dyn Agent> internally to work with dynamically dispatched agents.
pub struct TracingAgent {
    inner: std::sync::Arc<dyn Agent + Send + Sync>,
    agent_name: String,
}

impl TracingAgent {
    /// Create a new TracingAgent wrapping the given agent
    pub fn new(
        inner: std::sync::Arc<dyn Agent + Send + Sync>,
        agent_name: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            agent_name: agent_name.into(),
        }
    }

    /// Get the agent name for logging
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Get reference to inner agent
    pub fn inner(&self) -> &std::sync::Arc<dyn Agent + Send + Sync> {
        &self.inner
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for TracingAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        tracing::info!(
            "[{}] initialize: protocol={:?}",
            self.agent_name,
            request.protocol_version
        );

        let response = self.inner.initialize(request).await?;

        if let Some(ref info) = response.agent_info {
            tracing::info!(
                "[{}] response: agent={}, version={}",
                self.agent_name,
                info.name,
                info.version
            );
        }

        Ok(response)
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        tracing::info!("[{}] authenticate", self.agent_name);
        self.inner.authenticate(request).await
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        tracing::info!(
            "[{}] new_session: cwd={}",
            self.agent_name,
            request.cwd.display()
        );

        let response = self.inner.new_session(request).await?;

        tracing::info!(
            "[{}] response: session_id={}",
            self.agent_name,
            response.session_id
        );

        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let prompt_text = extract_prompt_text(&request.prompt);
        tracing::info!(
            "[{}] prompt ({} chars): {}",
            self.agent_name,
            prompt_text.len(),
            prompt_text
        );

        let response = self.inner.prompt(request).await?;

        tracing::info!(
            "[{}] response: stop_reason={:?}",
            self.agent_name,
            response.stop_reason
        );

        Ok(response)
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        tracing::info!(
            "[{}] cancel: session_id={}",
            self.agent_name,
            request.session_id
        );
        self.inner.cancel(request).await
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        tracing::info!(
            "[{}] load_session: session_id={}",
            self.agent_name,
            request.session_id
        );

        let response = self.inner.load_session(request).await?;

        tracing::info!("[{}] response: session loaded", self.agent_name);

        Ok(response)
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        tracing::info!(
            "[{}] set_session_mode: session={}, mode={}",
            self.agent_name,
            request.session_id,
            request.mode_id
        );

        let response = self.inner.set_session_mode(request).await?;

        tracing::info!("[{}] response: mode set", self.agent_name);

        Ok(response)
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        tracing::info!("[{}] ext_method", self.agent_name);

        let response = self.inner.ext_method(request).await?;

        tracing::info!("[{}] response: ext_method complete", self.agent_name);

        Ok(response)
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        tracing::info!("[{}] ext_notification", self.agent_name);
        self.inner.ext_notification(notification).await
    }
}
