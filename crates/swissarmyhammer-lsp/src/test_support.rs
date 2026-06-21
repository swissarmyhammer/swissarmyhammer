//! Shared, model-free test doubles for the LSP wire protocol.
//!
//! [`FakeTransport`] is an in-memory [`LspTransport`](crate::client::LspTransport)
//! that records every request and notification it is handed and replays a
//! scripted queue of responses. It is the rust-analyzer-free seam the
//! client-level parsing tests and the [`LspSession`](crate::session::LspSession)
//! open-document state-machine test drive, so neither needs a real language
//! server. It lives in one place so the two test modules share a single fake
//! rather than each re-implementing one.

use std::collections::VecDeque;

use serde_json::Value;

use crate::client::LspTransport;
use crate::error::LspError;

/// An in-memory [`LspTransport`] for unit tests.
///
/// It records every request/notification it is handed and replays a scripted
/// queue of responses for `send_request` and a separate queue of
/// server-initiated messages for `read_message`. Both record vectors are
/// public to the crate so tests can assert on the exact wire traffic the
/// higher-level state machine produced.
#[derive(Default)]
pub(crate) struct FakeTransport {
    /// `(method, params)` of every request that was sent, in order.
    pub sent_requests: Vec<(String, Value)>,
    /// `(method, params)` of every notification that was sent, in order.
    pub sent_notifications: Vec<(String, Value)>,
    /// Scripted responses returned by `send_request`, in FIFO order.
    pub responses: VecDeque<Value>,
    /// Scripted server messages returned by `read_message`, in FIFO order.
    pub incoming: VecDeque<Value>,
    /// When `true`, every `send_notification` records the call but returns an
    /// error, so tests can drive the notify-failure branches of higher-level
    /// state machines (e.g. a `didClose` that does not reach the server).
    pub fail_notifications: bool,
}

impl FakeTransport {
    /// Queue one scripted response, returned by the next `send_request`.
    pub fn with_response(mut self, response: Value) -> Self {
        self.responses.push_back(response);
        self
    }

    /// Queue one scripted server message, returned by the next `read_message`.
    pub fn with_incoming(mut self, message: Value) -> Self {
        self.incoming.push_back(message);
        self
    }

    /// Count notifications recorded for the given method.
    pub fn notification_count(&self, method: &str) -> usize {
        crate::client::count_recorded_method(&self.sent_notifications, method)
    }
}

impl LspTransport for FakeTransport {
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
        self.sent_requests.push((method.to_string(), params));
        self.responses
            .pop_front()
            .ok_or_else(|| LspError::JsonRpc(format!("no scripted response for {}", method)))
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
        self.sent_notifications.push((method.to_string(), params));
        if self.fail_notifications {
            return Err(LspError::JsonRpc(format!(
                "forced notify failure for {method}"
            )));
        }
        Ok(())
    }

    fn read_message(&mut self) -> Result<Value, LspError> {
        self.incoming
            .pop_front()
            .ok_or_else(|| LspError::JsonRpc("no scripted incoming message".into()))
    }
}
