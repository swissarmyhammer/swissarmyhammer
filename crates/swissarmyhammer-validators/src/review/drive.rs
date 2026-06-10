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
//! `wrap_claude_into_handle`/`wrap_llama_into_handle` bridge onto the connection
//! via `forward_session_notifications`. Because that bridge re-emits the very same
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
use model_embedding::TextEmbedder;
use rusqlite::Connection;
use tokio::sync::broadcast;

use crate::error::AvpError;
use crate::review::fleet::FleetConfig;
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
/// # Errors
///
/// Returns the [`AvpError`] from [`run_review`](crate::review::run_review) on a
/// scope/index failure, or [`AvpError::Agent`] when the ACP connection itself
/// fails to stand up.
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
                    now,
                )
            }
        })
        .await;

    forward_task.abort();

    match connect_result {
        Ok(report) => report,
        Err(e) => Err(AvpError::Agent(format!(
            "review agent connection failed: {e:?}"
        ))),
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

/// Read a text file the agent requested, resolved under `repo_root`, honoring the
/// optional 1-based `line` start and `limit` line count.
///
/// An absolute path is read as-is; a relative path is joined onto `repo_root`.
/// Returns the (possibly sliced) file content, or an error string when the file
/// cannot be read.
fn read_text_file_under_repo(
    repo_root: &Path,
    req: &agent_client_protocol::schema::ReadTextFileRequest,
) -> Result<String, String> {
    let path = if req.path.is_absolute() {
        req.path.clone()
    } else {
        repo_root.join(&req.path)
    };

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
        now,
    )
    .await;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use agent_client_protocol::schema::{
        ContentBlock, ContentChunk, InitializeResponse, NewSessionResponse, PromptRequest,
        PromptResponse, SessionNotification, SessionUpdate, TextContent,
    };
    use agent_client_protocol::{ConnectTo, ConnectionTo, Role};
    use model_embedding::mock::MockEmbedder;
    use rusqlite::Connection;
    use tempfile::TempDir;

    use swissarmyhammer_code_context::db::{configure_connection, create_schema};
    use swissarmyhammer_code_context::serialize_embedding;

    use crate::review::scope::Scope;
    use crate::validators::types::{RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch};
    use crate::validators::{Rule, Severity, ValidatorLoader, ValidatorSource};

    const DIM: usize = 4;

    // ---- git repo fixture (libgit2, real refs) ---------------------------

    struct TestRepo {
        dir: TempDir,
        repo: git2::Repository,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let repo = git2::Repository::init(dir.path()).unwrap();
            {
                let mut cfg = repo.config().unwrap();
                cfg.set_str("user.name", "Test").unwrap();
                cfg.set_str("user.email", "test@example.com").unwrap();
            }
            Self { dir, repo }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write(&self, rel: &str, content: &str) {
            let full = self.dir.path().join(rel);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(full, content).unwrap();
        }

        fn commit(&self, message: &str) -> String {
            let mut index = self.repo.index().unwrap();
            index
                .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = self.repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit> = parent.iter().collect();
            let oid = self
                .repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
                .unwrap();
            oid.to_string()
        }
    }

    // ---- code_context index fixture --------------------------------------

    fn index_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    fn seed_file(conn: &Connection, file_path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
             VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
            rusqlite::params![file_path],
        )
        .unwrap();
    }

    fn seed_chunk(conn: &Connection, file_path: &str, symbol_path: &str, text: &str, emb: &[f32]) {
        seed_file(conn, file_path);
        let blob = serialize_embedding(emb);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
            rusqlite::params![file_path, text.len() as i64, symbol_path, text, blob],
        )
        .unwrap();
    }

    fn body(label: &str) -> String {
        format!(
            "pub fn {label}(input: &[f64]) -> f64 {{\n    let mut total = 0.0;\n    for value in input {{\n        total += value * value;\n    }}\n    total / input.len() as f64\n}}"
        )
    }

    // ---- validator loader fixture ----------------------------------------

    fn loader_with(name: &str, file_glob: &str, probes: &[&str]) -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        loader.add_builtin_ruleset(ruleset(name, file_glob, probes));
        loader
    }

    fn ruleset(name: &str, file_glob: &str, probes: &[&str]) -> RuleSet {
        RuleSet {
            manifest: RuleSetManifest {
                name: name.to_string(),
                description: format!("{name} test ruleset"),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                match_criteria: Some(ValidatorMatch {
                    tools: vec![],
                    files: vec![file_glob.to_string()],
                }),
                trigger_matcher: None,
                tags: vec![],
                probes: probes.iter().map(|p| p.to_string()).collect(),
                severity: Severity::Error,
                timeout: 30,
                once: false,
            },
            rules: vec![Rule {
                name: format!("{name}-rule"),
                description: "rule".to_string(),
                body: "body".to_string(),
                severity: None,
                timeout: None,
            }],
            source: ValidatorSource::Builtin,
            base_path: std::path::PathBuf::from("/test"),
        }
    }

    // ---- scripted ACP agent (substring → response) -----------------------
    //
    // The scripted agent emits its streamed reply onto a `broadcast::Sender`
    // exactly the way a real backend (Claude/Llama) does: the backend publishes
    // every `session/update` onto its broadcast channel, and the driver consumes
    // a `resubscribe()` of that channel as `notification_rx`. The test passes
    // `notify_tx.subscribe()` as `notification_rx`, so the driver collects from
    // the same authoritative stream production does.
    //
    // When `bridge_to_connection` is set, the agent ALSO re-emits the same
    // notification over the live connection (`cx.send_notification`), reproducing
    // the real-handle shape where `wrap_claude_into_handle`'s
    // `forward_session_notifications` bridges the backend broadcast onto the
    // connection. The driver must NOT collect that re-emission a second time;
    // these tests pin that single-path invariant.

    struct ScriptedAgent {
        next_session: AtomicUsize,
        script: Vec<(String, String)>,
        /// Backend broadcast the agent streams its reply onto — the same channel
        /// the driver's `notification_rx` is a `subscribe()` of.
        notify_tx: broadcast::Sender<SessionNotification>,
        /// Whether to additionally re-emit each reply over the live connection,
        /// reproducing a real handle's broadcast→connection bridge.
        bridge_to_connection: bool,
        /// Whether the `prompt` handler issues a mid-turn `session/request_permission`
        /// request back to the client and blocks on its response before returning
        /// `end_turn` — exactly as a real `claude` agent does for tool consent. This
        /// reproduces the agent↔client deadlock: a client that registers no
        /// `on_receive_request` handler never answers, so the prompt never returns.
        demand_permission: bool,
    }

    impl ScriptedAgent {
        fn new(
            script: Vec<(String, String)>,
            notify_tx: broadcast::Sender<SessionNotification>,
            bridge_to_connection: bool,
        ) -> Arc<Self> {
            Arc::new(Self {
                next_session: AtomicUsize::new(0),
                script,
                notify_tx,
                bridge_to_connection,
                demand_permission: false,
            })
        }

        /// Like [`ScriptedAgent::new`] but the `prompt` handler issues a mid-turn
        /// `session/request_permission` round-trip to the client and only returns
        /// `end_turn` once the client answers (see [`demand_permission`]).
        fn new_demanding(
            script: Vec<(String, String)>,
            notify_tx: broadcast::Sender<SessionNotification>,
        ) -> Arc<Self> {
            Arc::new(Self {
                next_session: AtomicUsize::new(0),
                script,
                notify_tx,
                bridge_to_connection: false,
                demand_permission: true,
            })
        }

        fn response_for(&self, prompt: &str) -> String {
            for (needle, response) in &self.script {
                if prompt.contains(needle) {
                    return response.clone();
                }
            }
            "[]".to_string()
        }
    }

    struct ScriptedAdapter(Arc<ScriptedAgent>);

    impl ConnectTo<Client> for ScriptedAdapter {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            agent_client_protocol::Agent
                .builder()
                .name("scripted-agent")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |_n: agent_client_protocol::ClientNotification, _cx| Ok(()),
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    /// A scripted agent whose `prompt` issues an `fs/read_text_file` request to
    /// the client for `read_path` and records the content the client returns
    /// (`observed_content`) before streaming its reply. Used to prove the client's
    /// read handler serves real on-disk content under `repo_path`.
    struct FsReadingAgent {
        next_session: AtomicUsize,
        script: Vec<(String, String)>,
        notify_tx: broadcast::Sender<SessionNotification>,
        read_path: std::path::PathBuf,
        observed_content: Arc<std::sync::Mutex<Option<String>>>,
    }

    impl FsReadingAgent {
        fn response_for(&self, prompt: &str) -> String {
            for (needle, response) in &self.script {
                if prompt.contains(needle) {
                    return response.clone();
                }
            }
            "[]".to_string()
        }
    }

    struct FsReadingAdapter(Arc<FsReadingAgent>);

    impl ConnectTo<Client> for FsReadingAdapter {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            agent_client_protocol::Agent
                .builder()
                .name("fs-reading-agent")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch_fs_reading(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |_n: agent_client_protocol::ClientNotification, _cx| Ok(()),
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    fn dispatch_fs_reading(
        mock: &Arc<FsReadingAgent>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        let cx = cx.clone();
        cx.clone().spawn(async move {
            match request {
                Req::InitializeRequest(_) => responder
                    .cast()
                    .respond_with_result(Ok(InitializeResponse::new(1.into()))),
                Req::NewSessionRequest(_req) => {
                    let n = mock.next_session.fetch_add(1, Ordering::SeqCst);
                    let id = agent_client_protocol::schema::SessionId::new(format!("sess-{n}"));
                    responder
                        .cast()
                        .respond_with_result(Ok(NewSessionResponse::new(id)))
                }
                Req::PromptRequest(req) => {
                    use agent_client_protocol::schema::ReadTextFileRequest;
                    let read_request =
                        ReadTextFileRequest::new(req.session_id.clone(), mock.read_path.clone());
                    if let Ok(resp) = cx.send_request(read_request).block_task().await {
                        *mock.observed_content.lock().unwrap() = Some(resp.content);
                    }

                    let prompt = prompt_text(&req);
                    let text = mock.response_for(&prompt);
                    let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                        ContentBlock::Text(TextContent::new(text)),
                    ));
                    let notif = SessionNotification::new(req.session_id.clone(), update);
                    let _ = mock.notify_tx.send(notif);
                    responder.cast().respond_with_result(Ok(PromptResponse::new(
                        agent_client_protocol::schema::StopReason::EndTurn,
                    )))
                }
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    fn dispatch(
        mock: &Arc<ScriptedAgent>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        let cx = cx.clone();
        cx.clone().spawn(async move {
            match request {
                Req::InitializeRequest(_) => responder
                    .cast()
                    .respond_with_result(Ok(InitializeResponse::new(1.into()))),
                Req::NewSessionRequest(_req) => {
                    let n = mock.next_session.fetch_add(1, Ordering::SeqCst);
                    let id = agent_client_protocol::schema::SessionId::new(format!("sess-{n}"));
                    responder
                        .cast()
                        .respond_with_result(Ok(NewSessionResponse::new(id)))
                }
                Req::PromptRequest(req) => {
                    // Mid-turn, a real claude agent asks the client for tool
                    // consent via `session/request_permission` and blocks on the
                    // answer before finishing the turn. Model that here: send the
                    // request to the client and `.block_task().await` it. If the
                    // client registers no `on_receive_request` handler, this never
                    // returns and the whole prompt deadlocks — the production hang.
                    if mock.demand_permission {
                        use agent_client_protocol::schema::{
                            RequestPermissionRequest, ToolCallUpdate, ToolCallUpdateFields,
                        };
                        let tool_call_update = ToolCallUpdate::new(
                            agent_client_protocol::schema::ToolCallId::new("tool-read"),
                            ToolCallUpdateFields::new(),
                        );
                        let permission_request = RequestPermissionRequest::new(
                            req.session_id.clone(),
                            tool_call_update,
                            vec![],
                        );
                        if cx
                            .send_request(permission_request)
                            .block_task()
                            .await
                            .is_err()
                        {
                            return responder.cast::<serde_json::Value>().respond_with_error(
                                agent_client_protocol::Error::internal_error(),
                            );
                        }
                    }

                    let prompt = prompt_text(&req);
                    let text = mock.response_for(&prompt);
                    let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                        ContentBlock::Text(TextContent::new(text)),
                    ));
                    let notif = SessionNotification::new(req.session_id.clone(), update);
                    // Publish onto the backend broadcast — the driver's
                    // `notification_rx` is a `subscribe()` of this channel, so this
                    // is the authoritative stream the pool collects from.
                    let _ = mock.notify_tx.send(notif.clone());
                    // Optionally also re-emit over the connection, mirroring the
                    // real handle's broadcast→connection bridge. The driver must
                    // ignore this second copy.
                    if mock.bridge_to_connection {
                        let _ = cx.send_notification(notif);
                    }
                    responder.cast().respond_with_result(Ok(PromptResponse::new(
                        agent_client_protocol::schema::StopReason::EndTurn,
                    )))
                }
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    fn prompt_text(req: &PromptRequest) -> String {
        req.prompt
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// A findings array as an agent emits it, fenced in prose. The verify pass
    /// re-prompts the same agent; the verify prompt mentions the validator and
    /// the claim, so the same scripted response (a confirming verdict) is keyed
    /// off the claim text.
    fn findings_json(file: &str, severity: &str, claim: &str) -> String {
        format!(
            "```json\n[{{\"file\":\"{file}\",\"line\":1,\"validator\":\"agent-tagged\",\
             \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
             \"evidence\":\"per `duplicates`: 0.99\",\"suggestion\":\"extract a helper\"}}]\n```"
        )
    }

    /// A confirming verify verdict (the verify stage asks the agent to confirm
    /// or refute; `confirmed:true` keeps the finding).
    fn confirm_json() -> String {
        "```json\n{\"confirmed\": true, \"reason\": \"the duplicate is real\"}\n```".to_string()
    }

    // ---- the test: drive `review working` end to end ---------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_working_drives_the_pipeline_over_a_scripted_agent() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

        let conn = index_conn();
        let dup_emb = vec![1.0_f32, 0.0, 0.0, 0.0];
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &dup_emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &dup_emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

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
        let (notify_tx, notification_rx) = broadcast::channel(64);
        let agent = ScriptedAgent::new(
            vec![
                (
                    "# Validator: deduplicate".to_string(),
                    findings_json("src/lib.rs", "blocker", "compute duplicates old_compute"),
                ),
                ("compute duplicates old_compute".to_string(), confirm_json()),
            ],
            notify_tx,
            true,
        );

        let dyn_agent = DynConnectTo::new(ScriptedAdapter(agent));

        let report = run_review_over_agent(
            dyn_agent,
            notification_rx,
            Scope::Working,
            repo.path(),
            &loader,
            &conn,
            &embedder,
            PoolConfig::remote(2),
            FleetConfig::default(),
            "2026-06-05 12:00",
        )
        .await;

        let report = report.expect("pipeline should produce a report");
        assert!(
            report
                .markdown
                .contains("## Review Findings (2026-06-05 12:00)"),
            "report header must render: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("### Blockers"),
            "the confirmed blocker finding must be rendered: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("src/lib.rs:1"),
            "the finding's file:line must appear: {}",
            report.markdown
        );
        assert_eq!(report.counts.blockers, 1);
        assert_eq!(report.counts.confirmed, 1);
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
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

        let conn = index_conn();
        let dup_emb = vec![1.0_f32, 0.0, 0.0, 0.0];
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &dup_emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &dup_emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let (notify_tx, notification_rx) = broadcast::channel(64);
        // Every prompt this agent serves blocks on a `session/request_permission`
        // round-trip to the client first — both the fan-out prompt and the verify
        // prompt.
        let agent = ScriptedAgent::new_demanding(
            vec![
                (
                    "# Validator: deduplicate".to_string(),
                    findings_json("src/lib.rs", "blocker", "compute duplicates old_compute"),
                ),
                ("compute duplicates old_compute".to_string(), confirm_json()),
            ],
            notify_tx,
        );

        let dyn_agent = DynConnectTo::new(ScriptedAdapter(agent));

        let report = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            run_review_over_agent(
                dyn_agent,
                notification_rx,
                Scope::Working,
                repo.path(),
                &loader,
                &conn,
                &embedder,
                PoolConfig::remote(2),
                FleetConfig::default(),
                "2026-06-05 12:00",
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
            report.markdown.contains("### Blockers"),
            "the confirmed blocker finding must be rendered after the permission round-trips: {}",
            report.markdown
        );
        assert_eq!(report.counts.blockers, 1);
        assert_eq!(report.counts.confirmed, 1);
    }

    /// Companion to the deadlock reproduction: the agent demands an
    /// `fs/read_text_file` round-trip mid-prompt, and the client must serve the
    /// read from disk under `repo_path`. Proves the read handler returns the real
    /// file content (not just that the request is answered).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn review_serves_fs_read_text_file_from_disk_under_repo_path() {
        let repo = TestRepo::new();
        repo.write("src/lib.rs", "fn placeholder() {}\n");
        repo.commit("initial");
        let dup = body("compute");
        repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

        let conn = index_conn();
        let dup_emb = vec![1.0_f32, 0.0, 0.0, 0.0];
        seed_chunk(&conn, "src/lib.rs", "compute", &dup, &dup_emb);
        seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &dup_emb);

        let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
        let embedder = MockEmbedder::new(DIM);

        let read_path = repo.path().join("src/lib.rs");
        let (notify_tx, notification_rx) = broadcast::channel(64);
        let observed = Arc::new(std::sync::Mutex::new(Option::<String>::None));
        let agent = Arc::new(FsReadingAgent {
            next_session: AtomicUsize::new(0),
            script: vec![
                (
                    "# Validator: deduplicate".to_string(),
                    findings_json("src/lib.rs", "blocker", "compute duplicates old_compute"),
                ),
                ("compute duplicates old_compute".to_string(), confirm_json()),
            ],
            notify_tx,
            read_path: read_path.clone(),
            observed_content: Arc::clone(&observed),
        });

        let dyn_agent = DynConnectTo::new(FsReadingAdapter(agent));

        let report = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            run_review_over_agent(
                dyn_agent,
                notification_rx,
                Scope::Working,
                repo.path(),
                &loader,
                &conn,
                &embedder,
                PoolConfig::remote(2),
                FleetConfig::default(),
                "2026-06-05 12:00",
            ),
        )
        .await
        .expect("the review must serve fs/read_text_file without hanging");

        let _report = report.expect("pipeline should produce a report");
        let content = observed
            .lock()
            .unwrap()
            .clone()
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

    /// Collect a multi-chunk streamed reply through the pool's notifier, exactly
    /// as a pool worker does: subscribe to the notifier's broadcast, reassemble
    /// the streamed text for `session`, and return the collected string.
    async fn collect_through_notifier(
        notifier: &Arc<claude_agent::NotificationSender>,
        session: agent_client_protocol::schema::SessionId,
    ) -> String {
        let (collector, collected_text, notification_count, _matched) =
            claude_agent::spawn_notification_collector(notifier.sender().subscribe(), session);
        let prompt_response = agent_client_protocol::schema::PromptResponse::new(
            agent_client_protocol::schema::StopReason::EndTurn,
        );
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
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn notification_rx_is_the_pools_single_collected_stream() {
        let session = agent_client_protocol::schema::SessionId::new("sess-single".to_string());
        let reply = findings_json("src/lib.rs", "blocker", "compute duplicates old_compute");
        let stream = chunked_notifications(&session, &reply, 6);

        // --- (1) the driver's actual single-feed path collects the reply once ---

        // The backend broadcast: `notification_rx` is a `subscribe()` of it, just
        // as `wrap_claude_into_handle` resubscribes the agent's channel.
        let (notify_tx, notification_rx) = broadcast::channel::<SessionNotification>(256);
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

        // --- (2) the old dual-feed shape doubles the same stream ---------------
        //
        // Reproduce the bug: TWO forwarders draining the SAME backend broadcast
        // (one standing in for `notification_rx`, one for the connection
        // re-emission) both copy into one notifier. Every chunk lands twice, so
        // the collected text is twice as long for any interleaving — which is
        // precisely what corrupted the JSON the verify/fleet parser reads.
        let (dual_tx, dual_rx_a) = broadcast::channel::<SessionNotification>(256);
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
            "a dual feed doubles every chunk, doubling the collected length and \
             corrupting the JSON; the single-feed driver avoids this"
        );
    }
}
