//! Helper for sending ACP 0.11 extension-method requests over the
//! `AgentWithFixture` connection.
//!
//! ACP 0.10 had `Agent::ext_method(ExtRequest) -> ExtResponse`. ACP 0.11
//! drops the trait; ext requests now flow through
//! [`agent_client_protocol::ConnectionTo::send_request`] wrapped in
//! [`agent_client_protocol::ClientRequest::ExtMethodRequest`], with the
//! response dispatched as a raw [`serde_json::Value`].
//!
//! The conformance scenarios still talk in terms of
//! [`agent_client_protocol::schema::ExtResponse`]`(Arc<RawValue>)`, so this
//! helper bridges the two: it re-encodes the wire JSON back into that shape
//! for downstream code.
//!
//! # Wire-method `_` prefix
//!
//! The SDK's [`agent_client_protocol::ClientRequest::parse_message`] only
//! routes `_`-prefixed wire methods to the `ExtMethodRequest` variant —
//! every other method is rejected with `method_not_found` *before* reaching
//! any handler. The receiver then strips the leading `_` so the handler
//! sees the canonical bare method name (e.g. `terminal/create`,
//! `fs/read_text_file`).
//!
//! Conformance scenario callers pass the canonical bare method (matching
//! how `claude-agent` and `llama-agent` switch on it inside their typed
//! `dispatch_*_request` handlers). This helper prepends `_` when
//! constructing the outgoing [`ExtRequest`] so the SDK's parse layer routes
//! the request correctly. Without the prefix the SDK rejects the request
//! at parse time before any agent code runs — that is why the helper
//! exists rather than calling `send_request` inline.

use agent_client_protocol::schema::{ExtRequest, ExtResponse};
use agent_client_protocol::ClientRequest;
use agent_client_protocol_extras::AgentWithFixture;
use std::sync::Arc;

/// Send an `ExtRequest` over the wrapper's connection and reconstitute an
/// [`ExtResponse`] for downstream code.
///
/// The `request.method` is expected to be the canonical bare wire method
/// (e.g. `"terminal/create"`, `"fs/read_text_file"`). This helper prepends
/// `_` to the method when constructing the outgoing
/// [`ClientRequest::ExtMethodRequest`] so the SDK's
/// [`agent_client_protocol::ClientRequest::parse_message`] routes it as an
/// extension method on the receiver side. The receiver strips the `_` back
/// off so its typed handler sees the canonical bare name — matching the
/// production switch tables in `claude-agent::Server::dispatch_ext_request`
/// and `llama-agent::AcpServer::dispatch_ext_request`.
///
/// Returns the wire-shaped [`ExtResponse`] so existing conformance scenario
/// code can keep parsing the response as `Arc<RawValue>` JSON.
pub(crate) async fn send_ext_method(
    agent: &dyn AgentWithFixture,
    request: ExtRequest,
) -> agent_client_protocol::Result<ExtResponse> {
    let wire_request = ExtRequest::new(format!("_{}", request.method), request.params);
    let value: serde_json::Value = agent
        .connection()
        .send_request(ClientRequest::ExtMethodRequest(wire_request))
        .block_task()
        .await?;
    let raw = serde_json::value::to_raw_value(&value)
        .map_err(agent_client_protocol::Error::into_internal_error)?;
    Ok(ExtResponse::new(Arc::from(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{run_with_mock_agent_as_fixture, MockAgent};
    use agent_client_protocol::schema::ExtNotification;
    use futures::future::BoxFuture;
    use std::sync::Mutex;

    /// Mock that captures the `method` field of every incoming
    /// [`ExtRequest`] so we can assert the SDK stripped the `_` prefix
    /// before reaching the handler.
    struct CapturingMock {
        seen_methods: Mutex<Vec<String>>,
    }

    impl CapturingMock {
        fn new() -> Self {
            Self {
                seen_methods: Mutex::new(Vec::new()),
            }
        }
    }

    impl MockAgent for CapturingMock {
        fn ext_method<'a>(
            &'a self,
            request: ExtRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<ExtResponse>> {
            self.seen_methods
                .lock()
                .expect("mutex")
                .push(request.method.to_string());
            Box::pin(async move {
                let raw = serde_json::value::to_raw_value(&serde_json::json!({"ok": true}))
                    .expect("raw value");
                Ok(ExtResponse::new(Arc::from(raw)))
            })
        }

        fn ext_notification<'a>(
            &'a self,
            _n: ExtNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move { Ok(()) })
        }
    }

    /// Verifies that `send_ext_method` correctly reaches the SDK's
    /// `ExtMethodRequest` dispatch by prepending `_` to the wire method,
    /// and that the receiver-side `ExtRequest.method` is the canonical
    /// bare name.
    #[tokio::test]
    async fn ext_method_prefixes_underscore_so_receiver_sees_canonical_name() {
        let mock = Arc::new(CapturingMock::new());
        let captured = Arc::clone(&mock);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            let params = serde_json::value::to_raw_value(&serde_json::json!({"foo": "bar"}))
                .expect("raw value");
            let req = ExtRequest::new("terminal/create", Arc::from(params));
            send_ext_method(&fx, req).await
        })
        .await;
        result.expect("send_ext_method should succeed");
        let seen = captured.seen_methods.lock().expect("mutex").clone();
        assert_eq!(
            seen,
            vec!["terminal/create".to_string()],
            "receiver should see the canonical bare method name (SDK strips the `_` prefix)"
        );
    }
}
