//! Bridge between Claude CLI stream-json elicitation control requests and the
//! ACP `elicitation/create` round-trip.
//!
//! # Why this exists
//!
//! `claude-agent` wraps the Claude Code CLI over stream-json stdio and is the
//! ACP **Agent** to the webview. When the per-board SAH MCP server issues an
//! elicitation, the CLI relays it to its controlling program (us) as a
//! stream-json `control_request` whose `request.subtype` is `"elicitation"`.
//! Before this module there was no code path turning that into an ACP
//! `elicitation/create` request, so the request died in the wrapper.
//!
//! This module owns the *pure* translation between the two protocols, with no
//! I/O and no client connection — exactly the split the protocol translator
//! uses. The orchestration (sending over the ACP client connection and writing
//! the response back to the CLI's stdin) lives in [`crate::claude`] and
//! [`crate::agent`], which hold those resources.
//!
//! # The two wire shapes
//!
//! ## RECEIVE — CLI → claude-agent (on the CLI's stdout)
//!
//! ```json
//! {"type":"control_request","request_id":"<id>","request":{
//!   "subtype":"elicitation",
//!   "mcp_server_name":"<server>",
//!   "message":"<human prompt>",
//!   "mode":"form"|"url",
//!   "url":"<string>",
//!   "elicitation_id":"<string>",
//!   "requested_schema":{...JSON schema...},
//!   "title":"<string>",
//!   "display_name":"<string>",
//!   "description":"<string>"
//! }}
//! ```
//!
//! The CLI control wire is snake_case (`requested_schema`, `elicitation_id`,
//! `mcp_server_name`); the ACP `elicitation/create` method is camelCase
//! (`requestedSchema`, `elicitationId`). This module translates between them.
//!
//! ## RESPOND — claude-agent → CLI (on the CLI's stdin)
//!
//! ```json
//! {"type":"control_response","response":{
//!   "subtype":"success",
//!   "request_id":"<echoed id>",
//!   "response":{"action":"accept"|"decline"|"cancel","content":{...}}
//! }}
//! ```
//!
//! `content` is present only on `accept`. On internal failure the envelope uses
//! `{"subtype":"error","request_id":"<id>","error":"<msg>"}`.
//!
//! These shapes were verified against the installed Claude Code CLI binary's
//! embedded zod schemas and control-response builders (see the task's
//! investigation findings).

use agent_client_protocol::schema::{
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAction, ElicitationFormMode,
    ElicitationSchema, ElicitationSessionScope, SessionId,
};
use serde_json::{json, Value as JsonValue};

/// The result of servicing a CLI elicitation.
///
/// The CLI distinguishes two outcomes on its control wire: a *successful*
/// round-trip carrying the client's `{action, content?}` decision
/// (`subtype: "success"`), and an *error* (`subtype: "error"`) when the
/// elicitation could not be serviced at all. This enum mirrors that split so the
/// stream loop can pick the correct envelope:
///
/// * [`ElicitationOutcome::Responded`] — the client returned a genuine decision
///   (accept / decline / cancel), or the bridge decided locally to decline
///   (no client wired, capability not advertised). These are real answers and
///   map to the `success` envelope.
/// * [`ElicitationOutcome::Error`] — an infrastructure failure prevented a
///   decision from being obtained (transport failure relaying the request, or a
///   request/response (de)serialization failure). These map to the `error`
///   envelope so the CLI observes a failure rather than mistaking it for a clean
///   user cancellation.
#[derive(Debug, Clone)]
pub enum ElicitationOutcome {
    /// The elicitation was answered (by the client, or by a local decline).
    Responded(CreateElicitationResponse),
    /// An infrastructure failure prevented obtaining an answer; the payload is a
    /// human-readable description for the CLI's error envelope.
    Error(String),
}

/// Services a Claude CLI elicitation by relaying it to the ACP client.
///
/// The streaming loop in [`crate::claude`] reads the CLI's stdout but does not
/// hold the ACP client connection — that lives on [`crate::agent::ClaudeAgent`].
/// This trait is the seam between the two: the agent installs an implementation
/// that performs the `elicitation/create` round-trip over its stored
/// `ConnectionTo<Client>`, and the stream loop invokes it whenever it sees an
/// elicitation control request.
///
/// Implementations decide the outcome — relay to the client, decline when no
/// client is wired or the client did not advertise the elicitation capability,
/// or report an [`ElicitationOutcome::Error`] when the round-trip fails at the
/// infrastructure level. They must always return an [`ElicitationOutcome`]
/// (never hang) so the CLI is promptly unblocked.
#[async_trait::async_trait]
pub trait ElicitationHandler: Send + Sync {
    /// Handle one CLI elicitation and return the outcome to relay back.
    ///
    /// # Arguments
    ///
    /// * `request` - The parsed CLI elicitation control request.
    /// * `session_id` - The ACP session id the elicitation belongs to.
    async fn handle_elicitation(
        &self,
        request: &CliElicitationRequest,
        session_id: SessionId,
    ) -> ElicitationOutcome;
}

/// A parsed Claude CLI elicitation `control_request`.
///
/// Carries the snake_case fields the CLI surfaces on its stdout. Only the
/// fields this bridge actually uses are extracted; unknown fields are ignored
/// so a future CLI addition does not break parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct CliElicitationRequest {
    /// The JSON-RPC `request_id` to echo back in the control_response.
    pub request_id: String,
    /// The human-readable prompt describing what input is needed.
    pub message: String,
    /// The elicitation mode the CLI requested. Absent in the wire is treated as
    /// `"form"` (the only mode this bridge renders against the ACP client).
    pub mode: String,
    /// The JSON Schema describing the form fields, when the CLI provided one.
    ///
    /// Snake_case `requested_schema` on the wire. Carried as a raw
    /// [`serde_json::Value`] and handed to the ACP [`ElicitationSchema`]
    /// deserializer so the schema round-trips faithfully.
    pub requested_schema: Option<JsonValue>,
}

impl CliElicitationRequest {
    /// Parse a stream-json line into a [`CliElicitationRequest`] when it is an
    /// elicitation control request, otherwise `None`.
    ///
    /// Returns `None` for any line that is not valid JSON, is not a
    /// `control_request`, lacks a `request_id`, or whose `request.subtype` is
    /// not `"elicitation"`. This lets the stream loop call it on every line and
    /// only divert the ones that are genuinely elicitations.
    ///
    /// # Arguments
    ///
    /// * `line` - A single newline-delimited JSON line from the CLI's stdout.
    pub fn parse(line: &str) -> Option<Self> {
        let parsed: JsonValue = serde_json::from_str(line).ok()?;

        if parsed.get("type").and_then(JsonValue::as_str) != Some("control_request") {
            return None;
        }

        let request = parsed.get("request")?;
        if request.get("subtype").and_then(JsonValue::as_str) != Some("elicitation") {
            return None;
        }

        let request_id = parsed.get("request_id").and_then(JsonValue::as_str)?;
        let message = request
            .get("message")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string();
        let mode = request
            .get("mode")
            .and_then(JsonValue::as_str)
            .unwrap_or("form")
            .to_string();
        let requested_schema = request.get("requested_schema").cloned();

        Some(Self {
            request_id: request_id.to_string(),
            message,
            mode,
            requested_schema,
        })
    }

    /// Whether this elicitation is in (or defaults to) form mode.
    ///
    /// The bridge only renders form-mode elicitations against the ACP client.
    /// An absent mode is treated as form (matching the CLI default captured in
    /// [`Self::parse`]); any other mode (notably `"url"`) is not form.
    pub fn is_form_mode(&self) -> bool {
        self.mode == "form"
    }

    /// Build the ACP [`CreateElicitationRequest`] for this CLI elicitation.
    ///
    /// The request is always built in **form** mode tied to the supplied ACP
    /// session. The CLI elicitation subtype carries no ACP tool-call id (it is
    /// scoped to an MCP server, not an ACP tool call), so the scope is left
    /// without a `tool_call_id`.
    ///
    /// The CLI's `requested_schema` (when present and parseable as an ACP
    /// [`ElicitationSchema`]) is forwarded verbatim; an absent or unparseable
    /// schema yields an empty schema so the client still receives a well-formed
    /// form request rather than the bridge failing.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The ACP session id to scope the elicitation to.
    pub fn to_acp_request(&self, session_id: SessionId) -> CreateElicitationRequest {
        let schema = self
            .requested_schema
            .as_ref()
            .and_then(|value| serde_json::from_value::<ElicitationSchema>(value.clone()).ok())
            .unwrap_or_default();

        let scope = ElicitationSessionScope::new(session_id);
        let mode = ElicitationFormMode::new(scope, schema);

        CreateElicitationRequest::new(mode, self.message.clone())
    }

    /// Build the CLI `control_response` JSON for a serviced elicitation outcome.
    ///
    /// Routes the two [`ElicitationOutcome`] variants to the two CLI envelopes:
    /// a [`Responded`](ElicitationOutcome::Responded) decision becomes the
    /// `success` envelope (via [`Self::success_control_response`]), and an
    /// [`Error`](ElicitationOutcome::Error) becomes the `error` envelope (via
    /// [`Self::error_control_response`]). This keeps the stream loop from having
    /// to mistranslate an infrastructure failure as a user decision.
    ///
    /// # Arguments
    ///
    /// * `outcome` - The result of servicing this elicitation.
    pub fn control_response_for_outcome(&self, outcome: &ElicitationOutcome) -> JsonValue {
        match outcome {
            ElicitationOutcome::Responded(response) => self.success_control_response(response),
            ElicitationOutcome::Error(message) => self.error_control_response(message.clone()),
        }
    }

    /// Build the CLI `control_response` JSON for an ACP elicitation response.
    ///
    /// Maps the ACP [`CreateElicitationResponse`] action back into the
    /// `{action, content?}` payload the CLI's SDK consumer expects, wrapped in
    /// the `control_response` success envelope with this request's id echoed.
    ///
    /// `content` is included only for `accept`, mirroring the ACP type where
    /// content rides exclusively on the accept action.
    ///
    /// # Arguments
    ///
    /// * `response` - The client's elicitation response to relay to the CLI.
    pub fn success_control_response(&self, response: &CreateElicitationResponse) -> JsonValue {
        let payload = elicitation_action_to_payload(&response.action);
        success_envelope(&self.request_id, payload)
    }

    /// Build a CLI `control_response` that declines this elicitation.
    ///
    /// Used when the client cannot service the elicitation — no client
    /// connection is wired, or the client did not advertise the `elicitation`
    /// capability — so the CLI is unblocked promptly with a decline instead of
    /// hanging until its timeout.
    pub fn decline_control_response(&self) -> JsonValue {
        success_envelope(&self.request_id, json!({ "action": "decline" }))
    }

    /// Build a CLI `control_response` reporting an internal error.
    ///
    /// Used when relaying the elicitation to the client failed (transport
    /// error). The CLI surfaces the error rather than the bridge silently
    /// declining, which would hide the failure.
    ///
    /// # Arguments
    ///
    /// * `message` - A human-readable error description.
    pub fn error_control_response(&self, message: impl Into<String>) -> JsonValue {
        json!({
            "type": "control_response",
            "response": {
                "subtype": "error",
                "request_id": self.request_id,
                "error": message.into(),
            }
        })
    }
}

/// Wrap an inner `{action, content?}` payload in the CLI success envelope.
fn success_envelope(request_id: &str, payload: JsonValue) -> JsonValue {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": payload,
        }
    })
}

/// Translate an ACP [`ElicitationAction`] into the CLI's `{action, content?}`.
///
/// `accept` carries its content object (omitted when the accept provided none);
/// `decline` and `cancel` carry only the action discriminator.
fn elicitation_action_to_payload(action: &ElicitationAction) -> JsonValue {
    match action {
        ElicitationAction::Accept(accept) => match &accept.content {
            Some(content) => json!({ "action": "accept", "content": content }),
            None => json!({ "action": "accept" }),
        },
        ElicitationAction::Decline => json!({ "action": "decline" }),
        ElicitationAction::Cancel => json!({ "action": "cancel" }),
        // `ElicitationAction` is `#[non_exhaustive]`; an unknown future variant
        // is treated as a decline so the CLI is still unblocked correctly.
        _ => json!({ "action": "decline" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{
        ElicitationAcceptAction, ElicitationContentValue, ElicitationMode,
    };
    use std::collections::BTreeMap;

    /// A representative elicitation control_request line as the CLI emits it,
    /// with the snake_case fields and a one-field form schema.
    fn elicitation_line() -> String {
        json!({
            "type": "control_request",
            "request_id": "req_abc123",
            "request": {
                "subtype": "elicitation",
                "mcp_server_name": "per-board-kanban",
                "message": "What is your name?",
                "mode": "form",
                "requested_schema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "title": "Name" }
                    },
                    "required": ["name"]
                }
            }
        })
        .to_string()
    }

    #[test]
    fn parse_extracts_elicitation_fields() {
        let req = CliElicitationRequest::parse(&elicitation_line())
            .expect("an elicitation control_request must parse");

        assert_eq!(req.request_id, "req_abc123");
        assert_eq!(req.message, "What is your name?");
        assert_eq!(req.mode, "form");
        assert!(req.requested_schema.is_some());
    }

    #[test]
    fn parse_defaults_mode_to_form_when_absent() {
        let line = json!({
            "type": "control_request",
            "request_id": "req_1",
            "request": { "subtype": "elicitation", "message": "hi" }
        })
        .to_string();

        let req = CliElicitationRequest::parse(&line).expect("must parse without mode");
        assert_eq!(req.mode, "form");
        assert!(req.requested_schema.is_none());
    }

    #[test]
    fn parse_rejects_non_control_request() {
        let line = r#"{"type":"assistant","message":{"content":[]}}"#;
        assert!(CliElicitationRequest::parse(line).is_none());
    }

    #[test]
    fn parse_rejects_non_elicitation_subtype() {
        let line = json!({
            "type": "control_request",
            "request_id": "req_2",
            "request": { "subtype": "can_use_tool", "tool_name": "Bash" }
        })
        .to_string();
        assert!(CliElicitationRequest::parse(&line).is_none());
    }

    #[test]
    fn parse_rejects_malformed_json() {
        assert!(CliElicitationRequest::parse(r#"{"type":"control_request", bad"#).is_none());
    }

    #[test]
    fn parse_rejects_missing_request_id() {
        let line = json!({
            "type": "control_request",
            "request": { "subtype": "elicitation", "message": "hi" }
        })
        .to_string();
        assert!(CliElicitationRequest::parse(&line).is_none());
    }

    #[test]
    fn to_acp_request_builds_form_mode_with_schema() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let acp = req.to_acp_request(SessionId::new("sess_1"));

        assert_eq!(acp.message, "What is your name?");
        match &acp.mode {
            ElicitationMode::Form(form) => {
                // The schema's single property must survive the translation.
                assert!(form.requested_schema.properties.contains_key("name"));
            }
            other => panic!("expected form mode, got {other:?}"),
        }

        // Scope must be the session with no tool call id.
        assert_eq!(
            *acp.scope(),
            ElicitationSessionScope::new("sess_1").into(),
            "elicitation must be scoped to the session without a tool_call_id"
        );

        // The wire form must carry the camelCase ACP keys.
        let wire = serde_json::to_value(&acp).unwrap();
        assert_eq!(wire["sessionId"], "sess_1");
        assert_eq!(wire["mode"], "form");
        assert!(wire["requestedSchema"]["properties"]["name"].is_object());
    }

    #[test]
    fn to_acp_request_tolerates_absent_schema() {
        let line = json!({
            "type": "control_request",
            "request_id": "req_3",
            "request": { "subtype": "elicitation", "message": "no schema" }
        })
        .to_string();
        let req = CliElicitationRequest::parse(&line).unwrap();
        let acp = req.to_acp_request(SessionId::new("sess_2"));

        match &acp.mode {
            ElicitationMode::Form(form) => assert!(form.requested_schema.properties.is_empty()),
            other => panic!("expected form mode, got {other:?}"),
        }
    }

    #[test]
    fn success_response_maps_accept_with_content() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let response = CreateElicitationResponse::new(ElicitationAction::Accept(
            ElicitationAcceptAction::new().content(BTreeMap::from([(
                "name".to_string(),
                ElicitationContentValue::from("Alice"),
            )])),
        ));

        let wire = req.success_control_response(&response);

        assert_eq!(wire["type"], "control_response");
        assert_eq!(wire["response"]["subtype"], "success");
        assert_eq!(wire["response"]["request_id"], "req_abc123");
        assert_eq!(wire["response"]["response"]["action"], "accept");
        assert_eq!(wire["response"]["response"]["content"]["name"], "Alice");
    }

    #[test]
    fn success_response_maps_accept_without_content() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let response = CreateElicitationResponse::new(ElicitationAction::Accept(
            ElicitationAcceptAction::new(),
        ));

        let wire = req.success_control_response(&response);

        assert_eq!(wire["response"]["response"]["action"], "accept");
        assert!(
            wire["response"]["response"].get("content").is_none(),
            "an accept with no content must omit the content key"
        );
    }

    #[test]
    fn success_response_maps_decline() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let response = CreateElicitationResponse::new(ElicitationAction::Decline);

        let wire = req.success_control_response(&response);

        assert_eq!(wire["response"]["subtype"], "success");
        assert_eq!(wire["response"]["request_id"], "req_abc123");
        assert_eq!(wire["response"]["response"]["action"], "decline");
        assert!(wire["response"]["response"].get("content").is_none());
    }

    #[test]
    fn success_response_maps_cancel() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let response = CreateElicitationResponse::new(ElicitationAction::Cancel);

        let wire = req.success_control_response(&response);

        assert_eq!(wire["response"]["response"]["action"], "cancel");
        assert!(wire["response"]["response"].get("content").is_none());
    }

    #[test]
    fn decline_control_response_echoes_request_id() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let wire = req.decline_control_response();

        assert_eq!(wire["type"], "control_response");
        assert_eq!(wire["response"]["subtype"], "success");
        assert_eq!(wire["response"]["request_id"], "req_abc123");
        assert_eq!(wire["response"]["response"]["action"], "decline");
    }

    #[test]
    fn error_control_response_carries_message() {
        let req = CliElicitationRequest::parse(&elicitation_line()).unwrap();
        let wire = req.error_control_response("client transport failed");

        assert_eq!(wire["response"]["subtype"], "error");
        assert_eq!(wire["response"]["request_id"], "req_abc123");
        assert_eq!(wire["response"]["error"], "client transport failed");
    }
}
