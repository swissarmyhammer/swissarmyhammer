//! Every `session/prompt` exit that returns without running a turn still emits
//! the end-of-turn marker.
//!
//! A client that reassembles a turn's reply from streamed chunks drains the
//! session's notification stream until the agent's end-of-turn marker arrives
//! (see [`claude_agent::collect_response_content`]). A `prompt` call that
//! returns WITHOUT emitting the marker leaves that client waiting for the
//! drain's hang guard — [`claude_agent::NOTIFICATION_DRAIN_BACKSTOP_MS`] — and
//! then failing, even though the turn is over and the caller already has its
//! answer.
//!
//! `ClaudeAgent::prompt` emits the marker on ONE unconditional path, so the
//! exits that need pinning are the early ones, which is where an
//! implementation forgets the marker.
//!
//! # What these tests pin
//!
//! Three tests cover the three exits that return before the turn streams
//! anything at all — everything reachable before `run_prompt_turn` calls
//! `send_user_message_chunks`. That set is complete: `prompt` runs only
//! `resolve_session` ahead of the turn body, and the turn body runs only
//! `validate_prompt_request` ahead of the first chunk.
//!
//! | exit | result |
//! |------|--------|
//! | the session id resolves to no session | `invalid_params` |
//! | the session store cannot be read | `internal_error` (-32603) |
//! | request validation rejects the prompt | `invalid_params` |
//!
//! A fourth test covers the first exit PAST that point, and the only one of
//! them that is reachable without the claude CLI: a session cancelled before
//! the prompt arrives returns `Ok(StopReason::Cancelled)` after the user
//! message is streamed but before the model runs.
//!
//! # What these tests do NOT pin
//!
//! `run_prompt_turn` returns without running the model at four further exits,
//! and no test here covers any of them: `prepare_session_for_turn`,
//! `check_turn_limits` and `get_updated_session` each return a `prompt` error,
//! and a turn that trips its turn limit returns `Ok`. Neither does any test
//! here drive a turn that reaches the backend — that needs the claude CLI,
//! which these tests deliberately never spawn.
//!
//! Every one of those exits leaves through the very same emit, so this module
//! pins the rule at the exits an implementation is likeliest to miss, not every
//! path that obeys it. The one way past the emit is a panic or a dropped
//! future, which is why a client's drain keeps its hang guard at all.
//!
//! # Which id the marker is addressed to
//!
//! `prompt` picks the marker's address BEFORE it knows whether resolution
//! succeeded: a resolved session gets its canonical id, and an unresolved one
//! gets the id the client literally sent, because an unresolved id has no
//! canonical form. Every test therefore subscribes its collector on the id it
//! sends, never on a resolved id.
//!
//! Only the unresolvable-id test can tell those two rules apart. Wherever the
//! session resolves, `resolve_session` found it by parsing the client's own id,
//! so the canonical `session.id.to_string()` and the id the client sent are the
//! SAME string, and no assertion could say which of them the marker carried.
//! The unresolvable-id test names an id no session was ever created for, so the
//! client's id is the only address that can exist — and its drain finishing is
//! what proves the marker carried that address.
//!
//! # Why `worker_threads = 2`
//!
//! The multi-threaded flavor is what makes the marker cross a real thread
//! boundary: the collector is a `tokio::spawn`ed task, so it is polled on a
//! runtime worker while the test body runs on the thread that called
//! `block_on`. That is the production shape — the agent emits on one thread and
//! a client's collector observes on another. Two workers rather than one let
//! the collector and the tasks the agent spawns be polled at the same time
//! instead of in turn.
//!
//! The count is a deliberate choice, not a passing condition: these tests also
//! pass under `flavor = "current_thread"`, where the collector shares the test
//! body's thread and nothing crosses threads at all.
//!
//! The literal cannot be replaced by a named constant. `tokio::test` reads
//! `worker_threads` at macro-expansion time, and `tokio-macros` (2.7.0, the
//! locked version) matches the argument value against `syn::Expr::Lit`,
//! returning "Must be a literal" for anything else. A constant path is a
//! `syn::Expr::Path`, so `worker_threads = TEST_WORKER_THREADS` does not
//! compile, and the `2` has to be written out at each attribute.

use agent_client_protocol::schema::{
    CancelNotification, ContentBlock, NewSessionRequest, PromptRequest, PromptResponse, SessionId,
    SessionNotification, StopReason, TextContent,
};
use agent_client_protocol::ErrorCode;
use claude_agent::{config::AgentConfig, ClaudeAgent};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// How long the drain may take before the test calls it a hang.
///
/// Far below [`claude_agent::NOTIFICATION_DRAIN_BACKSTOP_MS`] (10 s): the drain
/// must end on the marker, never on the hang guard. Generous enough to absorb
/// scheduling latency on a loaded machine, small enough that hitting the
/// backstop cannot pass for success.
const DRAIN_MUST_FINISH_WITHIN: Duration = Duration::from_secs(2);

/// A well-formed ULID for which no session is ever created — the "the client
/// holds an id, but the agent has no session for it" miss (an expired or
/// already-cleaned-up session, as a live client would see it).
const UNKNOWN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// Build an agent that never spawns the claude CLI.
///
/// `spawn_claude_on_new_session` off is the headless seam this module shares
/// with `mcp_http_session`: `new_session` records a session without exec'ing
/// the CLI or waiting on its init line. No exit these tests pin ever reaches
/// the model, so no test here needs a live CLI — and must not depend on one
/// being installed.
async fn headless_agent() -> (ClaudeAgent, broadcast::Receiver<SessionNotification>) {
    let config = AgentConfig {
        spawn_claude_on_new_session: false,
        ..Default::default()
    };
    ClaudeAgent::new(config)
        .await
        .expect("agent construction should succeed")
}

/// Create a live session on `agent` and return its canonical id.
async fn new_session(agent: &ClaudeAgent) -> SessionId {
    agent
        .new_session(NewSessionRequest::new(
            std::env::current_dir().expect("a working directory"),
        ))
        .await
        .expect("session creation should succeed")
        .session_id
}

/// Drain the collector and assert it ended on the end-of-turn marker.
///
/// The timeout is the assertion: the collector only stops early on the marker
/// or on a closed channel, so finishing inside [`DRAIN_MUST_FINISH_WITHIN`]
/// means the marker arrived. Timing out means the drain was on its way to the
/// 10 s backstop, i.e. no marker was emitted for this session.
async fn drain_must_end_on_the_marker(
    collector: claude_agent::NotificationCollector,
    exit: &str,
) -> String {
    let prompt_response = PromptResponse::new(StopReason::EndTurn);
    tokio::time::timeout(
        DRAIN_MUST_FINISH_WITHIN,
        claude_agent::collect_response_content(collector, &prompt_response),
    )
    .await
    .unwrap_or_else(|elapsed| {
        panic!(
            "{exit} must emit its end-of-turn marker, not leave the collector waiting for the backstop: {elapsed:?}"
        )
    })
    .expect("the drain reached the end of the turn's stream")
}

/// Poison the agent's in-memory session store so every later read of it fails.
///
/// `SessionManager` keeps its sessions behind a `std::sync::RwLock`, and a
/// panic while a writer holds that lock poisons it: from then on every
/// `get_session` returns an error, which `resolve_session` maps to
/// `internal_error` (-32603). That is the real condition the -32603 path exists
/// for, reproduced through the manager's own public API — no test-only hook is
/// added to production code to stage it.
fn poison_the_session_store(agent: &ClaudeAgent, session_id: &SessionId) {
    let internal_id = claude_agent::session::SessionId::parse(session_id.0.as_ref())
        .expect("the agent minted this id, so it parses");
    let manager = Arc::clone(agent.session_manager());
    let poisoner = Arc::clone(&manager);

    // The panic is the mechanism, not a failure, so silence the default hook
    // for its duration: an unexplained backtrace in passing test output reads
    // as a real crash.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = std::thread::spawn(move || {
        let _ = poisoner.update_session(&internal_id, |_| {
            panic!("deliberately poisoning the session store");
        });
    })
    .join();
    std::panic::set_hook(previous_hook);

    assert!(
        panicked.is_err(),
        "the update closure must have panicked while holding the write lock"
    );
    assert!(
        manager.get_session(&internal_id).is_err(),
        "a poisoned lock must make every session read fail"
    );
}

/// A prompt whose session id resolves to no session still marks its turn
/// complete, addressed to the id the client sent.
///
/// The id never named a session on this agent, so there is no canonical id to
/// address the marker to — only the client's own id, which is what that
/// client's collector is subscribed on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unresolvable_session_id_ends_the_turn_for_a_subscribed_collector() {
    let (agent, notifications) = headless_agent().await;

    // The id the CLIENT sends. Subscribe on exactly that, as a client whose
    // session the agent has since forgotten would.
    let client_session_id = SessionId::new(UNKNOWN_ULID);
    let collector = claude_agent::spawn_notification_collector(
        notifications.resubscribe(),
        client_session_id.clone(),
    );

    let rejected = agent
        .prompt(PromptRequest::new(
            client_session_id,
            vec![ContentBlock::Text(TextContent::new("hello".to_string()))],
        ))
        .await
        .expect_err("a prompt for an unknown session is invalid");
    assert_eq!(
        rejected.code,
        ErrorCode::InvalidParams,
        "an unknown session id is rejected as invalid params: {rejected:?}"
    );

    let content = drain_must_end_on_the_marker(collector, "an unresolvable session id").await;
    assert!(
        content.is_empty(),
        "a prompt for an unknown session streams no reply: {content:?}"
    );
}

/// A prompt whose session store cannot be read still marks its turn complete,
/// addressed to the id the client sent.
///
/// This is the exit that matters most: the session DOES exist and a client is
/// draining it, but the store read failed, so the agent cannot recover the
/// canonical id. Addressing the marker to the id the client sent is what stops
/// that client's collector.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreadable_session_store_ends_the_turn_for_a_subscribed_collector() {
    let (agent, notifications) = headless_agent().await;
    let session = new_session(&agent).await;

    poison_the_session_store(&agent, &session);

    // Subscribe on the id the CLIENT holds — the same id it will send below.
    let collector =
        claude_agent::spawn_notification_collector(notifications.resubscribe(), session.clone());

    let failed = agent
        .prompt(PromptRequest::new(
            session,
            vec![ContentBlock::Text(TextContent::new("hello".to_string()))],
        ))
        .await
        .expect_err("a prompt cannot run while the session store is unreadable");
    assert_eq!(
        failed.code,
        ErrorCode::InternalError,
        "an unreadable session store is a retryable internal error, not a not-found: {failed:?}"
    );

    let content = drain_must_end_on_the_marker(collector, "an unreadable session store").await;
    assert!(
        content.is_empty(),
        "a prompt that never ran streams no reply: {content:?}"
    );
}

/// A prompt rejected by request validation still marks its turn complete, so a
/// collector that subscribed before the prompt stops immediately instead of
/// waiting out the drain's backstop.
///
/// An empty `prompt` array carries no content, which `validate_prompt_request`
/// rejects before any turn work begins, so no turn ever reaches the backend.
/// The session resolved here, so the marker carries the canonical id — the same
/// key the turn's chunks would have carried.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_prompt_ends_the_turn_for_a_subscribed_collector() {
    let (agent, notifications) = headless_agent().await;
    let session = new_session(&agent).await;

    // Subscribe exactly as a pool client does: before the prompt is sent.
    let collector =
        claude_agent::spawn_notification_collector(notifications.resubscribe(), session.clone());

    let rejected = agent
        .prompt(PromptRequest::new(session, vec![]))
        .await
        .expect_err("a prompt with no content is invalid");
    assert_eq!(
        rejected.code,
        ErrorCode::InvalidParams,
        "an empty prompt is rejected as invalid params: {rejected:?}"
    );

    let content = drain_must_end_on_the_marker(collector, "a rejected prompt").await;
    assert!(
        content.is_empty(),
        "a rejected prompt streams no reply: {content:?}"
    );
}

/// A prompt on a session cancelled beforehand still marks its turn complete,
/// even though it returns `Ok` rather than an error.
///
/// This is the first exit PAST the point where the turn starts streaming: the
/// user message goes out as a `UserMessageChunk`, then
/// `check_cancelled_before_processing` returns `Cancelled` and the model is
/// never reached. It pins the marker on a SUCCESSFUL return, so the rule cannot
/// be read as "error paths also emit the marker".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pre_cancelled_session_ends_the_turn_for_a_subscribed_collector() {
    let (agent, notifications) = headless_agent().await;
    let session = new_session(&agent).await;

    agent
        .cancel(CancelNotification::new(session.clone()))
        .await
        .expect("cancelling a live session should succeed");

    // Subscribe after the cancel and before the prompt, so the collector sees
    // only this turn's notifications.
    let collector =
        claude_agent::spawn_notification_collector(notifications.resubscribe(), session.clone());

    let cancelled = agent
        .prompt(PromptRequest::new(
            session,
            vec![ContentBlock::Text(TextContent::new("hello".to_string()))],
        ))
        .await
        .expect("a pre-cancelled prompt returns Ok, not an error");
    assert_eq!(
        cancelled.stop_reason,
        StopReason::Cancelled,
        "a pre-cancelled session stops the turn as cancelled: {cancelled:?}"
    );

    let content = drain_must_end_on_the_marker(collector, "a pre-cancelled session").await;
    assert!(
        content.is_empty(),
        "a turn that never reached the model streams no agent reply: {content:?}"
    );
}
