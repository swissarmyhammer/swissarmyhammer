//! Engine driver — wire a live ACP agent into the review pipeline.
//!
//! [`run_review`](crate::review::run_review) is the pure pipeline barrier: it
//! takes an already-built [`AgentPool`](crate::validators::AgentPool) plus the
//! resolved scope, loader, index connection and embedder. This module owns the
//! one piece of choreography the pure barrier deliberately leaves out — standing
//! the [`AgentPool`] up over a live ACP agent — so the MCP tool stays a thin
//! dispatch shim that supplies the agent and the scope and gets a
//! [`ReviewReport`] back.
//!
//! [`run_review_over_agent`] takes the two halves of an ACP agent handle (the
//! [`DynConnectTo<Client>`] component and a `broadcast::Receiver` of the agent's
//! streamed `session/update` notifications), builds the
//! `Client.builder().connect_with(...)` connection that yields a typed
//! [`ConnectionTo<Agent>`], constructs the shared [`AgentPool`] over it (sized by
//! the caller's [`PoolConfig`]), and runs [`run_review`](crate::review::run_review)
//! inside the connection. The pool — and therefore every agent task — lives only
//! for the duration of the pipeline; the connection tears down when the report is
//! ready.
//!
//! # Single notification path
//!
//! The pool's per-prompt collectors are fed from exactly ONE source: the agent's
//! own `notification_rx` broadcast, drained by [`forward_notifications`] into the
//! pool's [`NotificationSender`](claude_agent::NotificationSender). That is the
//! authoritative stream a real handle exposes — for a
//! `swissarmyhammer_agent::AcpAgentHandle`, `notification_rx` is a `resubscribe()`
//! of the backend's broadcast channel, the same channel
//! `wrap_claude_into_handle` bridges onto the connection via
//! `forward_session_notifications`. Because that bridge re-emits the very same
//! notifications onto the connection, the driver must NOT also forward what the
//! connection re-emits — doing so delivers every streamed chunk twice and
//! [`collect_response_content`](claude_agent::collect_response_content) would
//! concatenate the agent's reply twice, corrupting the JSON the fleet/verify
//! parser reads. Forwarding solely from `notification_rx` keeps delivery
//! single-path for both the real handle and a scripted agent.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::schema::{
    AgentRequest, ClientCapabilities, FileSystemCapabilities, InitializeRequest,
    PermissionOptionId, ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, WriteTextFileResponse,
};
use agent_client_protocol::{Client, ConnectionTo, DynConnectTo, Responder};
use agent_client_protocol_extras::TolerantResponseRouter;
use model_embedding::TextEmbedder;
use rusqlite::Connection;
use tokio::sync::broadcast;

use crate::error::AvpError;
use crate::review::fleet::{FleetConfig, ReviewProgressSender};
use crate::review::scope::Scope;
use crate::review::synthesize::{run_review, ReviewReport};
use crate::validators::{AgentPool, PoolConfig, ValidatorLoader};

/// Run the full review pipeline against a live ACP agent and synthesize the
/// report.
///
/// This is the engine entry point the MCP `review` tool calls. It owns the
/// agent-pool choreography the pure [`run_review`](crate::review::run_review)
/// barrier leaves to its caller:
///
/// 1. Drain the agent's `notification_rx` broadcast into a fresh
///    [`NotificationSender`](claude_agent::NotificationSender) the pool's
///    workers subscribe to — the single source of streamed `session/update`
///    content (see the module docs on why the connection re-emission is NOT
///    also forwarded).
/// 2. Stand up `Client.builder().connect_with(agent, ...)` to obtain a typed
///    [`ConnectionTo<Agent>`] and build the shared [`AgentPool`] over it, sized
///    by `pool_config` (the backend + `review.concurrency` policy).
/// 3. Call [`run_review`](crate::review::run_review) — scope → fan-out → guard →
///    verify → drain → synthesize — and return its [`ReviewReport`].
///
/// `agent` and `notification_rx` are the two halves of an ACP agent handle (e.g.
/// `swissarmyhammer_agent::AcpAgentHandle`'s `agent` + `notification_rx`),
/// supplied by the tool so this crate stays free of any agent-construction
/// dependency. `repo_path`, `loader`, `conn`, and `embedder` are resolved by the
/// caller from the MCP session/work-dir (never `current_dir()`); `now` is the
/// caller-formatted local timestamp rendered verbatim into the report header.
///
/// `progress` is the optional [`ReviewProgressSender`] the engine emits
/// [`ReviewProgressEvent`](crate::review::ReviewProgressEvent)s on — one
/// `Planned` per batch and a `PairStarted`/`PairDone` per (validator, file)
/// pair — so a caller (e.g. the MCP tool bridging to `notifications/progress`)
/// can report live progress. Pass `Some` when the caller wants those events;
/// `None` (the default for callers without a progress channel) emits nothing
/// and leaves the run's behavior byte-identical to the pre-progress path.
///
/// # Errors
///
/// Returns the [`AvpError`] from [`run_review`](crate::review::run_review) on a
/// scope/index failure, or [`AvpError::AgentConnection`] when the ACP
/// connection itself fails to stand up.
#[allow(clippy::too_many_arguments)]
pub async fn run_review_over_agent(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    scope: Scope,
    repo_path: &Path,
    loader: &ValidatorLoader,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
    pool_config: PoolConfig,
    fleet_config: FleetConfig,
    progress: Option<ReviewProgressSender>,
    now: &str,
) -> Result<ReviewReport, AvpError> {
    // A fresh notifier whose broadcast the pool's workers subscribe to, fed by a
    // single forwarding task draining the agent's `notification_rx`. This is the
    // ONLY feed into the notifier: the connection's `session/update` re-emission
    // is deliberately NOT forwarded as well, because for a real handle it carries
    // the very same notifications and double-feeding would concatenate every reply
    // twice (see the module docs).
    let (notifier, forward_task) = build_pool_notifier(notification_rx);

    // The repo root the agent's `fs/read_text_file` requests are resolved under.
    // Owned so the `'static` request handler can keep it for the connection's life.
    let repo_root: Arc<PathBuf> = Arc::new(repo_path.to_path_buf());

    let connect_result = Client
        .builder()
        .name("swissarmyhammer-review")
        // An abandoned turn (the pool's per-turn liveness dropped its
        // `block_task` receiver) must fail that turn only: route the agent's
        // late response into the void instead of letting "receiver dropped"
        // kill the dispatch loop and the whole review with it.
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
                run_pipeline_in_connection(
                    cx,
                    notifier,
                    pool_config,
                    scope,
                    repo_path,
                    loader,
                    conn,
                    embedder,
                    fleet_config,
                    progress,
                    now,
                )
            }
        })
        .await;

    forward_task.abort();

    match connect_result {
        Ok(report) => report,
        Err(e) => Err(AvpError::AgentConnection(e)),
    }
}

/// Buffer size for the pool's notification broadcast channel.
const NOTIFY_BUFFER: usize = 256;

/// Build the pool's notifier and spawn the single task that feeds it from the
/// agent's `notification_rx` broadcast.
///
/// This is the engine's one and only notification path: the per-prompt collectors
/// subscribe to the returned [`NotificationSender`](claude_agent::NotificationSender),
/// and exactly one [`forward_notifications`] task copies each incoming agent
/// notification into it. The caller aborts the returned [`JoinHandle`] once the
/// pipeline is done. Keeping this the sole feed is what guarantees a real handle's
/// reply is collected once rather than twice — see the module docs.
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

/// Answer a request the agent sends back to the review client mid-prompt.
///
/// A real `claude` agent, during a prompt turn, issues nested agent→client
/// requests and `block_task().await`s their responses before the turn can
/// finish. The review's client MUST answer them or the prompt deadlocks — the
/// pool never drains and the whole review hangs (the production symptom).
///
/// Each variant is handled and a response is ALWAYS sent — no agent request is
/// ever left unanswered:
///
/// - `session/request_permission` → auto-approve (`Selected("allow")`). The
///   review runs unattended; there is no human to prompt for tool consent.
/// - `fs/read_text_file` → read the file from disk under `repo_path` (honoring
///   the optional 1-based `line` and `limit`) and return its content.
/// - `fs/write_text_file` → respond success WITHOUT writing. A review is
///   read-only; the agent gets a clean ack rather than a hang or a repo mutation.
/// - anything else (terminals, etc.) → method-not-found error.
///
/// The work is dispatched via [`ConnectionTo::spawn`] so it runs OFF the
/// connection's single dispatch loop, keeping that loop free to route responses
/// (the same agent↔client deadlock discipline as
/// `swissarmyhammer_agent::dispatch_claude_request`). `read_text_file` touches
/// the disk, so spawning it off the loop also avoids blocking dispatch on IO.
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
            AgentRequest::WriteTextFileRequest(_req) => {
                // A review is read-only: ack success without touching the repo.
                responder
                    .cast()
                    .respond_with_result(Ok(WriteTextFileResponse::new()))
            }
            other => {
                tracing::warn!(
                    "review client received unsupported agent request: {}",
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
/// The agent names the path in its `fs/read_text_file` request, so the path is
/// untrusted: an absolute path could point anywhere, and a relative path can carry
/// `..` segments that climb out of the repo. The read is therefore confined — the
/// resolved, canonicalized target must live inside the canonicalized `repo_root`,
/// or the request is refused. A relative path is joined onto `repo_root`; an
/// absolute path is taken verbatim; either way the canonical result must be inside
/// the repo (so an absolute path that genuinely resolves under `repo_root` is
/// still honored — the boundary is location, not shape).
///
/// Returns the (possibly sliced) file content, or an error string when the file
/// cannot be read or resolves outside the repository (the caller maps this to an
/// `invalid_params` response).
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

/// Resolve an agent-requested path to a concrete on-disk path confined under
/// `repo_root`, rejecting any target that escapes the repository.
///
/// A relative path is joined onto `repo_root`; an absolute path is taken as-is.
/// Both the candidate and `repo_root` are canonicalized (resolving `..` and
/// symlinks) and the canonical candidate must `starts_with` the canonical repo
/// root — so a `..`-escape or an out-of-repo absolute path is refused even when
/// the target exists. The returned path is the canonical, in-repo path to read.
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

/// Build the pool inside the live connection and run the pipeline to a report.
///
/// Split out so the `connect_with` closure body has a single typed future to
/// return. The pool is dropped at the end of this scope, winding its workers
/// down before the connection tears down.
#[allow(clippy::too_many_arguments)]
async fn run_pipeline_in_connection(
    cx: ConnectionTo<agent_client_protocol::Agent>,
    notifier: Arc<claude_agent::NotificationSender>,
    pool_config: PoolConfig,
    scope: Scope,
    repo_path: &Path,
    loader: &ValidatorLoader,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
    fleet_config: FleetConfig,
    progress: Option<ReviewProgressSender>,
    now: &str,
) -> agent_client_protocol::Result<Result<ReviewReport, AvpError>> {
    // ACP `initialize` is a ONCE-per-connection handshake. Do it here, before
    // the pool's workers issue any prompts, rather than per prompt: the pool
    // shares this single connection across N workers, so initializing per prompt
    // raced N concurrent handshakes at the one real agent process and wedged it
    // (the first prompt completed; the rest hung forever with no timeout). The
    // workers now only `new_session` + `prompt` over the already-initialized
    // connection.
    // Advertise the client filesystem capability the request handler backs:
    // `fs/read_text_file` is served (from disk under `repo_path`), while
    // `fs/write_text_file` is declined as unsupported — a review is read-only.
    // The agent consults these capabilities before issuing the corresponding
    // requests, so they must match `answer_agent_request`.
    cx.send_request(
        InitializeRequest::new(1.into()).client_capabilities(
            ClientCapabilities::new().fs(FileSystemCapabilities::new()
                .read_text_file(true)
                .write_text_file(false)),
        ),
    )
    .block_task()
    .await?;

    let pool = AgentPool::new(cx, notifier, pool_config);
    let report = run_review(
        scope,
        repo_path,
        loader,
        conn,
        embedder,
        &pool,
        fleet_config,
        progress.as_ref(),
        now,
    )
    .await;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use std::time::Duration;

    use acp_conformance::test_utils::{numbered_session_response, MockAgent, MockAgentAdapter};
    use agent_client_protocol::schema::{
        CancelNotification, ContentBlock, ContentChunk, NewSessionRequest, NewSessionResponse,
        PromptRequest, PromptResponse, SessionId, SessionNotification, SessionUpdate, StopReason,
        TextContent,
    };
    use futures::future::BoxFuture;
    use swissarmyhammer_common::test_utils::EnvVarGuard;
    use tokio::sync::Notify;

    use crate::review::fleet::{FleetConfig, ReviewProgressEvent};
    use crate::review::scope::Scope;
    use crate::review::test_support::{
        counting_tool_script, findings_json as shared_findings_json, fixture_runs, loader_with,
        malformed_findings_json, prompt_text, ruleset, seeded_dup_repo, seeded_two_file_dup_repo,
        verdict_json, ScriptedAdapter, ScriptedAgent, ScriptedAgentConfig, ScriptedReply,
        FIXTURE_RUNS_PER_PROOF,
    };

    /// How long a wedged pipeline may run before a test fails instead of
    /// hanging CI — the one tuning knob shared by every end-to-end test here.
    const PIPELINE_TIMEOUT: Duration = Duration::from_secs(30);

    /// Capacity of the scripted backend's broadcast channel — the channel the
    /// driver's `notification_rx` subscribes to. It comfortably exceeds any
    /// test's notification volume, so a slow subscriber never lags chunks away
    /// (`broadcast` silently drops for lagging receivers — exactly the failure
    /// class these tests exist to pin).
    const BACKEND_BROADCAST_CAPACITY: usize = 64;

    /// Capacity for the single-stream invariant tests, whose channels hold the
    /// whole streamed reply plus its end-of-turn marker.
    const PRELOADED_STREAM_CAPACITY: usize = 256;

    /// How long the slow-hop regression holds its tail chunk back. Longer than
    /// the 500 ms the drain used to sleep, so a wall-clock drain truncates the
    /// reply and a marker-driven one does not.
    const SLOW_HOP_DELAY: Duration = Duration::from_millis(700);

    /// Worker count for the pool the pipeline tests fan out across. Two workers
    /// let the multi-validator/multi-batch tests exercise genuine concurrency
    /// (more than one task in flight) while staying small and deterministic.
    const TEST_POOL_WORKERS: usize = 2;

    /// Content budget per fan-out batch, in bytes, for the batching tests.
    ///
    /// The budget is spent in RENDERED bytes, and each changed file here
    /// renders to ~2.4 KB (its ~180 bytes of source plus the numbered-line
    /// legend, the semantic diff, and the probe-evidence framing), so a 3000
    /// byte budget fits one file and forces a two-file diff to split across two
    /// batches — the boundary these tests pin.
    const TEST_BATCH_SIZE_BYTES: usize = 3000;

    /// The caller-formatted timestamp rendered verbatim into the report header.
    const TEST_NOW: &str = "2026-06-05 12:00";

    /// The abandoned-turn test's pool idle window, in milliseconds: short
    /// enough that the stalled turn is abandoned quickly, long enough that a
    /// live turn's keep-alives sail well under it.
    ///
    /// It no longer has to clear a post-response drain window. The drain ends
    /// on the agent's end-of-turn marker, which is itself notification traffic
    /// the liveness supervisor counts as progress, so a successful turn is
    /// never silent while it drains.
    const ABANDON_IDLE_WINDOW_MS: u64 = 800;

    /// Keep-alive interval for the live turn — a small fraction of the idle
    /// window so the streaming turn never looks stalled.
    // Keep-alive cadence as a fraction of the idle window: this many keep-alives
    // per idle window keeps the streaming turn well clear of the idle-abandon deadline.
    const KEEP_ALIVES_PER_IDLE_WINDOW: u64 = 16;
    const KEEP_ALIVE_INTERVAL: Duration =
        Duration::from_millis(ABANDON_IDLE_WINDOW_MS / KEEP_ALIVES_PER_IDLE_WINDOW);

    // ---- scripted ACP agent (shared harness) ------------------------------
    //
    // The scripted ACP agent lives in `crate::review::test_support`. Drive
    // tests run it shaped like a real backend: the agent
    // publishes every `session/update` onto its backend broadcast channel
    // (`notify_tx`), and the driver consumes a `subscribe()` of that channel as
    // `notification_rx` — the same authoritative stream production collects
    // from. With `bridge_to_connection`, the agent ALSO re-emits each reply
    // over the live connection, reproducing the real-handle shape whose second
    // copy the driver must NOT collect (the single-path invariant these tests
    // pin). `demand_permission` and `read_file` add the mid-turn agent→client
    // round-trips a real `claude` agent performs.

    /// A scripted agent streaming onto the backend broadcast `notify_tx`,
    /// optionally bridged onto the live connection too.
    fn broadcast_agent(
        script: Vec<(String, ScriptedReply)>,
        notify_tx: broadcast::Sender<SessionNotification>,
        bridge_to_connection: bool,
    ) -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            script,
            ScriptedAgentConfig {
                broadcast: Some(notify_tx),
                bridge_to_connection,
                ..ScriptedAgentConfig::default()
            },
        )
    }

    /// The fan-out → verify script every drive scenario shares: the fan-out
    /// prompt names the validator, the verify prompt names the claim.
    fn dedup_script() -> Vec<(String, ScriptedReply)> {
        vec![
            (
                "# Validator: deduplicate".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/lib.rs",
                    "compute duplicates old_compute",
                )),
            ),
            (
                "compute duplicates old_compute".to_string(),
                ScriptedReply::Text(confirm_json()),
            ),
        ]
    }

    /// The first line `seeded_dup_repo`'s uncommitted change ADDED: line 1 is
    /// the committed `fn placeholder() {}` and line 2 is the blank line, so the
    /// duplicated body starts here.
    ///
    /// A `review working` op reviews the diffs, so a finding must land on a
    /// line the change touched — one on line 1 is about pre-existing code and
    /// the verify guard refutes it before it reaches the report.
    const FIRST_CHANGED_LINE: u32 = 3;

    /// A findings array keyed the way drive's scenarios need it: rule `r`, on
    /// [`FIRST_CHANGED_LINE`] (the report assertions check that same line).
    fn findings_json(file: &str, claim: &str) -> String {
        shared_findings_json(file, FIRST_CHANGED_LINE, "r", claim)
    }

    /// A confirming verify verdict (the verify stage asks the agent to confirm
    /// or refute; `confirmed:true` keeps the finding).
    fn confirm_json() -> String {
        verdict_json(true, "the duplicate is real")
    }

    // ---- the test: drive `review working` end to end ---------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_working_drives_the_pipeline_over_a_scripted_agent() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        // The fan-out prompt names the validator + file; the verify prompt names
        // the claim. Both substrings map to the right scripted response.
        //
        // Shape the agent like a real `AcpAgentHandle`: it streams its reply onto
        // the backend broadcast (`notify_tx`) AND bridges the same notification
        // onto the live connection (`bridge_to_connection: true`), exactly as
        // `wrap_claude_into_handle`'s `forward_session_notifications` does. The
        // driver subscribes to `notify_tx` as `notification_rx`; the connection
        // re-emission must NOT be collected a second time. Under the old dual-path
        // driver every reply was concatenated twice.
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(dedup_script(), notify_tx, true);

        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(TEST_POOL_WORKERS),
            FleetConfig::default(),
            None,
            TEST_NOW,
        )
        .await;

        let report = report.expect("pipeline should produce a report");
        assert!(
            report
                .markdown()
                .contains(&format!("## Review Findings ({TEST_NOW})")),
            "report header must render: {}",
            report.markdown()
        );
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "the confirmed blocker finding must be rendered: {}",
            report.markdown()
        );
        assert!(
            report
                .markdown()
                .contains(&format!("src/lib.rs:{FIRST_CHANGED_LINE}")),
            "the finding's file:line must appear: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().confirmed(), 1);
    }

    // ---- a reply that does not parse is re-asked once before the pair fails ---

    /// A fan-out reply the strict parse rejects but the bracket repair rescues:
    /// the model restates the schema (an object shape the bare-object fallback
    /// cannot read) and names the file in a bracketed aside, so the first `[`
    /// the parser balances is the aside rather than the findings array.
    fn repairable_findings_json(file: &str, claim: &str) -> String {
        let array = serde_json::json!([{
            "file": file,
            "line": FIRST_CHANGED_LINE,
            "rule": "r",
            "claim": claim,
            "evidence": "per `duplicates`: 0.94",
            "suggestion": "extract a helper",
        }]);
        format!(
            "Each finding is {{\"file\": <path>, \"line\": <n>}}. I reviewed [{file}] \
             and found:\n{array}\n"
        )
    }

    /// How many fan-out prompts the agent served — every prompt carrying the
    /// validator header. The prime turn and the verify turn carry no validator
    /// header, so this counts asks of the validator itself.
    fn fan_out_prompt_count(agent: &Arc<ScriptedAgent>) -> usize {
        agent
            .seen_prompts()
            .iter()
            .filter(|prompt| prompt.contains("# Validator: deduplicate"))
            .count()
    }

    /// One malformed reply must not throw away a whole validator's review. The
    /// fan-out task is re-asked once; the second reply parses, so the findings
    /// land and the report is COMPLETE — no incomplete-run banner.
    ///
    /// The production failure this pins: the duplication validator's reply
    /// failed JSON parse, the engine yielded zero findings, marked the task
    /// failed, and a six-minute review ended INCOMPLETE with a full re-run as
    /// the only recovery.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_re_asks_a_fan_out_task_whose_first_reply_does_not_parse() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        // The fan-out entry answers its FIRST match with the malformed reply and
        // its second with a valid findings array — the re-ask is what reaches
        // that second element.
        let script = vec![
            (
                "# Validator: deduplicate".to_string(),
                ScriptedReply::sequence([
                    malformed_findings_json(),
                    findings_json("src/lib.rs", "compute duplicates old_compute"),
                ]),
            ),
            (
                "compute duplicates old_compute".to_string(),
                ScriptedReply::Text(confirm_json()),
            ),
        ];

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(script, notify_tx, true);
        let agent_probe = Arc::clone(&agent);
        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(TEST_POOL_WORKERS),
            FleetConfig::default(),
            None,
            TEST_NOW,
        )
        .await
        .expect("pipeline should produce a report");

        assert_eq!(
            fan_out_prompt_count(&agent_probe),
            2,
            "the validator is asked once and re-asked exactly once"
        );
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "the re-asked reply's finding must be rendered: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().confirmed(), 1);
        assert_eq!(
            report.counts().tasks_failed(),
            0,
            "a task that answers on the re-ask is not a failed task: {}",
            report.markdown()
        );
        assert!(
            !report.markdown().contains("INCOMPLETE"),
            "a recovered run is complete: {}",
            report.markdown()
        );
    }

    /// The re-ask is ONE re-ask. A validator that answers unparseably twice
    /// fails its pair exactly as before, and the report says so — honesty about
    /// a lost validator is the correct behaviour, not something the retry
    /// papers over.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_fails_the_pair_when_both_replies_do_not_parse() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        // `Text` answers every match with the same malformed reply, so the
        // re-ask does not parse either.
        let script = vec![(
            "# Validator: deduplicate".to_string(),
            ScriptedReply::Text(malformed_findings_json()),
        )];

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(script, notify_tx, true);
        let agent_probe = Arc::clone(&agent);
        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(TEST_POOL_WORKERS),
            FleetConfig::default(),
            None,
            TEST_NOW,
        )
        .await
        .expect("pipeline should produce a report");

        assert_eq!(
            fan_out_prompt_count(&agent_probe),
            2,
            "the task is re-asked once and only once"
        );
        assert_eq!(report.counts().findings(), 0);
        assert_eq!(
            report.counts().tasks_failed(),
            1,
            "two unparseable replies fail the pair: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("INCOMPLETE"),
            "a failed pair must still flag the run incomplete: {}",
            report.markdown()
        );
    }

    /// The cheap rung comes first. A reply the bracket repair can rescue is a
    /// success, so the validator is never asked twice — the re-ask is reserved
    /// for a reply no parse can read.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_repairs_a_bracketed_reply_without_re_asking() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        let script = vec![
            (
                "# Validator: deduplicate".to_string(),
                ScriptedReply::Text(repairable_findings_json(
                    "src/lib.rs",
                    "compute duplicates old_compute",
                )),
            ),
            (
                "compute duplicates old_compute".to_string(),
                ScriptedReply::Text(confirm_json()),
            ),
        ];

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(script, notify_tx, true);
        let agent_probe = Arc::clone(&agent);
        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(TEST_POOL_WORKERS),
            FleetConfig::default(),
            None,
            TEST_NOW,
        )
        .await
        .expect("pipeline should produce a report");

        assert_eq!(
            fan_out_prompt_count(&agent_probe),
            1,
            "a repaired parse spends no re-ask"
        );
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "the repaired reply's finding must be rendered: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().tasks_failed(), 0);
    }

    // ---- content-budgeted batching: a large diff fans out as several batches --

    /// A diff too large for one shared prime is split into content-budgeted
    /// batches at whole-file granularity, each batch fans out independently, and
    /// the findings from EVERY batch are merged into the one report. This is the
    /// fix for the production bug where a large diff overflowed the single shared
    /// prime and every fan-out task failed uniformly (15/15).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_batches_a_large_diff_and_merges_findings_across_batches() {
        let (repo, conn, embedder) = seeded_two_file_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        // Each changed file inlines ~180 bytes of source; a 250-byte batch_size
        // packs one file per batch, so the two files fan out as TWO batches. The
        // fan-out prompt for each batch is keyed on the validator header AND that
        // batch file's unique function SIGNATURE (`pub fn compute(` / `pub fn
        // render(`), which only ever appears in that file's own inlined source —
        // the bare symbol names leak across batches via shared probe evidence, so
        // a path/symbol needle would not discriminate. The verify prompt names the
        // per-file claim; verify entries are listed first so a verify prompt never
        // matches a fan-out entry.
        let script: Vec<(Vec<String>, ScriptedReply)> = vec![
            (
                vec!["lib-dup-claim".to_string()],
                ScriptedReply::Text(verdict_json(true, "the lib duplicate is real")),
            ),
            (
                vec!["other-dup-claim".to_string()],
                ScriptedReply::Text(verdict_json(true, "the other duplicate is real")),
            ),
            (
                vec![
                    "# Validator: deduplicate".to_string(),
                    "pub fn compute(".to_string(),
                ],
                ScriptedReply::Text(shared_findings_json("src/lib.rs", 3, "r", "lib-dup-claim")),
            ),
            (
                vec![
                    "# Validator: deduplicate".to_string(),
                    "pub fn render(".to_string(),
                ],
                ScriptedReply::Text(shared_findings_json(
                    "src/other.rs",
                    3,
                    "r",
                    "other-dup-claim",
                )),
            ),
        ];

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = ScriptedAgent::with_script(
            script,
            ScriptedAgentConfig {
                broadcast: Some(notify_tx),
                bridge_to_connection: true,
                ..ScriptedAgentConfig::default()
            },
        );
        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(TEST_POOL_WORKERS),
            FleetConfig::new(TEST_BATCH_SIZE_BYTES),
            None,
            TEST_NOW,
        )
        .await
        .expect("pipeline should produce a report");

        // Both batches' confirmed findings are merged into the one report.
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "batch 1's finding must be rendered: {}",
            report.markdown()
        );
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/other.rs:{FIRST_CHANGED_LINE}`")),
            "batch 2's finding must be rendered: {}",
            report.markdown()
        );
        assert_eq!(
            report.counts().findings(),
            2,
            "findings from both batches are merged: {}",
            report.markdown()
        );
        assert_eq!(report.counts().confirmed(), 2);
    }

    // ---- agent↔client permission deadlock reproduction (the keystone) ------

    /// The keystone regression test for the real-claude review hang.
    ///
    /// A real `claude` agent, mid-prompt, sends `session/request_permission`
    /// (tool consent) and `fs/read_text_file` requests BACK to the client and
    /// blocks on the answer before finishing the turn. The review's ACP `Client`
    /// (built in [`run_review_over_agent`]) must register an `on_receive_request`
    /// handler that answers them; without it the agent's request hangs unanswered,
    /// the prompt never returns, the pool never drains, and the whole review hangs
    /// forever (the production symptom: one `new_session`, one `end_turn`, silence).
    ///
    /// This drives the REAL `run_review_over_agent` (and therefore the real client
    /// built in `drive.rs`) with a [`ScriptedAgent::new_demanding`] mock whose
    /// `prompt` issues that permission round-trip. The whole pipeline is wrapped in
    /// a [`tokio::time::timeout`] so a HANG becomes a fast test FAILURE rather than
    /// a wedged CI. Before the fix (no client handler) this times out; after the
    /// fix (handler auto-approves) it completes and renders the confirmed finding.
    ///
    /// The fan-out and verify prompts BOTH demand a permission round-trip, so this
    /// also proves the pool advances past the first task — a single unanswered
    /// request anywhere in the pipeline would wedge it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_does_not_deadlock_when_agent_demands_permission_mid_prompt() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        // Every prompt this agent serves blocks on a `session/request_permission`
        // round-trip to the client first — both the fan-out prompt and the verify
        // prompt.
        let agent = ScriptedAgent::with_config(
            dedup_script(),
            ScriptedAgentConfig {
                broadcast: Some(notify_tx),
                demand_permission: true,
                ..ScriptedAgentConfig::default()
            },
        );

        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_review_over_agent(
                dyn_agent,
                notification_rx,
                Scope::Working,
                repo.path(),
                &loader,
                &conn,
                &embedder,
                PoolConfig::remote(TEST_POOL_WORKERS),
                FleetConfig::default(),
                None,
                TEST_NOW,
            ),
        )
        .await
        .expect(
            "the review must not hang when the agent demands a mid-prompt permission \
             round-trip; a timeout here means the review Client never answered the agent's \
             session/request_permission request (the production deadlock)",
        );

        let report = report.expect("pipeline should produce a report");
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "the confirmed blocker finding must be rendered after the permission round-trips: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().confirmed(), 1);
    }

    /// Companion to the deadlock reproduction: the agent demands an
    /// `fs/read_text_file` round-trip mid-prompt, and the client must serve the
    /// read from disk under `repo_path`. Proves the read handler returns the real
    /// file content (not just that the request is answered).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_serves_fs_read_text_file_from_disk_under_repo_path() {
        let (repo, conn, embedder) = seeded_dup_repo();
        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);

        let read_path = repo.path().join("src/lib.rs");
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = ScriptedAgent::with_config(
            dedup_script(),
            ScriptedAgentConfig {
                broadcast: Some(notify_tx),
                read_file: Some(read_path.clone()),
                ..ScriptedAgentConfig::default()
            },
        );
        let agent_probe = Arc::clone(&agent);

        let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));

        let report = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_review_over_agent(
                dyn_agent,
                notification_rx,
                Scope::Working,
                repo.path(),
                &loader,
                &conn,
                &embedder,
                PoolConfig::remote(TEST_POOL_WORKERS),
                FleetConfig::default(),
                None,
                TEST_NOW,
            ),
        )
        .await
        .expect("the review must serve fs/read_text_file without hanging");

        let _report = report.expect("pipeline should produce a report");
        let content = agent_probe
            .observed_read()
            .expect("the agent must have received a read response");
        assert!(
            content.contains("pub fn compute"),
            "the client must serve the real file content from disk, got: {content}"
        );
    }

    // ---- single-path notification invariant (the double-delivery guard) ----

    /// Split `text` into `parts` roughly equal chunks, returning one
    /// `AgentMessageChunk` notification per chunk for the given session. Streaming
    /// the reply across several chunks (as a real backend does) is what makes
    /// double-delivery corrupt: a duplicated, interleaved chunk stream cannot be
    /// reassembled back into the original JSON.
    fn chunked_notifications(
        session: &agent_client_protocol::schema::SessionId,
        text: &str,
        parts: usize,
    ) -> Vec<SessionNotification> {
        let bytes = text.as_bytes();
        let step = bytes.len().div_ceil(parts).max(1);
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < bytes.len() {
            // Respect char boundaries so the test payload (ASCII here) never
            // splits a multi-byte sequence.
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

    /// A pool worker's collector, subscribed to the notifier's broadcast and
    /// waiting for `session`'s stream to end.
    ///
    /// Subscribing is separate from draining because a broadcast subscriber
    /// only ever sees what is sent AFTER it subscribes. A test therefore
    /// subscribes first, then feeds the stream, then awaits
    /// [`drain_collector`] — the same order production runs in, where the
    /// collector is spawned before the prompt is sent.
    fn subscribe_collector(
        notifier: &Arc<claude_agent::NotificationSender>,
        session: agent_client_protocol::schema::SessionId,
    ) -> claude_agent::NotificationCollector {
        claude_agent::spawn_notification_collector(notifier.sender().subscribe(), session)
    }

    /// Drain a subscribed collector to the end of its turn's stream and return
    /// the reassembled reply, exactly as a pool worker does.
    async fn drain_collector(collector: claude_agent::NotificationCollector) -> String {
        let prompt_response = agent_client_protocol::schema::PromptResponse::new(
            agent_client_protocol::schema::StopReason::EndTurn,
        );
        claude_agent::collect_response_content(collector, &prompt_response)
            .await
            .expect("the collector must reach the end of the turn's stream")
    }

    /// The driver feeds the pool's collectors from EXACTLY ONE source: the
    /// agent's `notification_rx`, drained by the single [`forward_notifications`]
    /// task [`build_pool_notifier`] spawns. This is the real `AcpAgentHandle`
    /// shape — `notification_rx` is a `subscribe()` of the backend broadcast that
    /// `wrap_claude_into_handle` ALSO bridges onto the connection. The driver
    /// deliberately does not forward that connection re-emission a second time.
    ///
    /// This test pins both halves of the invariant deterministically:
    ///
    /// 1. The driver's single-feed seam reassembles the streamed reply EXACTLY
    ///    once (byte-for-byte equal to the original).
    /// 2. A second feed of the same stream — the old dual-path bug, where the
    ///    connection re-emission was also forwarded into the notifier — doubles
    ///    every chunk, so the collected text is twice as long and no longer the
    ///    original. The length doubling holds for every interleaving, so the
    ///    discriminating assertion is not flaky.
    ///
    /// Both halves end their stream the way a real backend does: with the
    /// end-of-turn marker, in band, behind the last chunk. That is what makes
    /// the collected text deterministic — there is no wall-clock window for a
    /// slow hop to lose a chunk behind.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn notification_rx_is_the_pools_single_collected_stream() {
        let session = agent_client_protocol::schema::SessionId::new("sess-single".to_string());
        let reply = findings_json("src/lib.rs", "compute duplicates old_compute");
        let stream = chunked_notifications(&session, &reply, 6);

        // --- (1) the driver's actual single-feed path collects the reply once ---

        // The backend broadcast: `notification_rx` is a `subscribe()` of it, just
        // as `wrap_claude_into_handle` resubscribes the agent's channel.
        let (notify_tx, notification_rx) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let (single_notifier, single_forward) = build_pool_notifier(notification_rx);
        let collector = subscribe_collector(&single_notifier, session.clone());
        for notif in &stream {
            let _ = notify_tx.send(notif.clone());
        }
        let _ = notify_tx.send(claude_agent::turn_complete_notification(session.clone()));
        let collected_single = drain_collector(collector).await;
        single_forward.abort();

        assert_eq!(
            collected_single, reply,
            "the driver's single feed must reassemble the agent reply exactly once"
        );

        // --- (2) the old dual-feed shape doubles the same stream ---------------
        //
        // Reproduce the bug: TWO forwarders draining the SAME backend broadcast
        // (one standing in for `notification_rx`, one for the connection
        // re-emission) both copy into one notifier. Every chunk lands twice, so
        // the collected text is twice as long for any interleaving — which is
        // precisely what corrupted the JSON the verify/fleet parser reads.
        //
        // The marker cannot ride this doubled feed: each forwarder would copy
        // it too, and the collector would stop at whichever copy arrived first,
        // mid-stream. So the source is CLOSED and both forwarders are awaited —
        // proof that every chunk has been copied — and only then is the marker
        // put on the notifier, behind all twelve.
        let (dual_tx, dual_rx_a) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let dual_rx_b = dual_tx.subscribe();
        let (dual_notifier, _seed) = claude_agent::NotificationSender::new(NOTIFY_BUFFER);
        let dual_notifier = Arc::new(dual_notifier);
        let dual_collector = subscribe_collector(&dual_notifier, session.clone());
        let fwd_a = tokio::spawn(forward_notifications(dual_rx_a, Arc::clone(&dual_notifier)));
        let fwd_b = tokio::spawn(forward_notifications(dual_rx_b, Arc::clone(&dual_notifier)));
        for notif in &stream {
            let _ = dual_tx.send(notif.clone());
        }
        drop(dual_tx);
        fwd_a.await.expect("forwarder a drains its closed source");
        fwd_b.await.expect("forwarder b drains its closed source");
        let _ = dual_notifier
            .send_update(claude_agent::turn_complete_notification(session))
            .await;
        let collected_dual = drain_collector(dual_collector).await;

        assert_ne!(
            collected_dual, reply,
            "a dual feed must NOT reassemble the original reply — this is the bug the \
             single-path driver fixes"
        );
        assert_eq!(
            collected_dual.len(),
            reply.len() * 2,
            "a dual feed doubles every chunk, doubling the collected length and \
             corrupting the JSON; the single-feed driver avoids this"
        );
    }

    /// The drive-seam regression for the fixed-window drain: a chunk that
    /// reaches the pool's notifier LATER than the 500 ms the drain used to
    /// sleep is still collected, because the drain ends on the agent's
    /// end-of-turn marker rather than on wall-clock time.
    ///
    /// Against the fixed sleep this test fails with a reply truncated at the
    /// slow chunk — the silent truncation that made
    /// [`notification_rx_is_the_pools_single_collected_stream`] flaky under
    /// full-suite load.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_chunk_forwarded_after_the_old_drain_window_is_still_collected() {
        let session = agent_client_protocol::schema::SessionId::new("sess-slow".to_string());
        let reply = findings_json("src/lib.rs", "compute duplicates old_compute");
        let stream = chunked_notifications(&session, &reply, 6);
        let (head, tail) = stream.split_at(stream.len() - 1);

        let (notify_tx, notification_rx) =
            broadcast::channel::<SessionNotification>(PRELOADED_STREAM_CAPACITY);
        let (notifier, forward) = build_pool_notifier(notification_rx);
        let collector = subscribe_collector(&notifier, session.clone());

        for notif in head {
            let _ = notify_tx.send(notif.clone());
        }

        // The tail chunk lands well past the old fixed window, WHILE the drain
        // is already running — the shape a loaded machine produces, where a
        // forwarding hop does not get scheduled before the drain gives up.
        let tail: Vec<SessionNotification> = tail.to_vec();
        let slow_hop = tokio::spawn(async move {
            tokio::time::sleep(SLOW_HOP_DELAY).await;
            for notif in tail {
                let _ = notify_tx.send(notif);
            }
            let _ = notify_tx.send(claude_agent::turn_complete_notification(session));
        });

        let collected = drain_collector(collector).await;
        slow_hop.await.expect("the slow hop completes");
        forward.abort();

        assert_eq!(
            collected, reply,
            "a chunk forwarded after the old 500 ms drain window must still be collected"
        );
    }

    // ---- abandoned-turn tolerance (the dropped-receiver cascade) ----------

    /// Scripted agent for the abandoned-turn regression, driven through the
    /// shared `acp_conformance` [`MockAgent`] harness.
    ///
    /// Reproduces the production cascade shape on top of the pool's real
    /// liveness supervision:
    ///
    /// - The `staller` fan-out prompt goes silent until the pool abandons the
    ///   turn (dropping its `block_task` response receiver) and sends
    ///   `session/cancel`; the [`MockAgent::cancel`] hook releases it and the
    ///   prompt answers LATE — a response whose awaiter is already gone.
    /// - The `deduplicate` fan-out prompt holds its own (live, keep-alive
    ///   streaming) turn open until that late answer has been produced, so the
    ///   client connection MUST survive routing the late response before any
    ///   subsequent turn can complete.
    struct LateAnsweringAgent {
        next_session: AtomicUsize,
        /// Backend broadcast the driver's `notification_rx` subscribes to.
        notify_tx: broadcast::Sender<SessionNotification>,
        /// Notified by [`MockAgent::cancel`] when the pool abandons the
        /// stalled turn.
        cancelled: Notify,
        /// Notified once the stalled prompt has produced its late answer.
        late_answered: Notify,
    }

    impl LateAnsweringAgent {
        /// The stalled turn: stream ONE progress chunk to arm the per-turn idle
        /// window (the pool no longer arms it at submission — a turn that never
        /// streams is bounded by the ceiling, not idle), then wedge silently
        /// until the pool's idle liveness abandons this turn and cancels the
        /// session, and finally answer late — the response receiver is already
        /// dropped when this reply reaches the client. This models a real
        /// started-then-stalled turn (decoded a little, then wedged on an
        /// unanswered nested request).
        async fn stall_until_cancelled(&self, session_id: &SessionId) -> PromptResponse {
            self.stream_reply(session_id, "starting".to_string());
            self.cancelled.notified().await;
            self.late_answered.notify_one();
            self.mark_turn_complete(session_id);
            PromptResponse::new(StopReason::EndTurn)
        }

        /// The live turn's gate: hold the turn open — streaming keep-alive
        /// thought chunks so its own idle window never fires — until the late
        /// answer is on the wire. The connection processes inbound messages in
        /// order, so this turn can only complete after the client has routed
        /// the late response and survived.
        async fn keep_alive_until_late_answer(&self, session_id: &SessionId) {
            loop {
                tokio::select! {
                    _ = self.late_answered.notified() => break,
                    _ = tokio::time::sleep(KEEP_ALIVE_INTERVAL) => {
                        let keep_alive = SessionUpdate::AgentThoughtChunk(
                            ContentChunk::new(ContentBlock::Text(TextContent::new("…"))),
                        );
                        let _ = self
                            .notify_tx
                            .send(SessionNotification::new(session_id.clone(), keep_alive));
                    }
                }
            }
        }

        /// Stream `reply` onto the backend broadcast as a single
        /// `agent_message_chunk`, the way every scripted turn answers.
        fn stream_reply(&self, session_id: &SessionId, reply: String) {
            let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(reply),
            )));
            let _ = self
                .notify_tx
                .send(SessionNotification::new(session_id.clone(), update));
        }

        /// Close the turn's notification stream onto the same broadcast, as a
        /// real backend does, so the pool's collector stops on that marker.
        fn mark_turn_complete(&self, session_id: &SessionId) {
            let _ = self
                .notify_tx
                .send(claude_agent::turn_complete_notification(session_id.clone()));
        }
    }

    impl MockAgent for LateAnsweringAgent {
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
                let text = prompt_text(&request);

                if text.contains("# Validator: staller") {
                    return Ok(self.stall_until_cancelled(&request.session_id).await);
                }

                let reply = if text.contains("# Validator: deduplicate") {
                    self.keep_alive_until_late_answer(&request.session_id).await;
                    findings_json("src/lib.rs", "compute duplicates old_compute")
                } else if text.contains("compute duplicates old_compute") {
                    confirm_json()
                } else {
                    "[]".to_string()
                };

                self.stream_reply(&request.session_id, reply);
                self.mark_turn_complete(&request.session_id);
                Ok(PromptResponse::new(StopReason::EndTurn))
            })
        }

        fn cancel<'a>(
            &'a self,
            _notification: CancelNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move {
                self.cancelled.notify_one();
                Ok(())
            })
        }
    }

    // ---- path-traversal confinement (read_text_file_under_repo) -----------

    /// Build a `fs/read_text_file` request for `path` (relative or absolute),
    /// the way a mid-prompt agent issues one.
    fn read_request(
        path: impl Into<std::path::PathBuf>,
    ) -> agent_client_protocol::schema::ReadTextFileRequest {
        agent_client_protocol::schema::ReadTextFileRequest::new(
            agent_client_protocol::schema::SessionId::new("sess-read".to_string()),
            path.into(),
        )
    }

    /// A legitimate in-repo relative read returns the file's content.
    #[test]
    fn read_text_file_under_repo_serves_an_in_repo_relative_path() {
        let (repo, _conn, _embedder) = seeded_dup_repo();

        let content = read_text_file_under_repo(repo.path(), &read_request("src/lib.rs"))
            .expect("an in-repo relative read must succeed");
        assert!(
            content.contains("pub fn compute"),
            "the in-repo read must return the real file content, got: {content}"
        );
    }

    /// A `..`-escape relative path that climbs out of the repo is rejected,
    /// even though the target exists on disk.
    #[test]
    fn read_text_file_under_repo_rejects_a_dotdot_escape() {
        let (repo, _conn, _embedder) = seeded_dup_repo();

        // Plant a file in the repo's PARENT so a real, readable target exists
        // outside the confinement boundary — the read must still be refused.
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

    /// An absolute path pointing outside the repo is rejected.
    #[test]
    fn read_text_file_under_repo_rejects_an_absolute_outside_path() {
        let (repo, _conn, _embedder) = seeded_dup_repo();

        let err = read_text_file_under_repo(repo.path(), &read_request("/etc/passwd"))
            .expect_err("an absolute outside path must be rejected");
        assert!(
            err.contains("outside the repository"),
            "the rejection must name the confinement boundary, got: {err}"
        );
    }

    /// An absolute path that DOES resolve under the repo is honored — the
    /// confinement check is on location, not on the absolute/relative shape.
    #[test]
    fn read_text_file_under_repo_serves_an_absolute_in_repo_path() {
        let (repo, _conn, _embedder) = seeded_dup_repo();

        let abs = repo.path().join("src/lib.rs");
        let content = read_text_file_under_repo(repo.path(), &read_request(abs))
            .expect("an absolute in-repo read must succeed");
        assert!(
            content.contains("pub fn compute"),
            "an absolute in-repo read must return the real content, got: {content}"
        );
    }

    /// The drive-seam regression for the dropped-receiver cascade (the
    /// 2026-06-11 calcutron incident): one fan-out turn goes silent, the pool's
    /// per-turn liveness abandons it — dropping its `block_task` receiver — and
    /// the agent answers AFTER the abandonment. Without `TolerantResponseRouter`
    /// in [`run_review_over_agent`]'s client builder, that late response kills
    /// the connection's dispatch loop (`"failed to send response, receiver
    /// dropped"`), `connect_with` returns `Err`, and the whole review fails
    /// wholesale. With it, the abandoned turn degrades to a single failed task
    /// and the remaining turns — the other validator's fan-out and its verify —
    /// complete on the SAME connection, mirroring the
    /// `agent-client-protocol-extras` `tolerant_routing` test at this seam.
    #[tracing_test::traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_survives_a_late_response_to_an_abandoned_turn() {
        let (repo, conn, embedder) = seeded_dup_repo();
        // A second, untracked working file (Working scope includes untracked)
        // so a second validator gets its own concurrent fan-out turn.
        repo.write(
            "src/other.rs",
            "fn other_placeholder() {}\n\nfn extra() {}\n",
        );

        // Two validators over disjoint files → two concurrent fan-out turns on
        // the pool's two workers: `staller` wedges and is abandoned,
        // `deduplicate` stays live and must complete after the late response.
        let mut loader = loader_with("deduplicate", "src/lib.rs", &["duplicates"]);
        loader.add_builtin_ruleset(ruleset("staller", "src/other.rs", &[]));

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = Arc::new(LateAnsweringAgent {
            next_session: AtomicUsize::new(0),
            notify_tx,
            cancelled: Notify::new(),
            late_answered: Notify::new(),
        });
        let dyn_agent = DynConnectTo::new(MockAgentAdapter(agent));

        let report = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_review_over_agent(
                dyn_agent,
                notification_rx,
                Scope::Working,
                repo.path(),
                &loader,
                &conn,
                &embedder,
                // A sub-second idle window so the stalled turn is abandoned
                // fast; the live turn's keep-alives sail well under it. See
                // [`ABANDON_IDLE_WINDOW_MS`] for how it is sized.
                PoolConfig::remote(TEST_POOL_WORKERS)
                    .with_idle_timeout(Duration::from_millis(ABANDON_IDLE_WINDOW_MS)),
                FleetConfig::default(),
                None,
                TEST_NOW,
            ),
        )
        .await
        .expect("the review must not hang when a turn is abandoned and answered late");

        let report = report.expect(
            "a late response to an abandoned turn must fail that turn only, \
             not the whole review connection",
        );
        assert!(
            report
                .markdown()
                .contains(&format!("- [ ] `src/lib.rs:{FIRST_CHANGED_LINE}`")),
            "the live validator's confirmed blocker must still be rendered: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().confirmed(), 1);
    }

    // ---- tool rules: the tool reviews the code, not an LLM ------------------

    use crate::validators::types::{Rule, Supersedes, ToolDoctor, ToolScope, ToolSpec};
    use crate::validators::ValidatorLoader;

    /// A `files`-scope script that reports one `path:line: message` finding per
    /// line containing `TODO`, exits 0 either way, and — when an `explode`
    /// marker file exists in the working directory (planted at the repo root by
    /// the tool-error test, never in the fixtures directory) — breaks with a
    /// nonzero exit and stderr instead.
    const TODO_TOOL_SCRIPT: &str = r#"
if [ -f explode ]; then echo "the linter exploded" >&2; exit 3; fi
for f in "$@"; do awk -v f="$f" '/TODO/ { print f ":" NR ": TODO left in code" }' "$f"; done
"#;

    /// The prompt rule body the tool rule supersedes — a marker the tests can
    /// look for in prompts.
    const PROMPT_RULE_BODY: &str = "Report public items without documentation.";

    /// The tool rule body marker: no LLM prompt may ever carry it.
    const TOOL_RULE_BODY: &str = "TOOL RULE BODY - an LLM must never read this";

    /// A loader holding one `docs` ruleset over `*.rs`: the `missing-docs`
    /// prompt rule plus the `docs-tool` tool rule superseding it, based at
    /// `base` (where the doctor health check reads `fixtures/`).
    fn tool_rule_loader(base: &std::path::Path, check_command: &str) -> ValidatorLoader {
        let mut ruleset = ruleset("docs", "*.rs", &[]);
        ruleset.base_path = base.to_path_buf();
        ruleset.rules = vec![
            Rule {
                name: "missing-docs".to_string(),
                description: "docs by prompt".to_string(),
                body: PROMPT_RULE_BODY.to_string(),
                ..Rule::default()
            },
            Rule {
                name: "docs-tool".to_string(),
                description: "docs by tool".to_string(),
                body: TOOL_RULE_BODY.to_string(),
                supersedes: Supersedes::from_iter(["missing-docs"]),
                tool: Some(ToolSpec {
                    scope: ToolScope::Files,
                    run: TODO_TOOL_SCRIPT.to_string(),
                    doctor: Some(ToolDoctor {
                        check_command: check_command.to_string(),
                        check_version_command: None,
                        fix_hint: None,
                    }),
                    install: None,
                }),
                ..Rule::default()
            },
        ];
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset);
        loader
    }

    /// A repo whose `src/lib.rs` carries a TODO marker on line 2, committed so
    /// the file scope resolves against a valid HEAD.
    fn todo_repo() -> (crate::review::test_support::TestRepo, Connection) {
        let repo = crate::review::test_support::TestRepo::new();
        repo.write("src/lib.rs", "fn a() {}\n// TODO: fix this\n");
        repo.commit("initial");
        (repo, crate::review::test_support::index_conn())
    }

    /// Drive `review file src/lib.rs` over `agent` with `loader`, emitting
    /// progress on `progress` when the caller wants to watch the run.
    async fn drive_file_review(
        agent: Arc<ScriptedAgent>,
        notification_rx: broadcast::Receiver<SessionNotification>,
        repo: &crate::review::test_support::TestRepo,
        conn: &Connection,
        loader: &crate::validators::ValidatorLoader,
        progress: Option<ReviewProgressSender>,
    ) -> crate::review::synthesize::ReviewReport {
        let embedder = model_embedding::mock::MockEmbedder::new(crate::review::test_support::DIM);
        tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_review_over_agent(
                DynConnectTo::new(ScriptedAdapter::new(agent)),
                notification_rx,
                Scope::File("src/lib.rs".to_string()),
                repo.path(),
                loader,
                conn,
                &embedder,
                PoolConfig::remote(TEST_POOL_WORKERS),
                FleetConfig::default(),
                progress,
                TEST_NOW,
            ),
        )
        .await
        .expect("the review file pipeline must not hang")
        .expect("the review file pipeline must produce a report")
    }

    /// Acceptance: with the tool present and healthy, `review file` reports the
    /// tool's findings and issues ZERO LLM validator calls for the pair — the
    /// tool rule supersedes the only prompt rule, so no fleet task exists.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_file_runs_a_healthy_tool_rule_with_zero_llm_calls() {
        let (repo, conn) = todo_repo();
        let base = tempfile::tempdir().expect("tool rule base dir");
        crate::review::test_support::write_tool_rule_fixtures(base.path(), "docs-tool");
        let loader = tool_rule_loader(base.path(), "true");

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(vec![], notify_tx, true);

        let report = drive_file_review(
            Arc::clone(&agent),
            notification_rx,
            &repo,
            &conn,
            &loader,
            None,
        )
        .await;

        assert!(
            report.markdown().contains("- [ ] `src/lib.rs:2`"),
            "the tool finding must render as a checklist item: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("TODO left in code"),
            "the tool's message must be the finding's claim: {}",
            report.markdown()
        );
        assert_eq!(report.counts().findings(), 1);
        assert_eq!(report.counts().confirmed(), 1);
        assert_eq!(report.counts().tool_errors(), 0);
        assert_eq!(
            agent.seen_prompts(),
            Vec::<String>::new(),
            "no LLM prompt may run for a pair a healthy tool rule covers"
        );
    }

    /// Acceptance: with the tool absent, the superseded prompt rule runs as an
    /// ordinary LLM task and the report notes the fallback. The tool rule's
    /// body never reaches any prompt.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_file_falls_back_to_the_prompt_rule_when_the_tool_is_missing() {
        let (repo, conn) = todo_repo();
        let base = tempfile::tempdir().expect("tool rule base dir");
        crate::review::test_support::write_tool_rule_fixtures(base.path(), "docs-tool");
        // `false` as the doctor check: the tool is "missing".
        let loader = tool_rule_loader(base.path(), "false");

        let script = vec![
            (
                "undocumented-item-claim".to_string(),
                ScriptedReply::Text(verdict_json(true, "the missing docs are real")),
            ),
            (
                "# Validator: docs".to_string(),
                ScriptedReply::Text(shared_findings_json(
                    "src/lib.rs",
                    1,
                    "missing-docs",
                    "undocumented-item-claim",
                )),
            ),
        ];
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(script, notify_tx, true);

        let report = drive_file_review(
            Arc::clone(&agent),
            notification_rx,
            &repo,
            &conn,
            &loader,
            None,
        )
        .await;

        assert!(
            report.markdown().contains("- [ ] `src/lib.rs:1`"),
            "the prompt rule's confirmed finding must render: {}",
            report.markdown()
        );
        assert!(
            report
                .markdown()
                .contains("prompt rule 'missing-docs' ran instead"),
            "the report must note the prompt fallback: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("docs-tool"),
            "the fallback note must name the unavailable tool rule: {}",
            report.markdown()
        );
        let prompts = agent.seen_prompts();
        assert!(
            prompts.iter().any(|p| p.contains(PROMPT_RULE_BODY)),
            "the superseded prompt rule must run as a normal LLM task"
        );
        assert!(
            prompts.iter().all(|p| !p.contains(TOOL_RULE_BODY)),
            "no LLM prompt may ever carry a tool rule's body"
        );
    }

    /// Acceptance: a tool run that exits nonzero is reported as a tool error —
    /// not as clean and not as findings — with the raw stderr in the report.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_file_reports_a_nonzero_tool_exit_as_a_tool_error() {
        let (repo, conn) = todo_repo();
        // The marker file makes the run script break AT THE REPO ROOT only;
        // the doctor's fixture runs execute in the fixtures directory, where
        // no marker exists, so the rule is healthy and the run is attempted.
        std::fs::write(repo.path().join("explode"), "").expect("write explode marker");
        let base = tempfile::tempdir().expect("tool rule base dir");
        crate::review::test_support::write_tool_rule_fixtures(base.path(), "docs-tool");
        let loader = tool_rule_loader(base.path(), "true");

        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(vec![], notify_tx, true);

        let report = drive_file_review(
            Arc::clone(&agent),
            notification_rx,
            &repo,
            &conn,
            &loader,
            None,
        )
        .await;

        assert_eq!(
            report.counts().tool_errors(),
            1,
            "a broken tool run must count as a tool error: {}",
            report.markdown()
        );
        assert_eq!(
            report.counts().findings(),
            0,
            "a broken tool run must contribute no findings: {}",
            report.markdown()
        );
        assert!(
            report
                .markdown()
                .contains("tool rule 'docs/docs-tool' failed"),
            "the report must name the broken tool rule: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("the linter exploded"),
            "the raw stderr must reach the report: {}",
            report.markdown()
        );
        assert!(
            !report.markdown().contains("Nothing in scope to review."),
            "a broken tool run must never read as an empty scope: {}",
            report.markdown()
        );
    }

    // ---- tool rules: the health proof is kept, and the runs overlap the fleet

    /// How many reviews the stored-verdict acceptance test drives: the first
    /// proves the rule, the second must read the proof back.
    const REVIEWS_IN_THE_ACCEPTANCE: usize = 2;

    /// The tool version the test rules report. Any fixed string does: what the
    /// stored verdict turns on is that the string does not change between two
    /// reviews.
    const TEST_TOOL_VERSION: &str = "1.0.0";

    /// How long the overlap test waits for a fleet pair to finish while the
    /// tool script is still blocked. Well under [`PIPELINE_TIMEOUT`], so the
    /// test states what went wrong instead of hanging.
    const OVERLAP_WAIT: Duration = Duration::from_secs(10);

    /// How many times the blocking run script looks for its release file
    /// before it gives up. With [`TOOL_WAIT_STEP_SECONDS`] this bounds the
    /// wait at 30 seconds, so a run that never overlaps still ends.
    const TOOL_WAIT_STEPS: u32 = 600;

    /// How long the blocking run script sleeps between two looks for its
    /// release file.
    const TOOL_WAIT_STEP_SECONDS: &str = "0.05";

    /// A ruleset named `set` over `*.rs` whose only rule is the tool rule
    /// `rule`, running `script`.
    ///
    /// The doctor block reports the tool present and [`TEST_TOOL_VERSION`] as
    /// its version, so the rule is one a stored verdict can be keyed on.
    /// `base` is where the health check reads `fixtures/`.
    fn tool_only_ruleset(
        set: &str,
        rule: &str,
        base: &std::path::Path,
        script: &str,
    ) -> crate::validators::types::RuleSet {
        let mut ruleset = ruleset(set, "*.rs", &[]);
        ruleset.base_path = base.to_path_buf();
        ruleset.rules = vec![Rule {
            name: rule.to_string(),
            description: "findings by tool".to_string(),
            body: TOOL_RULE_BODY.to_string(),
            tool: Some(ToolSpec {
                scope: ToolScope::Files,
                run: script.to_string(),
                doctor: Some(ToolDoctor {
                    check_command: "true".to_string(),
                    check_version_command: Some(format!("echo {TEST_TOOL_VERSION}")),
                    fix_hint: None,
                }),
                install: None,
            }),
            ..Rule::default()
        }];
        ruleset
    }

    /// A ruleset named `set` over `*.rs` whose only rule is the `missing-docs`
    /// prompt rule, so the fleet has a pair to review.
    fn prompt_only_ruleset(set: &str) -> crate::validators::types::RuleSet {
        let mut ruleset = ruleset(set, "*.rs", &[]);
        ruleset.rules = vec![Rule {
            name: "missing-docs".to_string(),
            description: "docs by prompt".to_string(),
            body: PROMPT_RULE_BODY.to_string(),
            ..Rule::default()
        }];
        ruleset
    }

    /// A `files`-scope script that reports one `path:line: message` finding per
    /// line holding `TODO`, and waits for a `released` file first when a
    /// `wait-here` marker sits in its working directory.
    ///
    /// The test plants `wait-here` at the repo root alone, so the doctor's
    /// fixture runs never wait.
    fn blocking_tool_script() -> String {
        format!(
            r#"
if [ -f wait-here ]; then
  waited=0
  while [ ! -f released ] && [ "$waited" -lt {TOOL_WAIT_STEPS} ]; do
    sleep {TOOL_WAIT_STEP_SECONDS}
    waited=$((waited+1))
  done
fi
for f in "$@"; do awk -v f="$f" '/TODO/ {{ print f ":" NR ": TODO left in code" }}' "$f"; done
"#
        )
    }

    /// Acceptance: the fixture proof is kept. Two reviews of the same
    /// workspace, with the same tool and the same rule, run the fixture
    /// scripts once — the second reads the stored verdict.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_second_review_of_an_unchanged_tool_rule_runs_no_fixture_script() {
        let (repo, conn) = todo_repo();
        let base = tempfile::tempdir().expect("tool rule base dir");
        crate::review::test_support::write_counted_tool_rule_fixtures(base.path(), "docs-tool");
        let counter = base.path().join("fixture-runs");
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(tool_only_ruleset(
            "docs",
            "docs-tool",
            base.path(),
            &counting_tool_script(&counter),
        ));

        for _ in 0..REVIEWS_IN_THE_ACCEPTANCE {
            let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
            let agent = broadcast_agent(vec![], notify_tx, true);
            let report =
                drive_file_review(agent, notification_rx, &repo, &conn, &loader, None).await;
            assert_eq!(
                report.counts().tool_errors(),
                0,
                "the tool rule must stay healthy across both reviews: {}",
                report.markdown()
            );
            assert_eq!(
                report.counts().findings(),
                1,
                "every review must report the tool's finding: {}",
                report.markdown()
            );
        }

        assert_eq!(
            fixture_runs(&counter),
            FIXTURE_RUNS_PER_PROOF,
            "the first review proves the rule with the fail fixture and the pass fixture; \
             the second must read that verdict back instead of proving it again"
        );
    }

    /// Wait for a `PairDone` naming `validator`, up to [`OVERLAP_WAIT`],
    /// keeping every event it read on the way.
    ///
    /// `false` means none arrived in time — the fleet never finished a pair
    /// while the tool script was still running.
    async fn wait_for_pair_done(
        progress: &mut tokio::sync::mpsc::UnboundedReceiver<ReviewProgressEvent>,
        validator: &str,
        seen: &mut Vec<ReviewProgressEvent>,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + OVERLAP_WAIT;
        loop {
            match tokio::time::timeout_at(deadline, progress.recv()).await {
                Ok(Some(event)) => {
                    let finished = matches!(
                        &event,
                        ReviewProgressEvent::PairDone { validator: name, .. } if name == validator
                    );
                    seen.push(event);
                    if finished {
                        return true;
                    }
                }
                Ok(None) | Err(_) => return false,
            }
        }
    }

    /// Every finding `events` reported under `validator`.
    fn findings_of<'a>(
        events: &'a [ReviewProgressEvent],
        validator: &str,
    ) -> Vec<&'a crate::review::types::Finding> {
        events
            .iter()
            .filter_map(|event| match event {
                ReviewProgressEvent::Findings {
                    validator: name,
                    findings,
                } if name == validator => Some(findings),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// Acceptance: the tool scripts run WHILE the fleet works, and both sides'
    /// findings reach synthesis under their own rule.
    ///
    /// The run script blocks until the test releases it, and the test releases
    /// it only after a fleet pair has finished. A run that waited for the tool
    /// before it started the fleet therefore never reaches the release.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_tool_run_overlaps_the_fleet_instead_of_delaying_it() {
        let (repo, conn) = todo_repo();
        let base = tempfile::tempdir().expect("tool rule base dir");
        crate::review::test_support::write_tool_rule_fixtures(base.path(), "todo-tool");
        std::fs::write(repo.path().join("wait-here"), "").expect("plant the wait marker");

        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(prompt_only_ruleset("docs"));
        loader.add_builtin_ruleset(tool_only_ruleset(
            "todo",
            "todo-tool",
            base.path(),
            &blocking_tool_script(),
        ));

        let script = vec![
            (
                "undocumented-item-claim".to_string(),
                ScriptedReply::Text(verdict_json(true, "the missing docs are real")),
            ),
            (
                "# Validator: docs".to_string(),
                ScriptedReply::Text(shared_findings_json(
                    "src/lib.rs",
                    1,
                    "missing-docs",
                    "undocumented-item-claim",
                )),
            ),
        ];
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(script, notify_tx, true);
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();

        let (report, watched) = tokio::join!(
            drive_file_review(
                Arc::clone(&agent),
                notification_rx,
                &repo,
                &conn,
                &loader,
                Some(progress_tx),
            ),
            async {
                let mut seen = Vec::new();
                let finished = wait_for_pair_done(&mut progress_rx, "docs", &mut seen).await;
                std::fs::write(repo.path().join("released"), "")
                    .expect("release the blocked tool script");
                (finished, seen)
            }
        );
        let (fleet_finished, mut events) = watched;
        while let Some(event) = progress_rx.recv().await {
            events.push(event);
        }

        assert!(
            fleet_finished,
            "a fleet pair must finish while the tool script is still running; \
             the tool run delayed the fan-out instead of overlapping it"
        );
        assert!(
            report.markdown().contains("- [ ] `src/lib.rs:2`"),
            "the tool finding must survive the overlap and reach synthesis: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("- [ ] `src/lib.rs:1`"),
            "the prompt rule's finding must reach synthesis too: {}",
            report.markdown()
        );
        assert_eq!(
            report.counts().tool_errors(),
            0,
            "the overlapped tool run must not break: {}",
            report.markdown()
        );
        let prompts = agent.seen_prompts();
        assert!(
            prompts.iter().any(|p| p.contains(PROMPT_RULE_BODY)),
            "the prompt rule must run as a normal LLM task"
        );
        assert!(
            prompts.iter().all(|p| !p.contains(TOOL_RULE_BODY)),
            "no LLM prompt may ever carry a tool rule's body"
        );

        let tool_findings = findings_of(&events, "todo");
        assert_eq!(
            tool_findings.len(),
            1,
            "the tool must report its finding under its own validator: {events:?}"
        );
        assert_eq!(
            tool_findings[0].rule.as_deref(),
            Some("todo-tool"),
            "the tool finding must carry the tool rule's name"
        );
        let prompt_findings = findings_of(&events, "docs");
        assert!(
            prompt_findings
                .iter()
                .all(|finding| finding.rule.as_deref() == Some("missing-docs")),
            "no tool finding may be reported as a prompt-rule finding: {prompt_findings:?}"
        );
    }

    // ---- the shipped builtin layer's own fixture data

    /// The builtin set whose fixtures the review below reads, and whose name
    /// the planted user set shadows.
    const SHADOWED_BUILTIN_SET: &str = "code-hygiene";

    /// A fixture the `code-hygiene` set SHIPS, as the review scope holds its
    /// path. It stands under [`builtin_validators_dir`], the runtime builtin
    /// fixture root.
    const SHIPPED_BUILTIN_FIXTURE: &str =
        "builtin/validators/code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl";

    /// Acceptance: a REAL review of a SHIPPED builtin fixture reports the file
    /// as excluded with its reason, while the USER layer carries a set of the
    /// same name.
    ///
    /// This is the runtime shape `sah init` leaves behind: `~/.validators/`
    /// holds a copy of every builtin set, and a same-named copy replaces the
    /// builtin entry of the loader's ruleset map. The builtin set's own
    /// `fixtures/` directory must stay a fixture root through that, or a
    /// shipped fixture leaves the scope in silence — which is the one outcome
    /// the exclusion exists to stop.
    ///
    /// The loader is the one [`crate::load_rules`] builds for a live review,
    /// the repository is this one, and the fixture is a file it ships.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial_test::serial(cwd)]
    async fn review_file_excludes_a_shipped_builtin_fixture_under_a_shadowing_user_set() {
        let home = tempfile::tempdir().expect("temp home");
        crate::review::test_support::write_tool_rule_ruleset(
            &home.path().join(".validators"),
            SHADOWED_BUILTIN_SET,
            "**/*.rs",
            "true",
        )
        .expect("write the shadowing user set");
        let _env = EnvVarGuard::set("HOME", home.path());

        let repo = crate::review::test_support::repo_root();
        assert!(
            repo.join(SHIPPED_BUILTIN_FIXTURE).exists(),
            "the shipped fixture must exist for this test to mean anything"
        );

        // The loader a live review builds: builtins, then the user layer, then
        // the project layer of the repository under review.
        let loader = crate::load_rules(Some(&repo)).expect("the live review's loader");
        assert_eq!(
            loader
                .get_ruleset(SHADOWED_BUILTIN_SET)
                .map(|set| set.source.clone()),
            Some(crate::validators::ValidatorSource::User),
            "precondition: the user layer's set must shadow the builtin one"
        );

        let conn = crate::review::test_support::index_conn();
        let embedder = model_embedding::mock::MockEmbedder::new(crate::review::test_support::DIM);
        let (notify_tx, notification_rx) = broadcast::channel(BACKEND_BROADCAST_CAPACITY);
        let agent = broadcast_agent(vec![], notify_tx, true);

        let report = tokio::time::timeout(
            PIPELINE_TIMEOUT,
            run_review_over_agent(
                DynConnectTo::new(ScriptedAdapter::new(agent)),
                notification_rx,
                Scope::File(SHIPPED_BUILTIN_FIXTURE.to_string()),
                &repo,
                &loader,
                &conn,
                &embedder,
                PoolConfig::remote(TEST_POOL_WORKERS),
                FleetConfig::default(),
                None,
                TEST_NOW,
            ),
        )
        .await
        .expect("the review file pipeline must not hang")
        .expect("the review file pipeline must produce a report");

        assert_eq!(
            report.counts().skipped_files(),
            [SHIPPED_BUILTIN_FIXTURE.to_string()],
            "the shipped fixture is the one file the run did not review: {}",
            report.markdown()
        );
        assert!(
            report.markdown().contains("validator fixture"),
            "the reason must be reported: {}",
            report.markdown()
        );
        assert!(
            !report.markdown().contains("Nothing in scope to review."),
            "an excluded fixture is a reported exclusion, never an empty scope: {}",
            report.markdown()
        );
    }
}
