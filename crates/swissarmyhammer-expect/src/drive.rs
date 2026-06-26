//! Engine driver — wire a live ACP agent into the `expect` pipeline.
//!
//! `expect` borrows an agent to *reason about and act on* the system under test
//! (`ideas/expect.md` §"Delegation over ACP"), while the verdict stays
//! deterministic and inside `expect`. This module owns the one piece of
//! choreography that needs a live connection — standing an
//! [`AgentPool`](swissarmyhammer_validators::AgentPool) up over an ACP agent and
//! driving one scoped subagent per expectation goal — and is the mirror of
//! `swissarmyhammer-validators`' `review::drive::run_review_over_agent`. The
//! review machinery is reused, not re-derived: the same [`AgentPool`], the same
//! `TolerantResponseRouter`, and the same tolerant
//! [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
//! structured-output extractor.
//!
//! [`run_expect_over_agent`] takes the two halves of an ACP agent handle (the
//! [`DynConnectTo<Client>`] component and a `broadcast::Receiver` of the agent's
//! streamed `session/update` notifications), so this crate constructs no agent
//! itself — the tool layer injects a ready handle behind its pipeline gate. The
//! engine therefore stays agent-construction-free.
//!
//! # Single notification path
//!
//! The pool's per-prompt collectors are fed from exactly ONE source: the agent's
//! own `notification_rx` broadcast, drained by [`forward_notifications`] into the
//! pool's [`NotificationSender`](claude_agent::NotificationSender). For a real
//! `swissarmyhammer_agent::AcpAgentHandle`, `notification_rx` is a `subscribe()`
//! of the backend's broadcast channel that the handle ALSO bridges onto the
//! connection. Because that bridge re-emits the very same notifications onto the
//! connection, the driver must NOT also forward what the connection re-emits —
//! doing so delivers every streamed chunk twice and
//! [`collect_response_content`](claude_agent::collect_response_content) would
//! concatenate the reply twice, corrupting the JSON the structured-output parser
//! reads. Forwarding solely from `notification_rx` keeps delivery single-path.
//!
//! # Tamper-resistance
//!
//! The driving agent may read repo files (confined under the repo root) and is
//! auto-granted permission, but it MUST NOT edit the ledger it is being graded
//! against. [`answer_agent_request`] therefore DENIES any `fs/write_text_file`
//! that resolves under the repo's `.expect/` directory — specs, goldens, and
//! received fixtures are off-limits — while acking writes elsewhere.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::schema::{
    AgentRequest, ClientCapabilities, FileSystemCapabilities, InitializeRequest,
    PermissionOptionId, ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, WriteTextFileResponse,
};
use agent_client_protocol::{Client, ConnectionTo, DynConnectTo, Responder};
use agent_client_protocol_extras::TolerantResponseRouter;
use tokio::sync::broadcast;

use swissarmyhammer_validators::review::extract_json_value;
use swissarmyhammer_validators::{AgentPool, PoolConfig};

use crate::config::EXPECT_DIR;
use crate::error::ExpectError;

/// The set of goals to drive, one scoped subagent per goal.
///
/// This is the resolved-scope input to [`run_expect_over_agent`]: each goal is
/// the prompt that drives one expectation's subagent (open a scoped session,
/// send the goal, drain `session/update`, capture the forced structured output).
/// The richer scope-resolution that maps expectation specs onto these goals
/// lands with the observe-over-agent pipeline; this seam takes the goals it is
/// handed.
#[derive(Debug, Clone, Default)]
pub struct ExpectScope {
    /// The per-expectation goals to drive, in order.
    pub goals: Vec<String>,
}

/// The structured capture from one driven expectation subagent.
///
/// ACP's prompt turn returns only a control signal (`stopReason`), not a
/// payload, so the structured result is assembled here from the subagent's
/// reply: [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
/// strips any fences and the JSON object is parsed into [`Self::structured`].
#[derive(Debug, Clone)]
pub struct DrivenObservation {
    /// The goal the subagent was driven with (its identity in the scope).
    pub goal: String,
    /// The tolerant-extracted structured JSON the subagent produced.
    pub structured: serde_json::Value,
}

/// Drive every goal in `scope` against a live ACP agent and return each
/// subagent's structured capture.
///
/// This is the engine entry point the MCP `expect` tool calls. It owns the
/// agent-pool choreography:
///
/// 1. Drain the agent's `notification_rx` broadcast into a fresh
///    [`NotificationSender`](claude_agent::NotificationSender) the pool's workers
///    subscribe to — the single source of streamed `session/update` content (see
///    the module docs on why the connection re-emission is NOT also forwarded).
/// 2. Stand up `Client.builder().connect_with(agent, ...)` to obtain a typed
///    [`ConnectionTo<Agent>`] and build the shared [`AgentPool`] over it, sized
///    by `pool_config`.
/// 3. Run [`InitializeRequest`] ONCE per connection, then submit one prompt per
///    goal and collect each structured reply.
///
/// `agent` and `notification_rx` are the two halves of an ACP agent handle,
/// supplied by the tool so this crate stays free of any agent-construction
/// dependency. `repo_root` is resolved by the caller from the MCP session
/// work-dir (never `current_dir()`); the agent's `fs/read_text_file` reads are
/// confined under it and `fs/write_text_file` under its `.expect/` ledger is
/// refused.
///
/// # Errors
///
/// Returns [`ExpectError::Agent`] when the ACP connection fails to stand up, the
/// pool drops a turn, a driven prompt fails, or a subagent's reply is not JSON.
pub async fn run_expect_over_agent(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    scope: ExpectScope,
    repo_root: &Path,
    pool_config: PoolConfig,
) -> Result<Vec<DrivenObservation>, ExpectError> {
    // A fresh notifier whose broadcast the pool's workers subscribe to, fed by a
    // single forwarding task draining the agent's `notification_rx`. This is the
    // ONLY feed into the notifier (see the module docs on double-feeding).
    let (notifier, forward_task) = build_pool_notifier(notification_rx);

    // The repo root the agent's `fs` requests are resolved under. Owned so the
    // `'static` request handler can keep it for the connection's life.
    let repo_root: Arc<PathBuf> = Arc::new(repo_root.to_path_buf());

    let connect_result = Client
        .builder()
        .name("swissarmyhammer-expect")
        // An abandoned turn (the pool's per-turn liveness dropped its
        // `block_task` receiver) must fail that turn only: route the agent's
        // late response into the void instead of killing the dispatch loop and
        // the whole run with it.
        .with_handler(TolerantResponseRouter)
        .on_receive_request(
            {
                let repo_root = Arc::clone(&repo_root);
                move |req: AgentRequest,
                      responder: Responder<serde_json::Value>,
                      cx: ConnectionTo<agent_client_protocol::Agent>| {
                    let repo_root = Arc::clone(&repo_root);
                    async move {
                        answer_agent_request(req, responder, &cx, &repo_root);
                        Ok(())
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, {
            let notifier = Arc::clone(&notifier);
            move |cx: ConnectionTo<agent_client_protocol::Agent>| {
                run_pipeline_in_connection(cx, notifier, pool_config, scope)
            }
        })
        .await;

    forward_task.abort();

    match connect_result {
        Ok(observations) => observations,
        Err(e) => Err(ExpectError::Agent(format!(
            "expect agent connection failed: {e}"
        ))),
    }
}

/// Buffer size for the pool's notification broadcast channel.
const NOTIFY_BUFFER: usize = 256;

/// Build the pool's notifier and spawn the single task that feeds it from the
/// agent's `notification_rx` broadcast.
///
/// This is the engine's one and only notification path: the per-prompt
/// collectors subscribe to the returned
/// [`NotificationSender`](claude_agent::NotificationSender), and exactly one
/// [`forward_notifications`] task copies each incoming agent notification into
/// it. The caller aborts the returned [`JoinHandle`](tokio::task::JoinHandle)
/// once the pipeline is done. Keeping this the sole feed is what guarantees a
/// real handle's reply is collected once rather than twice (see the module docs).
fn build_pool_notifier(
    notification_rx: broadcast::Receiver<SessionNotification>,
) -> (
    Arc<claude_agent::NotificationSender>,
    tokio::task::JoinHandle<()>,
) {
    let (notifier, _seed_rx) = claude_agent::NotificationSender::new(NOTIFY_BUFFER);
    let notifier = Arc::new(notifier);
    let forward_task = tokio::spawn(forward_notifications(
        notification_rx,
        Arc::clone(&notifier),
    ));
    (notifier, forward_task)
}

/// Copy every notification from the agent's stream into the pool's notifier
/// until the source channel closes.
async fn forward_notifications(
    mut rx: broadcast::Receiver<SessionNotification>,
    notifier: Arc<claude_agent::NotificationSender>,
) {
    loop {
        match rx.recv().await {
            Ok(notif) => {
                let _ = notifier.send_update(notif).await;
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Answer a request the agent sends back to the `expect` client mid-prompt.
///
/// A real agent, during a prompt turn, issues nested agent→client requests and
/// blocks on their responses before the turn can finish. The client MUST answer
/// them or the prompt deadlocks and the whole run hangs.
///
/// Each variant is handled and a response is ALWAYS sent:
///
/// - `session/request_permission` → auto-approve (`Selected("allow")`). The run
///   is unattended; there is no human to prompt for tool consent.
/// - `fs/read_text_file` → read the file from disk **confined under `repo_root`**
///   (honoring the optional 1-based `line` and `limit`) and return its content.
/// - `fs/write_text_file` → **DENY** any write that resolves under the repo's
///   `.expect/` ledger (tamper-resistance: the driving agent must not edit the
///   specs/goldens/fixtures it is graded against); ack any other write WITHOUT
///   writing (the system under test is driven via its surface adapter, not ACP
///   fs, so an ack keeps the agent from hanging without mutating the repo).
/// - anything else → method-not-found error.
///
/// The work is dispatched via [`ConnectionTo::spawn`] so it runs OFF the
/// connection's single dispatch loop, keeping that loop free to route responses.
fn answer_agent_request(
    request: AgentRequest,
    responder: Responder<serde_json::Value>,
    cx: &ConnectionTo<agent_client_protocol::Agent>,
    repo_root: &Arc<PathBuf>,
) {
    let repo_root = Arc::clone(repo_root);
    let _ = cx.clone().spawn(async move {
        match request {
            AgentRequest::RequestPermissionRequest(_req) => {
                let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    PermissionOptionId::new("allow"),
                ));
                responder
                    .cast()
                    .respond_with_result(Ok(RequestPermissionResponse::new(outcome)))
            }
            AgentRequest::ReadTextFileRequest(req) => {
                let result = read_text_file_under_repo(&repo_root, &req)
                    .map(ReadTextFileResponse::new)
                    .map_err(|e| agent_client_protocol::Error::invalid_params().data(e));
                responder.cast().respond_with_result(result)
            }
            AgentRequest::WriteTextFileRequest(req) => {
                // Refuse a ledger write; ack any other write without touching disk.
                let result = refuse_ledger_write(&repo_root, &req.path)
                    .map(|()| WriteTextFileResponse::new())
                    .map_err(|reason| {
                        tracing::warn!("expect client denied a ledger write: {reason}");
                        agent_client_protocol::Error::invalid_params().data(reason)
                    });
                responder.cast().respond_with_result(result)
            }
            other => {
                tracing::warn!(
                    "expect client received unsupported agent request: {}",
                    other.method()
                );
                responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found())
            }
        }
    });
}

/// Read a text file the agent requested, **confined under `repo_root`**, honoring
/// the optional 1-based `line` start and `limit` line count.
///
/// The agent names the path, so it is untrusted: an absolute path could point
/// anywhere and a relative path can carry `..` segments. The read is confined —
/// the canonicalized target must live inside the canonicalized `repo_root` or the
/// request is refused. The boundary is location, not shape: an absolute path that
/// genuinely resolves under `repo_root` is still honored.
///
/// Returns the (possibly sliced) content, or an error string when the file
/// cannot be read or resolves outside the repository (mapped to `invalid_params`
/// by the caller).
fn read_text_file_under_repo(
    repo_root: &Path,
    req: &agent_client_protocol::schema::ReadTextFileRequest,
) -> Result<String, String> {
    let path = confine_under_repo(repo_root, &req.path)?;

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    // No slice requested: return the whole file.
    if req.line.is_none() && req.limit.is_none() {
        return Ok(content);
    }

    let lines: Vec<&str> = content.lines().collect();
    let start = req.line.map(|l| (l.max(1) - 1) as usize).unwrap_or(0);
    let end = req
        .limit
        .map(|l| start + l as usize)
        .unwrap_or(lines.len())
        .min(lines.len());

    if start >= lines.len() {
        return Ok(String::new());
    }
    Ok(lines[start..end].join("\n"))
}

/// Resolve an agent-requested **read** path to a concrete on-disk path confined
/// under `repo_root`, rejecting any target that escapes the repository.
///
/// A relative path is joined onto `repo_root`; an absolute path is taken as-is.
/// Both the candidate and `repo_root` are canonicalized (resolving `..` and
/// symlinks) and the canonical candidate must `starts_with` the canonical repo
/// root — so a `..`-escape or out-of-repo absolute path is refused even when the
/// target exists.
///
/// Returns an error string (mapped to `invalid_params` by the caller) when the
/// repo root or the target cannot be canonicalized, or when the target lies
/// outside the repository.
fn confine_under_repo(repo_root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let canonical_root = repo_root
        .canonicalize()
        .map_err(|e| format!("failed to resolve repo root {}: {e}", repo_root.display()))?;

    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        canonical_root.join(requested)
    };

    let canonical = candidate.canonicalize().map_err(|e| {
        format!(
            "failed to resolve requested path {}: {e}",
            candidate.display()
        )
    })?;

    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "requested path {} is outside the repository {}",
            canonical.display(),
            canonical_root.display()
        ));
    }

    Ok(canonical)
}

/// Refuse a **write** that resolves under the repo's `.expect/` ledger.
///
/// The tamper-resistance guard: the driving agent may act on the system under
/// test but must never edit the ledger it is graded against (specs, goldens,
/// received fixtures all live under [`EXPECT_DIR`]). A write target's final
/// components may not exist yet, so — unlike [`confine_under_repo`], which can
/// canonicalize an existing read target whole — this resolves the path's longest
/// *existing* ancestor through the filesystem (folding away symlinks, the
/// `/var`→`/private/var` class of escape) and only normalizes the non-existent
/// tail lexically, then checks the result against the canonical ledger prefix.
///
/// Returns `Err` with a refusal message (mapped to `invalid_params` by the
/// caller) when the resolved target lies under `<repo_root>/.expect/`, and
/// `Ok(())` for every write outside the ledger.
fn refuse_ledger_write(repo_root: &Path, requested: &Path) -> Result<(), String> {
    let canonical_root = repo_root
        .canonicalize()
        .map_err(|e| format!("failed to resolve repo root {}: {e}", repo_root.display()))?;
    let ledger = canonical_root.join(EXPECT_DIR);

    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        canonical_root.join(requested)
    };
    let resolved = resolve_existing_prefix(&candidate);

    if resolved.starts_with(&ledger) {
        return Err(format!(
            "refusing to write {} under the {}/ ledger: the driving agent must not edit \
             specs, goldens, or received fixtures",
            resolved.display(),
            EXPECT_DIR
        ));
    }
    Ok(())
}

/// Resolve `path` to a concrete location by canonicalizing its longest existing
/// ancestor (resolving symlinks and `..` in the part that is on disk) and
/// re-appending the lexically-normalized remainder.
///
/// A write target's final components typically do not exist yet, so a whole-path
/// [`Path::canonicalize`] would fail. Canonicalizing the existing prefix instead
/// closes the symlinked-prefix escape (e.g. macOS's `/var` → `/private/var`,
/// where a purely lexical check would not match a `/private/var`-rooted ledger),
/// while [`normalize_lexically`] folds any `..` in the not-yet-existing tail.
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(canonical) = ancestor.canonicalize() {
            // `ancestor` is always a prefix of `path`, so this strip cannot fail.
            let tail = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return normalize_lexically(&canonical.join(tail));
        }
    }
    normalize_lexically(path)
}

/// Normalize a path lexically: resolve `.` and `..` components by string
/// manipulation, without consulting the filesystem (so it works on the
/// not-yet-existing tail of a write target).
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Build the pool inside the live connection and drive the scope to a result.
///
/// Split out so the `connect_with` closure body has a single typed future to
/// return. The pool is dropped at the end of this scope, winding its workers down
/// before the connection tears down.
async fn run_pipeline_in_connection(
    cx: ConnectionTo<agent_client_protocol::Agent>,
    notifier: Arc<claude_agent::NotificationSender>,
    pool_config: PoolConfig,
    scope: ExpectScope,
) -> agent_client_protocol::Result<Result<Vec<DrivenObservation>, ExpectError>> {
    // ACP `initialize` is a ONCE-per-connection handshake. Do it here, before the
    // pool's workers issue any prompts, rather than per prompt: the pool shares
    // this single connection across N workers, so initializing per prompt would
    // race N concurrent handshakes at the one agent process and wedge it.
    //
    // Advertise the filesystem capabilities the request handler backs: both
    // `fs/read_text_file` (served from disk under the repo root) and
    // `fs/write_text_file` (handled — ledger writes refused, others acked) so the
    // agent's capability view matches `answer_agent_request`.
    cx.send_request(
        InitializeRequest::new(1.into()).client_capabilities(
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(true)),
        ),
    )
    .block_task()
    .await?;

    let pool = AgentPool::new(cx, notifier, pool_config);
    Ok(drive_scope(&pool, scope).await)
}

/// Submit one prompt per goal, then collect each subagent's structured reply in
/// submission order.
///
/// Submission is non-blocking, so all goals are queued first (pipelining across
/// the pool's workers) and then awaited. Each reply is parsed through the shared
/// tolerant [`extract_json_value`](swissarmyhammer_validators::review::extract_json_value)
/// extractor.
async fn drive_scope(
    pool: &AgentPool,
    scope: ExpectScope,
) -> Result<Vec<DrivenObservation>, ExpectError> {
    let pending: Vec<(String, _)> = scope
        .goals
        .into_iter()
        .map(|goal| {
            let rx = pool.submit(goal.clone());
            (goal, rx)
        })
        .collect();

    let mut observations = Vec::with_capacity(pending.len());
    for (goal, rx) in pending {
        let collected = rx
            .await
            .map_err(|e| {
                ExpectError::Agent(format!("the agent pool dropped the turn for `{goal}`: {e}"))
            })?
            .map_err(|e| ExpectError::Agent(format!("driving `{goal}` failed: {e}")))?;

        let json = extract_json_value(&collected.content, '{', '}');
        let structured: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            ExpectError::Agent(format!(
                "the subagent's reply for `{goal}` was not structured JSON: {e}"
            ))
        })?;
        observations.push(DrivenObservation { goal, structured });
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use acp_conformance::test_utils::{numbered_session_response, MockAgent, MockAgentAdapter};
    use agent_client_protocol::schema::{
        ContentBlock, ContentChunk, NewSessionRequest, NewSessionResponse, PromptRequest,
        PromptResponse, SessionId, SessionUpdate, StopReason, TextContent,
    };
    use futures::future::BoxFuture;
    use tempfile::TempDir;

    /// How long a wedged pipeline may run before the test fails instead of
    /// hanging CI.
    const PIPELINE_TIMEOUT: Duration = Duration::from_secs(30);

    /// Capacity of the scripted backend's broadcast channel — the channel the
    /// driver's `notification_rx` subscribes to. It comfortably exceeds any
    /// test's notification volume so a slow subscriber never lags chunks away.
    const BACKEND_BROADCAST_CAPACITY: usize = 64;

    /// Capacity for the single-stream invariant test, whose channels must hold
    /// EVERY chunk sent before any collector subscribes and drains.
    const PRELOADED_STREAM_CAPACITY: usize = 256;

    /// A representative structured reply: the JSON object shape a driven subagent
    /// emits and [`drive_scope`] captures via [`extract_json_value`].
    const STRUCTURED_REPLY: &str = r#"{"path": "src/checkout/coupon", "verdict": "pass"}"#;

    // ---- a temp repo fixture --------------------------------------------

    /// The repo-relative path of the file [`temp_repo`] plants, and a substring
    /// of its content the read assertions check.
    const FIXTURE_FILE: &str = "src/lib.rs";
    const FIXTURE_NEEDLE: &str = "pub fn compute";

    /// A throwaway repo with one readable source file under `src/`.
    fn temp_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join(FIXTURE_FILE),
            "pub fn compute() -> u32 { 42 }\n",
        )
        .unwrap();
        dir
    }

    // ---- single-path notification invariant (the double-feed guard) -----

    /// Split `text` into `parts` roughly equal chunks, one `AgentMessageChunk`
    /// notification per chunk. Streaming the reply across several chunks (as a
    /// real backend does) is what makes double-delivery corrupt: a duplicated,
    /// interleaved chunk stream cannot be reassembled back into the original JSON.
    fn chunked_notifications(
        session: &SessionId,
        text: &str,
        parts: usize,
    ) -> Vec<SessionNotification> {
        let bytes = text.as_bytes();
        let step = bytes.len().div_ceil(parts).max(1);
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < bytes.len() {
            let mut end = (start + step).min(bytes.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            let piece = &text[start..end];
            let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(piece.to_string()),
            )));
            chunks.push(SessionNotification::new(session.clone(), update));
            start = end;
        }
        chunks
    }

    /// Collect a multi-chunk streamed reply through the pool's notifier, exactly
    /// as a pool worker does: subscribe to the notifier's broadcast, reassemble
    /// the streamed text for `session`, and return the collected string.
    async fn collect_through_notifier(
        notifier: &Arc<claude_agent::NotificationSender>,
        session: SessionId,
    ) -> String {
        let (collector, collected_text, notification_count, _matched) =
            claude_agent::spawn_notification_collector(notifier.sender().subscribe(), session);
        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        claude_agent::collect_response_content(
            collector,
            collected_text,
            notification_count,
            &prompt_response,
        )
        .await
    }

    /// The driver feeds the pool's collectors from EXACTLY ONE source: the
    /// agent's `notification_rx`, drained by the single [`forward_notifications`]
    /// task [`build_pool_notifier`] spawns. This pins both halves of the
    /// invariant deterministically:
    ///
    /// 1. The single-feed seam reassembles the streamed reply EXACTLY once
    ///    (byte-for-byte equal to the original).
    /// 2. A second feed of the same stream — the dual-path bug — doubles every
    ///    chunk, so the collected text is twice as long and no longer the
    ///    original. The doubling holds for every interleaving, so the
    ///    discriminating assertion is not flaky.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn notification_rx_is_the_pools_single_collected_stream() {
        let session = SessionId::new("sess-single".to_string());
        let reply = STRUCTURED_REPLY.to_string();
        let stream = chunked_notifications(&session, &reply, 6);

        // --- (1) the driver's actual single-feed path collects the reply once ---
        let (notify_tx, notification_rx) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let (single_notifier, single_forward) = build_pool_notifier(notification_rx);
        for notif in &stream {
            let _ = notify_tx.send(notif.clone());
        }
        let collected_single = collect_through_notifier(&single_notifier, session.clone()).await;
        single_forward.abort();

        assert_eq!(
            collected_single, reply,
            "the driver's single feed must reassemble the agent reply exactly once"
        );

        // --- (2) the dual-feed shape doubles the same stream -------------------
        let (dual_tx, dual_rx_a) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let dual_rx_b = dual_tx.subscribe();
        let (dual_notifier, _seed) = claude_agent::NotificationSender::new(NOTIFY_BUFFER);
        let dual_notifier = Arc::new(dual_notifier);
        let fwd_a = tokio::spawn(forward_notifications(dual_rx_a, Arc::clone(&dual_notifier)));
        let fwd_b = tokio::spawn(forward_notifications(dual_rx_b, Arc::clone(&dual_notifier)));
        for notif in &stream {
            let _ = dual_tx.send(notif.clone());
        }
        let collected_dual = collect_through_notifier(&dual_notifier, session).await;
        fwd_a.abort();
        fwd_b.abort();

        assert_ne!(
            collected_dual, reply,
            "a dual feed must NOT reassemble the original reply — this is the bug the \
             single-path driver fixes"
        );
        assert_eq!(
            collected_dual.len(),
            reply.len() * 2,
            "a dual feed doubles every chunk, doubling the collected length and corrupting \
             the JSON; the single-feed driver avoids this"
        );
    }

    // ---- read confinement (read_text_file_under_repo) -------------------

    /// Build a `fs/read_text_file` request for `path` (relative or absolute).
    fn read_request(
        path: impl Into<PathBuf>,
    ) -> agent_client_protocol::schema::ReadTextFileRequest {
        agent_client_protocol::schema::ReadTextFileRequest::new(
            SessionId::new("sess-read".to_string()),
            path.into(),
        )
    }

    #[test]
    fn read_text_file_under_repo_serves_an_in_repo_relative_path() {
        let repo = temp_repo();
        let content = read_text_file_under_repo(repo.path(), &read_request(FIXTURE_FILE))
            .expect("an in-repo relative read must succeed");
        assert!(
            content.contains(FIXTURE_NEEDLE),
            "the in-repo read must return the real file content, got: {content}"
        );
    }

    #[test]
    fn read_text_file_under_repo_rejects_a_dotdot_escape() {
        let repo = temp_repo();
        // Plant a readable target in the repo's PARENT so the read must still be
        // refused on location, not on absence.
        let parent = repo
            .path()
            .parent()
            .expect("temp repo has a parent dir")
            .to_path_buf();
        std::fs::write(parent.join("secret.txt"), "top secret").unwrap();

        let err = read_text_file_under_repo(repo.path(), &read_request("../secret.txt"))
            .expect_err("a ..-escape must be rejected");
        assert!(
            err.contains("outside the repository"),
            "the rejection must name the confinement boundary, got: {err}"
        );
    }

    #[test]
    fn read_text_file_under_repo_rejects_an_absolute_outside_path() {
        let repo = temp_repo();
        let err = read_text_file_under_repo(repo.path(), &read_request("/etc/passwd"))
            .expect_err("an absolute outside path must be rejected");
        assert!(
            err.contains("outside the repository"),
            "the rejection must name the confinement boundary, got: {err}"
        );
    }

    #[test]
    fn read_text_file_under_repo_serves_an_absolute_in_repo_path() {
        let repo = temp_repo();
        let abs = repo.path().join(FIXTURE_FILE);
        let content = read_text_file_under_repo(repo.path(), &read_request(abs))
            .expect("an absolute in-repo read must succeed");
        assert!(
            content.contains(FIXTURE_NEEDLE),
            "an absolute in-repo read must return the real content, got: {content}"
        );
    }

    // ---- write tamper-resistance (refuse_ledger_write) ------------------

    #[test]
    fn refuse_ledger_write_denies_a_relative_write_under_the_ledger() {
        let repo = temp_repo();
        let err = refuse_ledger_write(repo.path(), Path::new(".expect/goldens/coupon.golden.json"))
            .expect_err("a relative write under .expect/ must be refused");
        assert!(
            err.contains(EXPECT_DIR),
            "the refusal must name the ledger boundary, got: {err}"
        );
    }

    #[test]
    fn refuse_ledger_write_denies_an_absolute_write_under_the_ledger() {
        let repo = temp_repo();
        let abs = repo.path().join(".expect").join("received").join("x.json");
        let err = refuse_ledger_write(repo.path(), &abs)
            .expect_err("an absolute write under .expect/ must be refused");
        assert!(err.contains(EXPECT_DIR), "got: {err}");
    }

    #[test]
    fn refuse_ledger_write_denies_a_dotdot_climb_back_into_the_ledger() {
        let repo = temp_repo();
        // A path dressed up to climb out of `src/` and back into the ledger must
        // still be refused after lexical normalization.
        let err = refuse_ledger_write(repo.path(), Path::new("src/../.expect/config.toml"))
            .expect_err("a ..-dressed ledger write must be refused");
        assert!(err.contains(EXPECT_DIR), "got: {err}");
    }

    #[test]
    fn refuse_ledger_write_allows_a_write_outside_the_ledger() {
        let repo = temp_repo();
        refuse_ledger_write(repo.path(), Path::new("src/generated_output.txt"))
            .expect("a write outside the .expect/ ledger must be allowed");
    }

    // ---- end-to-end over a stub ACP agent -------------------------------

    /// A minimal stub ACP agent: every prompt streams `reply` onto the backend
    /// broadcast (the channel the driver's `notification_rx` subscribes to) under
    /// the prompt's own session id, then ends the turn — the real-handle shape
    /// `run_expect_over_agent` collects from.
    struct EchoAgent {
        next_session: AtomicUsize,
        notify_tx: broadcast::Sender<SessionNotification>,
        reply: String,
    }

    impl MockAgent for EchoAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            numbered_session_response(&self.next_session, "sess")
        }

        fn prompt<'a>(
            &'a self,
            request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move {
                let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                    ContentBlock::Text(TextContent::new(self.reply.clone())),
                ));
                let _ = self
                    .notify_tx
                    .send(SessionNotification::new(request.session_id.clone(), update));
                Ok(PromptResponse::new(StopReason::EndTurn))
            })
        }
    }

    /// `run_expect_over_agent` connects over ACP, initializes once, drives the
    /// scope's goal over the pool, and captures the subagent's structured reply —
    /// the full seam end to end against a stub agent.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_expect_over_agent_drives_a_goal_over_a_stub_agent() {
        let repo = temp_repo();
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = Arc::new(EchoAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            reply: STRUCTURED_REPLY.to_string(),
        });
        let dyn_agent = DynConnectTo::new(MockAgentAdapter(agent));

        const GOAL: &str = "observe src/checkout/coupon";
        let scope = ExpectScope {
            goals: vec![GOAL.to_string()],
        };

        let observations = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_expect_over_agent(
                dyn_agent,
                notification_rx,
                scope,
                repo.path(),
                PoolConfig::remote(1),
            ),
        )
        .await
        .expect("the run must not hang")
        .expect("the pipeline must produce observations");

        assert_eq!(observations.len(), 1, "exactly one goal was driven");
        assert_eq!(observations[0].goal, GOAL, "the goal identity is preserved");
        assert_eq!(
            observations[0].structured["verdict"], "pass",
            "the subagent's structured reply is captured: {:?}",
            observations[0].structured
        );
    }
}
