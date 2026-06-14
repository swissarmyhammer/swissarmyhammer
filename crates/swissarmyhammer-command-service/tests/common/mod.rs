//! Shared test helpers for end-to-end service tests.
//!
//! Mints an rmcp `Peer<RoleServer>` against a closed transport so tests
//! can build a real `RequestContext` and drive
//! `CommandService::call_tool` without spinning up a full transport pair.
//! The helpers in this module are the only place that knows about the
//! peer-minting trick — individual tests just call [`call_tool`] with a
//! [`CallerId`] and assert on the response.

#![allow(dead_code)] // shared by multiple integration test binaries

use std::borrow::Cow;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};
use swissarmyhammer_command_service::{
    CallbackDispatcher, CallbackHandle, CallbackInvokeError, CommandService,
};
use swissarmyhammer_plugin::CallerId;

/// A transport that yields no messages and closes immediately, used solely
/// to mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call
/// needs.
struct ClosedTransport;

impl Transport<RoleServer> for ClosedTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        std::future::ready(Ok(()))
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        std::future::ready(None)
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// Mint an inert `Peer<RoleServer>` by briefly serving a placeholder
/// handler over a closed transport.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Build a `RequestContext` optionally pre-populated with a [`CallerId`]
/// in its extensions.
///
/// `Some(caller)` mirrors what the in-process transport does in
/// production. `None` mirrors an external rmcp transport that doesn't
/// thread a caller through — the service then falls back to
/// [`CallerId::Unknown`].
pub fn request_context_for(caller: Option<CallerId>) -> RequestContext<RoleServer> {
    let mut context = RequestContext::new(NumberOrString::Number(0), mint_peer());
    if let Some(c) = caller {
        context.extensions.insert(c);
    }
    context
}

/// Build a JSON `register command` payload with the given identity and
/// `execute` callback id, with everything else left at sensible defaults.
///
/// The callback id is encoded as the `{"$callback": "<id>"}` wire shape
/// the plugin SDK emits, so the deserializer round-trips through
/// [`swissarmyhammer_command_service::CallbackMarker`].
pub fn register_payload(id: &str, name: &str, execute_callback_id: &str) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": execute_callback_id },
    })
}

/// Build a `register command` payload that ALSO carries an `available`
/// callback marker (some tests need to assert both markers land on the
/// stack entry).
pub fn register_payload_with_available(
    id: &str,
    name: &str,
    execute_callback_id: &str,
    available_callback_id: &str,
) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": execute_callback_id },
        "available": { "$callback": available_callback_id },
    })
}

/// Invoke a `command` tool verb through the service's `ServerHandler`
/// surface, with `caller` planted in the request context's extensions.
///
/// Wraps the rmcp boilerplate so tests read at the verb level: build a
/// `CallToolRequestParams { name: "command", arguments }` and dispatch.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site (e.g. passing the
/// `"register command"` op string with an `"unregister command"`
/// arguments map) is caught immediately instead of silently running the
/// wrong verb.
pub async fn call_tool(
    service: &CommandService,
    op: &str,
    arguments: Value,
    caller: &CallerId,
) -> Result<CallToolResult, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context_for(Some(caller.clone()));
    let mut request = CallToolRequestParams::new(Cow::Borrowed("command"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    service.call_tool(request, context).await
}

/// One recorded invocation against a [`FakeDispatcher`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedInvocation {
    /// Caller the dispatcher was asked to route the callback to.
    pub caller: CallerId,
    /// Callback id from the [`CallbackHandle`].
    pub callback_id: String,
    /// Positional args the verb handler passed through.
    pub args: Value,
}

/// Programmed response for one `(callback_id)` lookup.
///
/// `Reply` lets a test stage either a successful JSON return value, an
/// error message, or an artificial delay before responding (used to drive
/// the latency-budget tests). Delays are additive: the dispatcher sleeps
/// for `delay` first, then either returns `value` or raises `error`.
#[derive(Debug, Clone)]
pub struct Reply {
    /// JSON value the callback should return on success.
    pub value: Value,
    /// Optional error to raise instead of returning `value`.
    pub error: Option<String>,
    /// Optional sleep applied before the dispatcher answers.
    pub delay: Option<Duration>,
}

impl Reply {
    /// Return `value` immediately.
    pub fn ok(value: Value) -> Self {
        Self {
            value,
            error: None,
            delay: None,
        }
    }

    /// Sleep for `delay`, then return `value`.
    pub fn ok_after(value: Value, delay: Duration) -> Self {
        Self {
            value,
            error: None,
            delay: Some(delay),
        }
    }

    /// Fail with `message`.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            value: Value::Null,
            error: Some(message.into()),
            delay: None,
        }
    }
}

/// A fake [`CallbackDispatcher`] for service-level tests.
///
/// Maps callback ids to programmed [`Reply`]s and records every invocation
/// so tests can assert on the order and contents of dispatches without
/// spinning up the real plugin runtime.
#[derive(Debug, Default)]
pub struct FakeDispatcher {
    /// `callback_id` → programmed reply. A missing id surfaces as a
    /// [`CallbackInvokeError`] so tests catch typos in fixture wiring.
    replies: Mutex<std::collections::HashMap<String, Reply>>,
    /// Log of every dispatch, in call order.
    invocations: Mutex<Vec<RecordedInvocation>>,
}

impl FakeDispatcher {
    /// Construct a dispatcher with no programmed replies. Tests typically
    /// follow this with one or more [`Self::program`] calls.
    pub fn new() -> Self {
        Self::default()
    }

    /// Program `reply` for `callback_id`.
    pub fn program(&self, callback_id: impl Into<String>, reply: Reply) {
        self.replies
            .lock()
            .expect("fake dispatcher lock poisoned")
            .insert(callback_id.into(), reply);
    }

    /// Snapshot of every dispatch recorded so far, in call order.
    pub fn recorded(&self) -> Vec<RecordedInvocation> {
        self.invocations
            .lock()
            .expect("fake dispatcher lock poisoned")
            .clone()
    }
}

#[async_trait]
impl CallbackDispatcher for FakeDispatcher {
    async fn invoke(
        &self,
        handle: &CallbackHandle,
        args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        self.invocations
            .lock()
            .expect("fake dispatcher lock poisoned")
            .push(RecordedInvocation {
                caller: handle.caller.clone(),
                callback_id: handle.callback_id.clone(),
                args: args.clone(),
            });

        let reply = self
            .replies
            .lock()
            .expect("fake dispatcher lock poisoned")
            .get(handle.callback_id.as_str())
            .cloned();

        let Some(reply) = reply else {
            return Err(CallbackInvokeError::new(format!(
                "fake dispatcher has no programmed reply for {:?}",
                handle.callback_id
            )));
        };

        if let Some(delay) = reply.delay {
            tokio::time::sleep(delay).await;
        }

        if let Some(message) = reply.error {
            return Err(CallbackInvokeError::new(message));
        }

        Ok(reply.value)
    }
}

/// Construct a [`CommandService`] wired to `dispatcher`.
///
/// Wrapper around the chained constructor so individual tests stay terse.
pub fn service_with_dispatcher(dispatcher: Arc<FakeDispatcher>) -> CommandService {
    CommandService::new().with_dispatcher(dispatcher)
}
