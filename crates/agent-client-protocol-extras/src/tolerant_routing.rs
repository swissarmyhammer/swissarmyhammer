//! Tolerant response routing — keep a connection alive when a turn is abandoned.
//!
//! # The failure this guards against
//!
//! `agent-client-protocol`'s dispatch loop routes an incoming JSON-RPC
//! *response* to the local awaiter (the oneshot behind
//! [`SentRequest::block_task`](agent_client_protocol::SentRequest::block_task)).
//! When a caller abandons a turn — e.g. wraps `block_task()` in
//! `tokio::time::timeout` and the timeout fires — that oneshot receiver is
//! dropped while the request is still in flight. When the counterpart finally
//! responds, the default routing fallback fails with
//! `"failed to send response, receiver dropped"`, and the dispatch loop treats
//! that as a CONNECTION-fatal error: `connect_with` returns `Err` and every
//! other in-flight and future turn on the connection dies with it.
//!
//! That is exactly the cascade observed in production: one review fan-out task
//! hit its per-turn timeout, the agent's late response had nowhere to go, and
//! the whole review connection was torn down.
//!
//! # What this module does
//!
//! [`TolerantResponseRouter`] is a [`HandleDispatchFrom`] middleware that
//! claims every [`Dispatch::Response`] and forwards it to its awaiter exactly
//! like the default fallback — but when delivery fails because the awaiter is
//! gone, it logs and swallows the error instead of letting it kill the
//! dispatch loop. A dropped receiver therefore fails *that turn only*; the
//! connection keeps serving every other turn.
//!
//! Requests and notifications are declined untouched, so the rest of the
//! handler chain (and the role's default handlers) see them as usual.
//!
//! # Usage
//!
//! ```ignore
//! Client
//!     .builder()
//!     .name("my-client")
//!     .with_handler(TolerantResponseRouter)
//!     .connect_with(agent, async |cx| { /* drive turns */ })
//!     .await?;
//! ```

use agent_client_protocol::{ConnectionTo, Dispatch, HandleDispatchFrom, Handled, Role};

/// Dispatch middleware that makes response delivery to a dropped awaiter
/// non-fatal for the connection.
///
/// Claims [`Dispatch::Response`] and forwards the result to the awaiting
/// oneshot (the same thing the dispatch loop's default fallback does). If the
/// awaiter has been dropped — an abandoned turn — the delivery error is logged
/// at WARN and swallowed, so only that turn is lost. All other dispatches are
/// declined and flow to the rest of the chain.
///
/// The unit struct is its own handler; register it with
/// [`Builder::with_handler`](agent_client_protocol::Builder::with_handler).
pub struct TolerantResponseRouter;

impl<Counterpart: Role> HandleDispatchFrom<Counterpart> for TolerantResponseRouter {
    async fn handle_dispatch_from(
        &mut self,
        message: Dispatch,
        _connection: ConnectionTo<Counterpart>,
    ) -> Result<Handled<Dispatch>, agent_client_protocol::Error> {
        match message {
            Dispatch::Response(result, router) => {
                let method = router.method().to_string();
                let id = router.id();
                if let Err(delivery_error) = router.respond_with_result(result) {
                    // The awaiter is gone — an abandoned turn (e.g. a caller's
                    // per-turn timeout dropped its `block_task` future). Losing
                    // this one response is expected; killing the connection's
                    // dispatch loop over it is not.
                    tracing::warn!(
                        %method,
                        ?id,
                        error = ?delivery_error,
                        "response arrived for an abandoned request; \
                         dropping it and keeping the connection alive"
                    );
                }
                Ok(Handled::Yes)
            }
            other => Ok(Handled::No {
                message: other,
                retry: false,
            }),
        }
    }

    fn describe_chain(&self) -> impl std::fmt::Debug {
        "TolerantResponseRouter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use agent_client_protocol::schema::{
        ContentBlock, PromptRequest, PromptResponse, SessionId, StopReason, TextContent,
    };
    use agent_client_protocol::{Agent, Client, ConnectionTo, Responder};
    use tokio::sync::Notify;

    /// Build a scripted in-process agent whose FIRST prompt response is held
    /// back until `release_first` is notified; every later prompt responds
    /// immediately. This reproduces the production shape: the client abandons
    /// turn 1 (drops its response receiver), then the agent's late response
    /// arrives while turn 2 is outstanding.
    fn scripted_agent(
        release_first: Arc<Notify>,
    ) -> impl agent_client_protocol::ConnectTo<Client> + 'static {
        let prompt_count = Arc::new(AtomicUsize::new(0));
        Agent.builder().name("scripted-agent").on_receive_request(
            {
                move |req: PromptRequest,
                      responder: Responder<PromptResponse>,
                      _cx: ConnectionTo<Client>| {
                    let release_first = Arc::clone(&release_first);
                    let prompt_count = Arc::clone(&prompt_count);
                    async move {
                        let _ = req;
                        if prompt_count.fetch_add(1, Ordering::SeqCst) == 0 {
                            release_first.notified().await;
                        }
                        responder.respond(PromptResponse::new(StopReason::EndTurn))
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
    }

    fn prompt_request(text: &str) -> PromptRequest {
        PromptRequest::new(
            SessionId::new("tolerant-routing-test"),
            vec![ContentBlock::Text(TextContent::new(text))],
        )
    }

    /// The regression for the production cascade: turn 1 is abandoned (its
    /// `block_task` future — and thus its response receiver — is dropped by a
    /// timeout); the agent's late response to it must fail that turn only.
    /// Turn 2 on the SAME connection must still complete, and `connect_with`
    /// must return cleanly instead of
    /// `"failed to send response, receiver dropped"`.
    #[tokio::test]
    async fn abandoned_turn_does_not_kill_the_connection() {
        let release_first = Arc::new(Notify::new());
        let agent = scripted_agent(Arc::clone(&release_first));

        let connect_result = Client
            .builder()
            .name("tolerant-routing-test-client")
            .with_handler(TolerantResponseRouter)
            .connect_with(agent, async |cx: ConnectionTo<Agent>| {
                // Turn 1: send, then abandon the await — exactly what a
                // per-turn timeout around `block_task()` does in production.
                let abandoned = tokio::time::timeout(
                    Duration::from_millis(50),
                    cx.send_request(prompt_request("turn 1")).block_task(),
                )
                .await;
                assert!(abandoned.is_err(), "turn 1 must time out (be abandoned)");

                // Let the agent answer the abandoned turn now. Its response
                // arrives at our dispatch loop with the receiver dropped.
                release_first.notify_one();

                // Turn 2 on the same connection must still succeed.
                let response = tokio::time::timeout(
                    Duration::from_secs(5),
                    cx.send_request(prompt_request("turn 2")).block_task(),
                )
                .await
                .expect("turn 2 must not hang")?;
                assert_eq!(response.stop_reason, StopReason::EndTurn);
                Ok(())
            })
            .await;

        assert!(
            connect_result.is_ok(),
            "an abandoned turn must fail that turn only, not the connection: {:?}",
            connect_result.err()
        );
    }
}
