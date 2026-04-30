//! Test scaffolding for mock agents used by the conformance unit tests.
//!
//! ACP 0.11 removed the `agent_client_protocol::Agent` trait. The 0.10 unit
//! tests at the bottom of every `src/*.rs` scenario file used to define a
//! per-file `struct ScenarioMockAgent;` and `impl Agent for ScenarioMockAgent`
//! to drive the public conformance helpers (`test_minimal_initialization`,
//! `test_text_content_support`, …) directly through the trait. With the trait
//! gone, this module supplies a local replacement:
//!
//! - [`MockAgent`] is a project-local trait whose method shapes mirror the
//!   methods the old SDK trait exposed. Default impls return either a stock
//!   no-op response (where the conformance tests treat the call as
//!   uninteresting) or `agent_client_protocol::Error::method_not_found()`.
//! - [`MockAgentAdapter`] is a [`ConnectTo<Client>`] adapter that demultiplexes
//!   incoming `ClientRequest` / `ClientNotification` enums onto the mock's
//!   per-method hooks, so a single adapter shape works for every scenario's
//!   mock.
//! - [`run_with_mock_agent`] wires a [`MockAgent`] up to a fresh `Client` over
//!   an in-process duplex transport and yields a [`ConnectionTo<Agent>`] to
//!   the body, matching the public surface of the new
//!   [`agent_client_protocol_extras::AgentWithFixture::connection`] handle so
//!   conformance helpers can run against either path uniformly.
//!
//! The shape closely mirrors the equivalent pattern in
//! `avp-common/src/validator/runner.rs` — kept identical so reviewers can
//! cross-reference both implementations without surprise.

use agent_client_protocol::schema::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification, ExtRequest,
    ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest, LoadSessionResponse,
    NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse, SetSessionModeRequest,
    SetSessionModeResponse,
};
use agent_client_protocol::{Agent, Channel, Client, ConnectTo, ConnectionTo};
use futures::future::BoxFuture;
use std::sync::Arc;

/// Project-local replacement for the removed `agent_client_protocol::Agent`
/// trait, scoped to the conformance test scaffolding.
///
/// Each method has a default impl so individual mocks only override the
/// methods they actually exercise. Defaults that produce a real response
/// (`initialize`, `authenticate`, `cancel`, `ext_notification`) return stock
/// values; defaults for session-bearing methods return
/// [`agent_client_protocol::Error::method_not_found`] so accidental calls show
/// up as a clearly-named failure rather than silent success.
///
/// The `BoxFuture` return shape matches the SDK's typed handler signature in
/// `Agent.builder().on_receive_request(...)`, which lets [`MockAgentAdapter`]
/// forward without per-mock specialisation.
pub trait MockAgent: Send + Sync {
    /// Handle an `initialize` request. Default returns a stock response with
    /// `protocolVersion: 1` and empty capabilities.
    fn initialize<'a>(
        &'a self,
        _request: InitializeRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<InitializeResponse>> {
        Box::pin(async move { Ok(InitializeResponse::new(1.into())) })
    }

    /// Handle an `authenticate` request. Default returns an empty success.
    fn authenticate<'a>(
        &'a self,
        _request: AuthenticateRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<AuthenticateResponse>> {
        Box::pin(async move { Ok(AuthenticateResponse::new()) })
    }

    /// Handle a `session/new` request. Default returns method-not-found so
    /// scenarios that don't model session creation surface accidental calls
    /// loudly.
    fn new_session<'a>(
        &'a self,
        _request: NewSessionRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
        Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
    }

    /// Handle a `session/load` request. Default returns method-not-found.
    fn load_session<'a>(
        &'a self,
        _request: LoadSessionRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<LoadSessionResponse>> {
        Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
    }

    /// Handle a `session/set_mode` request. Default returns method-not-found.
    fn set_session_mode<'a>(
        &'a self,
        _request: SetSessionModeRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<SetSessionModeResponse>> {
        Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
    }

    /// Handle a `session/prompt` request. Default returns method-not-found.
    fn prompt<'a>(
        &'a self,
        _request: PromptRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
        Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
    }

    /// Handle a `session/cancel` notification. Default succeeds silently.
    fn cancel<'a>(
        &'a self,
        _notification: CancelNotification,
    ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
        Box::pin(async move { Ok(()) })
    }

    /// Handle an extension request whose wire method starts with `_`. Default
    /// returns method-not-found.
    fn ext_method<'a>(
        &'a self,
        _request: ExtRequest,
    ) -> BoxFuture<'a, agent_client_protocol::Result<ExtResponse>> {
        Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
    }

    /// Handle an extension notification. Default succeeds silently.
    fn ext_notification<'a>(
        &'a self,
        _notification: ExtNotification,
    ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
        Box::pin(async move { Ok(()) })
    }
}

/// `ConnectTo<Client>` adapter that drives a [`MockAgent`] as an ACP 0.11
/// agent.
///
/// Spins up an `Agent.builder()` whose `on_receive_request` /
/// `on_receive_notification` handlers demultiplex the incoming `ClientRequest`
/// / `ClientNotification` enums onto the mock's per-method hooks. The builder
/// runs in server-only mode (`connect_to`) so its main loop terminates exactly
/// when the wired client transport closes — no shutdown signal needed.
pub struct MockAgentAdapter<M: MockAgent + 'static>(pub Arc<M>);

impl<M: MockAgent + 'static> ConnectTo<Client> for MockAgentAdapter<M> {
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> agent_client_protocol::Result<()> {
        let mock = Arc::clone(&self.0);
        let mock_for_notifications = Arc::clone(&self.0);

        agent_client_protocol::Agent
            .builder()
            .name("conformance-mock-agent")
            .on_receive_request(
                {
                    let mock = Arc::clone(&mock);
                    async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                        dispatch_mock_request(&mock, req, responder, &cx)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notif: agent_client_protocol::ClientNotification, _cx| {
                    dispatch_mock_notification(&mock_for_notifications, notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

/// Demultiplex an incoming `ClientRequest` onto the mock's per-method
/// handlers.
///
/// Each per-method dispatch is offloaded to `cx.spawn` so the SDK's event
/// loop can keep dispatching new incoming requests while a slow handler is
/// awaiting (matching the production agents in `llama-agent` and
/// `claude-agent`). `ClientRequest` is `#[non_exhaustive]`, so unmodelled
/// variants fall through to method-not-found rather than silent acceptance.
fn dispatch_mock_request<M: MockAgent + 'static>(
    mock: &Arc<M>,
    request: agent_client_protocol::ClientRequest,
    responder: agent_client_protocol::Responder<serde_json::Value>,
    cx: &ConnectionTo<Client>,
) -> agent_client_protocol::Result<()> {
    use agent_client_protocol::ClientRequest as Req;

    let mock = Arc::clone(mock);
    cx.spawn(async move {
        match request {
            Req::InitializeRequest(req) => responder
                .cast()
                .respond_with_result(mock.initialize(req).await),
            Req::AuthenticateRequest(req) => responder
                .cast()
                .respond_with_result(mock.authenticate(req).await),
            Req::NewSessionRequest(req) => responder
                .cast()
                .respond_with_result(mock.new_session(req).await),
            Req::LoadSessionRequest(req) => responder
                .cast()
                .respond_with_result(mock.load_session(req).await),
            Req::SetSessionModeRequest(req) => responder
                .cast()
                .respond_with_result(mock.set_session_mode(req).await),
            Req::PromptRequest(req) => responder
                .cast()
                .respond_with_result(mock.prompt(req).await),
            Req::ExtMethodRequest(req) => {
                let result = mock.ext_method(req).await.and_then(|ext_response| {
                    serde_json::from_str::<serde_json::Value>(ext_response.0.get())
                        .map_err(|_| agent_client_protocol::Error::internal_error())
                });
                responder.respond_with_result(result)
            }
            _ => responder
                .cast::<serde_json::Value>()
                .respond_with_error(agent_client_protocol::Error::method_not_found()),
        }
    })
}

/// Demultiplex an incoming `ClientNotification` onto the mock. Errors are
/// logged inside the per-variant handler and never propagated.
async fn dispatch_mock_notification<M: MockAgent + ?Sized>(
    mock: &Arc<M>,
    notification: agent_client_protocol::ClientNotification,
) {
    use agent_client_protocol::ClientNotification as Notif;

    match notification {
        Notif::CancelNotification(n) => {
            let _ = mock.cancel(n).await;
        }
        Notif::ExtNotification(n) => {
            let _ = mock.ext_notification(n).await;
        }
        _ => {}
    }
}

/// Wire a [`MockAgent`] up to a fresh `Client` and run `body` against the
/// resulting `ConnectionTo<Agent>` handle.
///
/// 1. Builds a `Channel::duplex()` pair of in-process transports.
/// 2. Spawns the mock as an Agent server on one end via [`MockAgentAdapter`].
/// 3. Runs `Client.builder().connect_with(...)` on the other end and invokes
///    `body` with the resulting [`ConnectionTo<Agent>`].
///
/// The agent task is aborted after the client closure returns so we never
/// leak a half-set-up connection if the mock hangs in a `pending()`-style
/// future after its main work completes.
pub async fn run_with_mock_agent<M, F, Fut, R>(mock: Arc<M>, body: F) -> R
where
    M: MockAgent + 'static,
    F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (channel_a, channel_b) = Channel::duplex();

    let agent_task = tokio::spawn(async move {
        let _ = MockAgentAdapter(mock).connect_to(channel_a).await;
    });

    let result = Client
        .builder()
        .name("conformance-mock-client")
        .connect_with(channel_b, async move |conn: ConnectionTo<Agent>| {
            Ok(body(conn).await)
        })
        .await
        .expect("mock agent client connect_with should succeed");

    agent_task.abort();
    let _ = agent_task.await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A no-override mock — exercises the default impls.
    struct DefaultMock;
    impl MockAgent for DefaultMock {}

    #[tokio::test]
    async fn default_mock_initialize_returns_stock_response() {
        let mock = Arc::new(DefaultMock);
        let resp = run_with_mock_agent(mock, |conn| async move {
            conn.send_request(InitializeRequest::new(
                agent_client_protocol::schema::ProtocolVersion::V1,
            ))
            .block_task()
            .await
            .expect("initialize should succeed against default mock")
        })
        .await;
        assert_eq!(
            resp.protocol_version,
            agent_client_protocol::schema::ProtocolVersion::V1
        );
    }

    #[tokio::test]
    async fn default_mock_new_session_returns_method_not_found() {
        let mock = Arc::new(DefaultMock);
        let result = run_with_mock_agent(mock, |conn| async move {
            conn.send_request(NewSessionRequest::new(std::path::PathBuf::from("/tmp")))
                .block_task()
                .await
        })
        .await;
        assert!(result.is_err(), "expected method-not-found, got Ok");
    }
}
