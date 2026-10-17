//! Shared, model-free test doubles for the LSP wire protocol.
//!
//! [`FakeTransport`] is an in-memory [`LspTransport`](crate::client::LspTransport)
//! that records every request and notification it is handed and replays a
//! scripted queue of responses. It is the rust-analyzer-free seam the
//! client-level parsing tests and the [`LspSession`](crate::session::LspSession)
//! open-document state-machine test drive, so neither needs a real language
//! server. It lives in one place so the test modules share a single fake
//! rather than each re-implementing one.
//!
//! This crate's own tests compile the module. Another crate's tests reach it
//! through the `test-support` feature, for example the indexing worker tests in
//! `swissarmyhammer-code-context`.

use std::collections::{HashMap, VecDeque};

use serde_json::Value;

use crate::client::LspTransport;
use crate::error::LspError;

/// One message that a [`FakeTransport`] got, in wire order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WireMessage {
    /// A request with this method.
    Request(String),
    /// A notification with this method.
    Notification(String),
}

/// An in-memory [`LspTransport`] for unit tests.
///
/// It records every request/notification it is handed and replays a scripted
/// queue of responses for `send_request` and a separate queue of
/// server-initiated messages for `read_message`. The record fields are public
/// so tests can assert on the exact wire traffic the higher-level state machine
/// produced.
///
/// `send_request` answers from the first source that has an answer: the
/// scripted `responses` queue, then `method_responses` by method, then
/// `fallback_response`. With no answer, it gives an error.
#[derive(Debug, Clone, Default)]
pub struct FakeTransport {
    /// `(method, params)` of every request that was sent, in order.
    pub sent_requests: Vec<(String, Value)>,
    /// `(method, params)` of every notification that was sent, in order.
    pub sent_notifications: Vec<(String, Value)>,
    /// Each request and notification, in the order it was sent.
    pub wire: Vec<WireMessage>,
    /// Scripted responses returned by `send_request`, in FIFO order.
    pub responses: VecDeque<Value>,
    /// Responses by request method, used when `responses` is empty. Each
    /// request with the method gets a copy.
    pub method_responses: HashMap<String, Value>,
    /// The response to a request that no other source answers.
    pub fallback_response: Option<Value>,
    /// Scripted server messages returned by `read_message`, in FIFO order.
    pub incoming: VecDeque<Value>,
    /// When `true`, every `send_request` records the call but returns an
    /// error, so tests can drive the request-failure branches of higher-level
    /// flows (e.g. an indexing request that fails).
    pub fail_requests: bool,
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

    /// Answer each request with `method` with `response`, when the scripted
    /// queue is empty.
    pub fn with_method_response(mut self, method: &str, response: Value) -> Self {
        self.method_responses.insert(method.to_string(), response);
        self
    }

    /// Answer each request that no other source answers with `response`.
    pub fn with_fallback_response(mut self, response: Value) -> Self {
        self.fallback_response = Some(response);
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

    /// The answer to a request with `method`, from the first source that has
    /// one.
    fn answer(&mut self, method: &str) -> Option<Value> {
        self.responses
            .pop_front()
            .or_else(|| self.method_responses.get(method).cloned())
            .or_else(|| self.fallback_response.clone())
    }
}

impl LspTransport for FakeTransport {
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
        self.sent_requests.push((method.to_string(), params));
        self.wire.push(WireMessage::Request(method.to_string()));
        if self.fail_requests {
            return Err(LspError::JsonRpc(format!(
                "forced request failure for {method}"
            )));
        }
        self.answer(method)
            .ok_or_else(|| LspError::JsonRpc(format!("no scripted response for {}", method)))
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
        self.sent_notifications.push((method.to_string(), params));
        self.wire
            .push(WireMessage::Notification(method.to_string()));
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
