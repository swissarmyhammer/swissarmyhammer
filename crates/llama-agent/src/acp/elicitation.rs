//! Bridge MCP elicitation requests to the ACP client.
//!
//! When the per-board SAH MCP server issues an `elicitation/create` request to
//! llama-agent's MCP client during a tool call, that request must be redirected
//! to the user. llama-agent is itself an ACP **Agent**, so the redirect is an
//! agent→client ACP `elicitation/create` request sent over the live
//! [`agent_client_protocol::ConnectionTo`] connection. The user's answer travels
//! back the same way and is translated into the rmcp elicitation result the SAH
//! server expects.
//!
//! # Why an extension request
//!
//! The `agent-client-protocol` runtime crate (0.11) predates elicitation and
//! does not register `elicitation/create` as a typed request, so the ACP
//! elicitation *types* (from `agent-client-protocol-schema`) cannot be handed to
//! `ConnectionTo::send_request` directly. Instead the request is sent as an
//! [`agent_client_protocol::UntypedMessage`] keyed on the
//! `"elicitation/create"` method, and the raw JSON response is decoded back into
//! a [`CreateElicitationResponse`]. This matches the method name used by the
//! TypeScript SDK.

use std::collections::BTreeMap;

use agent_client_protocol::schema::{
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAcceptAction,
    ElicitationAction, ElicitationContentValue, ElicitationFormMode,
    ElicitationSchema as AcpElicitationSchema, ElicitationSessionScope, SessionId as AcpSessionId,
};
use agent_client_protocol::{Client, ConnectionTo, UntypedMessage};
use rmcp::model::{
    CreateElicitationRequestParams, CreateElicitationResult,
    ElicitationAction as McpElicitationAction,
};

/// ACP method name for an agent→client elicitation request. Matches the
/// TypeScript SDK and the `agent-client-protocol-schema`
/// `ELICITATION_CREATE_METHOD_NAME` constant.
const ELICITATION_CREATE_METHOD: &str = "elicitation/create";

/// Sends an ACP `elicitation/create` request to the connected client and awaits
/// the response.
///
/// Implementors own whatever transport reaches the client. The production
/// implementation wraps a live [`agent_client_protocol::ConnectionTo`]; tests use
/// an in-memory fake.
#[async_trait::async_trait]
pub trait ElicitationSender: Send + Sync {
    /// Send the elicitation request to the client and await the user's response.
    ///
    /// # Errors
    ///
    /// Returns an error string when the request could not be delivered or the
    /// response could not be decoded.
    async fn send(
        &self,
        request: CreateElicitationRequest,
    ) -> Result<CreateElicitationResponse, String>;
}

/// Production [`ElicitationSender`] backed by the agent's live ACP connection.
///
/// `agent-client-protocol` 0.11 does not register `elicitation/create` as a
/// typed request, so the request is sent as an [`UntypedMessage`] keyed on the
/// `elicitation/create` method and the raw JSON reply is decoded back into a
/// [`CreateElicitationResponse`].
#[derive(Clone)]
pub struct ConnectionElicitationSender {
    connection: ConnectionTo<Client>,
}

impl ConnectionElicitationSender {
    /// Wrap a live connection to the ACP client.
    #[must_use]
    pub fn new(connection: ConnectionTo<Client>) -> Self {
        Self { connection }
    }
}

#[async_trait::async_trait]
impl ElicitationSender for ConnectionElicitationSender {
    async fn send(
        &self,
        request: CreateElicitationRequest,
    ) -> Result<CreateElicitationResponse, String> {
        let untyped = UntypedMessage::new(ELICITATION_CREATE_METHOD, &request)
            .map_err(|e| format!("failed to encode elicitation request: {e}"))?;
        let value = self
            .connection
            .send_request(untyped)
            .block_task()
            .await
            .map_err(|e| format!("elicitation request failed: {e}"))?;
        serde_json::from_value(value)
            .map_err(|e| format!("failed to decode elicitation response: {e}"))
    }
}

/// Translate an inbound rmcp elicitation request into an ACP elicitation request
/// scoped to the given session.
///
/// Only form-mode elicitation is produced — the SAH `ask` tool always elicits a
/// form. URL-mode rmcp requests are also rendered as a form whose message
/// carries the URL, because the local webview collects answers inline rather
/// than opening a browser.
///
/// # Errors
///
/// Returns an error string when the rmcp schema cannot be re-encoded into the
/// ACP schema shape.
pub fn mcp_request_to_acp(
    params: &CreateElicitationRequestParams,
    session_id: &AcpSessionId,
) -> Result<CreateElicitationRequest, String> {
    let (message, acp_schema) = match params {
        CreateElicitationRequestParams::FormElicitationParams {
            message,
            requested_schema,
            ..
        } => (message.clone(), translate_schema(requested_schema)?),
        CreateElicitationRequestParams::UrlElicitationParams { message, url, .. } => {
            // No URL-mode UI in the local webview: surface the URL in the prompt
            // and collect a free-form acknowledgement field.
            let message = format!("{message}\n\n{url}");
            (message, AcpElicitationSchema::new())
        }
    };

    let scope = ElicitationSessionScope::new(session_id.clone());
    let mode = ElicitationFormMode::new(scope, acp_schema);
    Ok(CreateElicitationRequest::new(mode, message))
}

/// Re-encode an rmcp [`rmcp::model::ElicitationSchema`] into the ACP
/// [`AcpElicitationSchema`].
///
/// Both types serialize to the same JSON-Schema object (`type: "object"`,
/// `properties`, `required`), so the translation goes through JSON.
///
/// # Errors
///
/// Returns an error string when serialization or deserialization fails.
fn translate_schema(
    schema: &rmcp::model::ElicitationSchema,
) -> Result<AcpElicitationSchema, String> {
    let value = serde_json::to_value(schema)
        .map_err(|e| format!("failed to encode elicitation schema: {e}"))?;
    serde_json::from_value(value)
        .map_err(|e| format!("failed to decode elicitation schema into ACP shape: {e}"))
}

/// Translate an ACP elicitation response back into the rmcp elicitation result
/// the SAH MCP server expects.
pub fn acp_response_to_mcp(response: CreateElicitationResponse) -> CreateElicitationResult {
    match response.action {
        ElicitationAction::Accept(accept) => CreateElicitationResult {
            action: McpElicitationAction::Accept,
            content: Some(accept_content_to_json(accept)),
            meta: None,
        },
        ElicitationAction::Decline => CreateElicitationResult::new(McpElicitationAction::Decline),
        ElicitationAction::Cancel => CreateElicitationResult::new(McpElicitationAction::Cancel),
        // `ElicitationAction` is `#[non_exhaustive]`; an unknown action is
        // treated conservatively as a decline so the operation continues.
        _ => CreateElicitationResult::new(McpElicitationAction::Decline),
    }
}

/// Convert an accepted ACP elicitation payload into the JSON content object the
/// rmcp result carries. An accept with no content yields an empty object.
fn accept_content_to_json(accept: ElicitationAcceptAction) -> serde_json::Value {
    let map = accept
        .content
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| (key, content_value_to_json(value)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    serde_json::Value::Object(map)
}

/// Convert a single ACP elicitation content value into a JSON value.
fn content_value_to_json(value: ElicitationContentValue) -> serde_json::Value {
    match value {
        ElicitationContentValue::String(s) => serde_json::Value::String(s),
        ElicitationContentValue::Integer(i) => serde_json::Value::from(i),
        ElicitationContentValue::Number(n) => serde_json::Value::from(n),
        ElicitationContentValue::Boolean(b) => serde_json::Value::Bool(b),
        ElicitationContentValue::StringArray(items) => {
            serde_json::Value::Array(items.into_iter().map(serde_json::Value::String).collect())
        }
        // `ElicitationContentValue` is `#[non_exhaustive]`; unknown variants
        // round-trip to null rather than panicking.
        _ => serde_json::Value::Null,
    }
}

/// Run the full elicitation round-trip: translate the rmcp request to ACP, send
/// it to the client, and translate the response back to an rmcp result.
///
/// The request is declined without contacting the client when any precondition
/// for a useful round-trip is missing:
///
/// - `client_supports_elicitation` is `false` — the connected ACP client did not
///   advertise the `elicitation` capability in its `initialize`, so it will not
///   service the request. Declining first mirrors
///   `claude-agent`'s `ElicitationBridgeHandler::client_supports_elicitation`
///   and unblocks the MCP server promptly rather than waiting on a request the
///   client cannot answer.
/// - `sender` is `None` — no ACP client is connected.
/// - `session_id` is `None` — no session context is driving the tool call.
///
/// Declining is the same conservative default rmcp uses for clients without
/// elicitation UI.
pub async fn bridge_elicitation(
    sender: Option<&dyn ElicitationSender>,
    params: &CreateElicitationRequestParams,
    session_id: Option<&AcpSessionId>,
    client_supports_elicitation: bool,
) -> CreateElicitationResult {
    if !client_supports_elicitation {
        tracing::warn!(
            "Client did not advertise the elicitation capability; declining elicitation request"
        );
        return CreateElicitationResult::new(McpElicitationAction::Decline);
    }

    let (Some(sender), Some(session_id)) = (sender, session_id) else {
        tracing::warn!("Elicitation requested but no ACP client connected; declining");
        return CreateElicitationResult::new(McpElicitationAction::Decline);
    };

    let acp_request = match mcp_request_to_acp(params, session_id) {
        Ok(request) => request,
        Err(e) => {
            tracing::error!("Failed to translate elicitation request: {e}");
            return CreateElicitationResult::new(McpElicitationAction::Decline);
        }
    };

    match sender.send(acp_request).await {
        Ok(response) => acp_response_to_mcp(response),
        Err(e) => {
            tracing::error!("Failed to relay elicitation to ACP client: {e}");
            CreateElicitationResult::new(McpElicitationAction::Decline)
        }
    }
}

/// Build an ACP accept response carrying a single string field. Shared by tests
/// and any caller constructing a canned accept payload.
#[must_use]
pub fn accept_with_string(field: &str, value: &str) -> CreateElicitationResponse {
    let content = BTreeMap::from([(field.to_string(), ElicitationContentValue::from(value))]);
    CreateElicitationResponse::new(ElicitationAction::Accept(
        ElicitationAcceptAction::new().content(content),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::ElicitationSchema as McpElicitationSchema;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Fake ACP client that records the requests it receives and replies with a
    /// canned response.
    struct FakeSender {
        received: Arc<Mutex<Vec<CreateElicitationRequest>>>,
        reply: CreateElicitationResponse,
    }

    impl FakeSender {
        fn new(reply: CreateElicitationResponse) -> Self {
            Self {
                received: Arc::new(Mutex::new(Vec::new())),
                reply,
            }
        }
    }

    #[async_trait::async_trait]
    impl ElicitationSender for FakeSender {
        async fn send(
            &self,
            request: CreateElicitationRequest,
        ) -> Result<CreateElicitationResponse, String> {
            self.received.lock().await.push(request);
            Ok(self.reply.clone())
        }
    }

    fn form_params(question: &str) -> CreateElicitationRequestParams {
        let schema = McpElicitationSchema::builder()
            .required_string_with("answer", |s| s.description(question.to_string()))
            .build_unchecked();
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: question.to_string(),
            requested_schema: schema,
        }
    }

    #[tokio::test]
    async fn accept_round_trips_through_the_bridge() {
        let sender = FakeSender::new(accept_with_string("answer", "Ada"));
        let received = sender.received.clone();
        let session = AcpSessionId::new("sess_1");
        let params = form_params("What is your name?");

        let result = bridge_elicitation(Some(&sender), &params, Some(&session), true).await;

        // Exactly one ACP elicitation/create was emitted, form-mode, carrying
        // the original prompt.
        let requests = received.lock().await;
        assert_eq!(
            requests.len(),
            1,
            "expected exactly one ACP request emitted"
        );
        assert_eq!(requests[0].message, "What is your name?");

        // The accept content round-trips into the rmcp result.
        assert_eq!(result.action, McpElicitationAction::Accept);
        let answer = result
            .content
            .as_ref()
            .and_then(|c| c.get("answer"))
            .and_then(|v| v.as_str());
        assert_eq!(answer, Some("Ada"));
    }

    #[tokio::test]
    async fn decline_propagates_through_the_bridge() {
        let sender = FakeSender::new(CreateElicitationResponse::new(ElicitationAction::Decline));
        let session = AcpSessionId::new("sess_1");
        let params = form_params("What is your name?");

        let result = bridge_elicitation(Some(&sender), &params, Some(&session), true).await;

        assert_eq!(result.action, McpElicitationAction::Decline);
        assert!(result.content.is_none());
    }

    #[tokio::test]
    async fn cancel_propagates_through_the_bridge() {
        let sender = FakeSender::new(CreateElicitationResponse::new(ElicitationAction::Cancel));
        let session = AcpSessionId::new("sess_1");
        let params = form_params("What is your name?");

        let result = bridge_elicitation(Some(&sender), &params, Some(&session), true).await;

        assert_eq!(result.action, McpElicitationAction::Cancel);
        assert!(result.content.is_none());
    }

    #[tokio::test]
    async fn no_sender_declines() {
        let session = AcpSessionId::new("sess_1");
        let params = form_params("What is your name?");

        let result = bridge_elicitation(None, &params, Some(&session), true).await;

        assert_eq!(result.action, McpElicitationAction::Decline);
    }

    #[tokio::test]
    async fn unsupported_capability_declines_without_sending() {
        let sender = FakeSender::new(accept_with_string("answer", "Ada"));
        let received = sender.received.clone();
        let session = AcpSessionId::new("sess_1");
        let params = form_params("What is your name?");

        // Client did not advertise the elicitation capability: decline up front
        // and never contact the client, matching claude-agent's bridge.
        let result = bridge_elicitation(Some(&sender), &params, Some(&session), false).await;

        assert_eq!(result.action, McpElicitationAction::Decline);
        assert!(
            received.lock().await.is_empty(),
            "no ACP request should be emitted when the client lacks elicitation support"
        );
    }

    #[test]
    fn form_request_carries_session_scope_and_schema() {
        let session = AcpSessionId::new("sess_42");
        let params = form_params("Pick a value");

        let request = mcp_request_to_acp(&params, &session).unwrap();

        // Serialize and assert the wire shape matches the ACP form-mode contract.
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["mode"], "form");
        assert_eq!(json["sessionId"], "sess_42");
        assert_eq!(json["message"], "Pick a value");
        assert_eq!(json["requestedSchema"]["type"], "object");
        assert_eq!(
            json["requestedSchema"]["properties"]["answer"]["type"],
            "string"
        );
    }
}
