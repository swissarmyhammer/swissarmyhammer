//! MCP ClientHandler that converts MCP notifications to ACP SessionNotifications

use crate::acp::elicitation::{bridge_elicitation, ElicitationSender};
use agent_client_protocol::schema::{
    ClientCapabilities, SessionId, SessionNotification, SessionUpdate,
};
use rmcp::{
    model::{
        CreateElicitationRequestParams, CreateElicitationResult, LoggingMessageNotificationParam,
        ProgressNotificationParam,
    },
    service::{NotificationContext, RequestContext, RoleClient},
    ClientHandler, ErrorData as McpError,
};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

/// Shared slot that publishes the live ACP elicitation endpoint to MCP client
/// handlers.
///
/// The endpoint (a sender wrapping the agent's `ConnectionTo<Client>`) only
/// exists once an ACP client is connected, which happens in
/// `AcpServer::start_with_streams` — well after the per-session
/// [`NotifyingClientHandler`]s are created. The handlers therefore read the
/// endpoint through this shared, late-populated slot rather than owning it.
pub type ElicitationEndpoint = Arc<RwLock<Option<Arc<dyn ElicitationSender>>>>;

/// Shared cell holding the ACP client's capabilities, populated when the
/// `initialize` request arrives.
///
/// The handler reads the connected client's capabilities through this
/// late-populated slot — shared with [`crate::acp::AcpServer`] — to confirm the
/// client advertised the `elicitation` capability before relaying an MCP
/// `elicitation/create` to it. Mirrors `claude-agent`'s
/// `SharedClientCapabilities`.
pub type SharedClientCapabilities = Arc<RwLock<Option<ClientCapabilities>>>;

/// ClientHandler that forwards MCP notifications as ACP SessionNotifications
#[derive(Clone)]
pub struct NotifyingClientHandler {
    /// Broadcaster for ACP notifications
    notification_tx: broadcast::Sender<SessionNotification>,
    /// Current session context (set before tool calls)
    current_session: Arc<Mutex<Option<SessionId>>>,
    /// Shared endpoint used to relay MCP elicitation requests to the ACP client.
    elicitation_endpoint: ElicitationEndpoint,
    /// Shared ACP client capabilities, used to gate elicitation on the client
    /// having advertised the `elicitation` capability.
    client_capabilities: SharedClientCapabilities,
}

impl NotifyingClientHandler {
    /// Create a new NotifyingClientHandler with no elicitation endpoint and no
    /// shared client capabilities.
    ///
    /// Elicitation requests received before an endpoint is installed are
    /// declined, as are requests received before client capabilities are known
    /// or when the client did not advertise elicitation support. Use
    /// [`NotifyingClientHandler::with_elicitation_endpoint`] to share the
    /// server-level endpoint and capabilities cells that are populated once an
    /// ACP client connects and initializes.
    pub fn new(notification_tx: broadcast::Sender<SessionNotification>) -> Self {
        Self::with_elicitation_endpoint(
            notification_tx,
            Arc::new(RwLock::new(None)),
            Arc::new(RwLock::new(None)),
        )
    }

    /// Create a new NotifyingClientHandler sharing the given elicitation endpoint
    /// and client capabilities cells.
    pub fn with_elicitation_endpoint(
        notification_tx: broadcast::Sender<SessionNotification>,
        elicitation_endpoint: ElicitationEndpoint,
        client_capabilities: SharedClientCapabilities,
    ) -> Self {
        Self {
            notification_tx,
            current_session: Arc::new(Mutex::new(None)),
            elicitation_endpoint,
            client_capabilities,
        }
    }

    /// Set the current session context before making MCP tool calls
    pub async fn set_session(&self, session_id: SessionId) {
        *self.current_session.lock().await = Some(session_id);
    }

    /// Clear the session context after tool calls complete
    pub async fn clear_session(&self) {
        *self.current_session.lock().await = None;
    }

    /// Relay an MCP elicitation request to the ACP client and return the rmcp
    /// result.
    ///
    /// Looks up the current session context, the shared elicitation endpoint,
    /// and the connected client's advertised capabilities, then runs the full
    /// ACP round-trip via [`crate::acp::elicitation::bridge_elicitation`]. The
    /// request is declined — without contacting the client — when the client did
    /// not advertise the `elicitation` capability, when no endpoint is installed
    /// (no ACP client connected), or when no session context is set. This is the
    /// testable core of the [`ClientHandler::create_elicitation`] hook, factored
    /// out so it can be exercised without constructing an rmcp `RequestContext`.
    pub async fn relay_elicitation(
        &self,
        request: CreateElicitationRequestParams,
    ) -> CreateElicitationResult {
        let session_id = self.current_session.lock().await.clone();
        let endpoint = self.elicitation_endpoint.read().await.clone();
        let supports_elicitation = self.client_supports_elicitation().await;
        bridge_elicitation(
            endpoint.as_deref(),
            &request,
            session_id.as_ref(),
            supports_elicitation,
        )
        .await
    }

    /// Whether the connected ACP client advertised the `elicitation` capability.
    ///
    /// The bridge only ever issues form-mode elicitations, so the presence of
    /// the `elicitation` capability object is sufficient. Returns `false` when
    /// capabilities are not yet known (no client has initialized). Mirrors
    /// `claude-agent`'s `ElicitationBridgeHandler::client_supports_elicitation`.
    async fn client_supports_elicitation(&self) -> bool {
        self.client_capabilities
            .read()
            .await
            .as_ref()
            .map(|caps| caps.elicitation.is_some())
            .unwrap_or(false)
    }

    /// Broadcast an ACP notification if we have a session context
    fn broadcast_acp_notification(&self, session_id: SessionId, update: SessionUpdate) {
        let notification = SessionNotification::new(session_id, update);

        match self.notification_tx.send(notification) {
            Ok(count) => {
                tracing::debug!("Forwarded MCP notification as ACP to {} subscribers", count);
            }
            Err(e) => {
                tracing::warn!("Failed to forward MCP notification: {}", e);
            }
        }
    }
}

impl ClientHandler for NotifyingClientHandler {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        // Advertise elicitation support so the SAH MCP server is permitted to
        // call `peer.create_elicitation(...)`. The actual request is redirected
        // to the ACP client in `create_elicitation` below.
        let capabilities = rmcp::model::ClientCapabilities::builder()
            .enable_elicitation()
            .build();
        rmcp::model::ClientInfo::new(
            capabilities,
            rmcp::model::Implementation::new(
                "llama_agent_notifying_client",
                env!("CARGO_PKG_VERSION"),
            )
            .with_title("Llama Agent Notifying MCP Client"),
        )
    }

    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, McpError> {
        // Relay the MCP elicitation to the ACP client over the live connection,
        // tying it to the session currently driving the tool call. If no client
        // is connected (or no session context is set), the relay declines.
        Ok(self.relay_elicitation(request).await)
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "MCP progress notification: token={:?}, progress={}/{:?}, message={:?}",
            params.progress_token,
            params.progress,
            params.total,
            params.message
        );

        // Get current session if available
        if let Some(session_id) = self.current_session.lock().await.clone() {
            // For now, log as AgentMessageChunk with progress info
            // In the future, we could add a custom Progress update type to ACP
            if let Some(message) = params.message {
                let text = format!(
                    "[Progress: {:.0}%] {}",
                    (params.progress / params.total.unwrap_or(100.0)) * 100.0,
                    message
                );
                let text_content = agent_client_protocol::schema::TextContent::new(text);
                let update = SessionUpdate::AgentMessageChunk(
                    agent_client_protocol::schema::ContentChunk::new(
                        agent_client_protocol::schema::ContentBlock::Text(text_content),
                    ),
                );
                self.broadcast_acp_notification(session_id, update);
            }
        } else {
            tracing::warn!("Received MCP progress notification but no session context available");
        }
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "MCP logging notification: level={:?}, logger={:?}, data={:?}",
            params.level,
            params.logger,
            params.data
        );

        // Get current session if available
        if let Some(session_id) = self.current_session.lock().await.clone() {
            // Convert logging message to AgentMessageChunk
            let message = if let Some(msg) = params.data.get("message") {
                msg.as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| params.data.to_string())
            } else {
                params.data.to_string()
            };

            // Only forward Info and higher level messages to avoid spam
            if matches!(
                params.level,
                rmcp::model::LoggingLevel::Info
                    | rmcp::model::LoggingLevel::Warning
                    | rmcp::model::LoggingLevel::Error
                    | rmcp::model::LoggingLevel::Critical
            ) {
                let text = format!("[MCP] {}", message);
                let text_content = agent_client_protocol::schema::TextContent::new(text);
                let update = SessionUpdate::AgentMessageChunk(
                    agent_client_protocol::schema::ContentChunk::new(
                        agent_client_protocol::schema::ContentBlock::Text(text_content),
                    ),
                );
                self.broadcast_acp_notification(session_id, update);
            }
        } else {
            tracing::warn!("Received MCP logging notification but no session context available");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::elicitation::accept_with_string;
    use agent_client_protocol::schema::{
        CreateElicitationRequest, CreateElicitationResponse, ElicitationCapabilities,
        ElicitationFormCapabilities,
    };
    use rmcp::model::{ElicitationAction as McpElicitationAction, ElicitationSchema};

    /// Client capabilities advertising form-mode elicitation support, matching
    /// what the kanban webview reports in `initialize`.
    fn caps_with_elicitation() -> SharedClientCapabilities {
        let caps = ClientCapabilities::new()
            .elicitation(ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()));
        Arc::new(RwLock::new(Some(caps)))
    }

    /// Fake elicitation endpoint that records emitted ACP requests and replies
    /// with a canned response.
    struct FakeEndpoint {
        received: Arc<Mutex<Vec<CreateElicitationRequest>>>,
        reply: CreateElicitationResponse,
    }

    #[async_trait::async_trait]
    impl ElicitationSender for FakeEndpoint {
        async fn send(
            &self,
            request: CreateElicitationRequest,
        ) -> Result<CreateElicitationResponse, String> {
            self.received.lock().await.push(request);
            Ok(self.reply.clone())
        }
    }

    fn form_params(question: &str) -> CreateElicitationRequestParams {
        let schema = ElicitationSchema::builder()
            .required_string_with("answer", |s| s.description(question.to_string()))
            .build_unchecked();
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: question.to_string(),
            requested_schema: schema,
        }
    }

    fn handler_with_endpoint(
        reply: CreateElicitationResponse,
    ) -> (
        NotifyingClientHandler,
        Arc<Mutex<Vec<CreateElicitationRequest>>>,
    ) {
        let (tx, _rx) = broadcast::channel(16);
        let received = Arc::new(Mutex::new(Vec::new()));
        let endpoint: Arc<dyn ElicitationSender> = Arc::new(FakeEndpoint {
            received: received.clone(),
            reply,
        });
        let slot: ElicitationEndpoint = Arc::new(RwLock::new(Some(endpoint)));
        let handler =
            NotifyingClientHandler::with_elicitation_endpoint(tx, slot, caps_with_elicitation());
        (handler, received)
    }

    #[test]
    fn get_info_advertises_elicitation_capability() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = NotifyingClientHandler::new(tx);

        let info = handler.get_info();

        assert!(
            info.capabilities.elicitation.is_some(),
            "MCP client must advertise elicitation capability"
        );
    }

    #[tokio::test]
    async fn relay_emits_one_acp_request_and_round_trips_accept() {
        let (handler, received) = handler_with_endpoint(accept_with_string("answer", "Grace"));
        handler.set_session(SessionId::new("sess_1")).await;

        let result = handler
            .relay_elicitation(form_params("What is your name?"))
            .await;

        let requests = received.lock().await;
        assert_eq!(requests.len(), 1, "exactly one ACP elicitation expected");
        assert_eq!(requests[0].message, "What is your name?");

        assert_eq!(result.action, McpElicitationAction::Accept);
        let answer = result
            .content
            .as_ref()
            .and_then(|c| c.get("answer"))
            .and_then(|v| v.as_str());
        assert_eq!(answer, Some("Grace"));
    }

    #[tokio::test]
    async fn relay_propagates_cancel() {
        let (handler, _received) = handler_with_endpoint(CreateElicitationResponse::new(
            agent_client_protocol::schema::ElicitationAction::Cancel,
        ));
        handler.set_session(SessionId::new("sess_1")).await;

        let result = handler.relay_elicitation(form_params("Pick one")).await;

        assert_eq!(result.action, McpElicitationAction::Cancel);
    }

    #[tokio::test]
    async fn relay_without_endpoint_declines() {
        // Capabilities advertised and a session set, but no endpoint installed:
        // the no-ACP-client decline path.
        let (tx, _rx) = broadcast::channel(16);
        let handler = NotifyingClientHandler::with_elicitation_endpoint(
            tx,
            Arc::new(RwLock::new(None)),
            caps_with_elicitation(),
        );
        handler.set_session(SessionId::new("sess_1")).await;

        let result = handler
            .relay_elicitation(form_params("Anyone there?"))
            .await;

        assert_eq!(result.action, McpElicitationAction::Decline);
    }

    #[tokio::test]
    async fn relay_without_capability_declines_without_sending() {
        // Endpoint installed and a session set, but the client never advertised
        // the elicitation capability: decline up front without contacting it,
        // matching claude-agent's bridge.
        let (tx, _rx) = broadcast::channel(16);
        let received = Arc::new(Mutex::new(Vec::new()));
        let endpoint: Arc<dyn ElicitationSender> = Arc::new(FakeEndpoint {
            received: received.clone(),
            reply: accept_with_string("answer", "x"),
        });
        let slot: ElicitationEndpoint = Arc::new(RwLock::new(Some(endpoint)));
        let handler = NotifyingClientHandler::with_elicitation_endpoint(
            tx,
            slot,
            Arc::new(RwLock::new(None)),
        );
        handler.set_session(SessionId::new("sess_1")).await;

        let result = handler
            .relay_elicitation(form_params("Anyone there?"))
            .await;

        assert_eq!(result.action, McpElicitationAction::Decline);
        assert!(
            received.lock().await.is_empty(),
            "no ACP request should be emitted when the client lacks elicitation support"
        );
    }

    #[tokio::test]
    async fn relay_without_session_declines() {
        let (handler, received) = handler_with_endpoint(accept_with_string("answer", "x"));
        // No set_session call: no session context.

        let result = handler.relay_elicitation(form_params("Hi?")).await;

        assert_eq!(result.action, McpElicitationAction::Decline);
        assert!(
            received.lock().await.is_empty(),
            "no ACP request should be emitted without a session context"
        );
    }
}
