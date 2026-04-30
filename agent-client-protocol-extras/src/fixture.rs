//! Fixture-recording entry points for ACP conformance tests.
//!
//! The ACP 0.10 conformance test suite picked between a real recorded agent
//! (`RecordingAgent`) and a stubbed-out replay (`PlaybackAgent`) through a
//! single `Box<dyn AgentWithFixture>` factory pattern. Each per-agent factory
//! looked up `<workspace>/.fixtures/<agent_type>/<test_name>.json` — if the
//! file existed it returned a `PlaybackAgent`, otherwise it returned a
//! `RecordingAgent` wrapping the real agent so the next run would have a
//! fixture.
//!
//! This module rebuilds that scaffolding for ACP 0.11. The trait and helpers
//! are deliberately small: a test driver constructs one of the concrete
//! wrappers through a factory, calls [`AgentWithFixture::connection`] to get
//! a [`ConnectionTo<Agent>`] handle, and drives `send_request(...)` against
//! it exactly the way an integration test would talk to a real agent.
//! Dropping the wrapper closes the underlying duplex transport, which lets
//! the inner `connect_to` future resolve and (for recording) flush the
//! fixture to disk.
//!
//! ## Trait
//!
//! [`AgentWithFixture`] is the dyn-compatible facade the conformance crate
//! consumes. It exposes the agent's static identifier
//! ([`AgentWithFixture::agent_type`]) and a cheaply-cloneable connection
//! handle ([`AgentWithFixture::connection`]).
//!
//! ## Concrete wrappers
//!
//! - [`PlaybackAgentWithFixture`] wraps a [`crate::PlaybackAgent`]. It is the
//!   "fixture exists, replay it" branch of the factory.
//! - [`RecordingAgentWithFixture`] wraps a [`crate::RecordingAgent`] around
//!   an inner agent that implements [`ConnectTo<Client>`]. It is the "no
//!   fixture yet, record one" branch.
//!
//! Both wrappers internally:
//! 1. Create a [`Channel::duplex`] pair so the inner agent has somewhere to
//!    serve from.
//! 2. Spawn the inner agent's `connect_to` future on one end of the duplex.
//! 3. Run [`Client.builder().connect_with`](agent_client_protocol::Client) on
//!    the other end and stash the resulting `ConnectionTo<Agent>` on the
//!    wrapper.
//! 4. Hold the spawned tasks on the wrapper. Dropping the wrapper aborts the
//!    client task, which closes its end of the duplex and lets the inner
//!    `connect_to` future shut down (and, for the recording case, flush the
//!    fixture).
//!
//! ## Path layout
//!
//! [`get_fixture_path_for`] returns
//! `<workspace>/.fixtures/<agent_type>/<test_name>.json` relative to the
//! cargo workspace root. [`get_test_name_from_thread`] reads the current
//! `tokio::test`-flavoured thread name and returns its leaf component (so
//! `tests::integration::initialization::test_minimal_initialization` becomes
//! `test_minimal_initialization`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_client_protocol::{Agent, Channel, Client, ConnectTo, ConnectionTo};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::recording::{RecordingState, SourceHandle};
use crate::{PlaybackAgent, RecordingAgent};

// ---------------------------------------------------------------------------
// AgentWithFixture trait
// ---------------------------------------------------------------------------

/// Dyn-compatible facade over an ACP 0.11 connection that points at either a
/// recorded fixture or a live recording wrap.
///
/// In ACP 0.10 this trait extended `Agent` and exposed an associated
/// `agent_type()` constant. ACP 0.11 has no `Agent` trait, so the new shape
/// owns the connection itself. Callers obtain a [`ConnectionTo<Agent>`] from
/// [`Self::connection`] and drive ACP requests through it like any other
/// in-process client.
///
/// The connection handle is *cheaply cloneable* — every call to `connection`
/// returns a fresh clone that shares the underlying message-routing actors
/// with the wrapper. The wrapper owns the lifetime of the actors; dropping
/// it tears the connection down.
///
/// Implementations must remain `Send + Sync` so the conformance suite can
/// box them through `Box<dyn AgentWithFixture>`.
pub trait AgentWithFixture: Send + Sync {
    /// Static identifier used in fixture paths and log lines (e.g.
    /// `"claude"`, `"llama"`, `"test"`).
    fn agent_type(&self) -> &'static str;

    /// Cheaply-cloneable handle for sending ACP requests to the wrapped
    /// agent. Returned by clone, not by reference, so callers can move the
    /// handle into spawned tasks without juggling lifetimes.
    fn connection(&self) -> ConnectionTo<Agent>;
}

// ---------------------------------------------------------------------------
// Path / test-name helpers
// ---------------------------------------------------------------------------

/// Return `<workspace>/.fixtures/<agent_type>/<test_name>.json`.
///
/// The workspace root is detected via [`workspace_root`]. Parent directories
/// are *not* created — callers that need them should call
/// [`std::fs::create_dir_all`] on `path.parent()` before writing.
///
/// # Arguments
/// * `agent_type` - Static label like `"claude"` or `"llama"`. Becomes a
///   subdirectory under `.fixtures/`.
/// * `test_name` - Leaf test name like `"test_minimal_initialization"`. Used
///   verbatim as the file stem.
pub fn get_fixture_path_for(agent_type: &str, test_name: &str) -> PathBuf {
    workspace_root()
        .join(".fixtures")
        .join(agent_type)
        .join(format!("{test_name}.json"))
}

/// Return the leaf component of the current thread's name.
///
/// `tokio::test`-flavoured threads inherit the test function's fully
/// qualified path as the thread name (e.g.
/// `integration::initialization::test_minimal_initialization`). The fixture
/// pattern only cares about the leaf — the part after the final `::`. When
/// the thread has no name (e.g. running in a custom executor), the literal
/// string `"unknown"` is returned so callers don't have to handle a missing
/// value.
pub fn get_test_name_from_thread() -> String {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unknown");
    name.rsplit("::").next().unwrap_or(name).to_string()
}

/// Resolve the workspace root for fixture paths.
///
/// Tries, in order:
/// 1. The directory containing a top-level `Cargo.toml` discovered by walking
///    up from `CARGO_MANIFEST_DIR`. This is the common case during
///    `cargo test` / `cargo nextest`.
/// 2. The current working directory as a final fallback.
///
/// The function never panics — a missing workspace root degrades to a
/// relative `.fixtures/` directory under cwd, matching the legacy 0.10
/// behaviour.
fn workspace_root() -> PathBuf {
    if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let start = PathBuf::from(manifest_dir);
        if let Some(root) = walk_up_for_workspace(&start) {
            return root;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Walk up from `start` looking for a `Cargo.toml` whose top-level table
/// contains a `[workspace]` key. Returns the first such directory found.
fn walk_up_for_workspace(start: &Path) -> Option<PathBuf> {
    let mut dir: &Path = start;
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() && is_workspace_manifest(&candidate) {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Cheaply detect a workspace `Cargo.toml`. We don't need full TOML parsing —
/// the unique `[workspace]` table heading is enough and avoids pulling in
/// `toml` as a dependency just for this check.
fn is_workspace_manifest(path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    contents
        .lines()
        .any(|line| line.trim_start().starts_with("[workspace]"))
}

// ---------------------------------------------------------------------------
// Shared driver — runs an inner ConnectTo<Client> on a duplex and gives back
// a ConnectionTo<Agent> for tests to drive.
// ---------------------------------------------------------------------------

/// Background tasks owned by an `AgentWithFixture` wrapper.
///
/// The wrapper aborts both tasks on drop. Aborting the client task closes
/// its end of the duplex transport, which causes the inner agent's
/// `connect_to` future to wind down — and, for [`RecordingAgentWithFixture`],
/// to flush its fixture to disk.
struct WrapperTasks {
    /// Task running the inner agent's `connect_to` future on the agent end
    /// of the duplex.
    agent_task: JoinHandle<()>,
    /// Task running `Client.builder().connect_with(...)` on the client end.
    client_task: JoinHandle<()>,
    /// Oneshot used to release the `connect_with` body so the client task
    /// can finish cleanly. Sending `()` (or just dropping the sender as part
    /// of the wrapper's drop) lets the body return.
    ///
    /// Wrapped in `Option` so the wrapper's `Drop` impl can take the sender
    /// without needing `&mut self` mutability tricks at the field level.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl Drop for WrapperTasks {
    fn drop(&mut self) {
        // Politely ask the client closure to return. If the channel is
        // already closed (e.g. the closure has already exited), the send
        // is a no-op.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Aborting the client task closes its end of the duplex, which ends
        // the inner `connect_to` future. The agent task is similarly aborted
        // to ensure no zombies survive teardown.
        self.client_task.abort();
        self.agent_task.abort();
    }
}

/// Wire `agent` (something that knows how to serve clients) to a fresh
/// duplex channel and return both the cheaply-cloneable
/// `ConnectionTo<Agent>` the test will drive *and* the background tasks
/// that keep the connection alive.
///
/// On any wiring failure, the spawned tasks are aborted before the error is
/// returned so we never leak a half-set-up connection.
async fn drive_inner_agent<A>(
    agent: A,
) -> Result<(ConnectionTo<Agent>, WrapperTasks), agent_client_protocol::Error>
where
    A: ConnectTo<Client> + Send + 'static,
{
    let (agent_side, client_side) = Channel::duplex();

    // Spawn the inner agent on its end of the duplex.
    let agent_task = tokio::spawn(async move {
        if let Err(err) = agent.connect_to(agent_side).await {
            tracing::warn!("inner agent connect_to returned error: {}", err);
        }
    });

    // Run the client-side builder. The closure's body parks on
    // `shutdown_rx` until the wrapper is dropped, so the connection actors
    // stay alive while tests hold the connection handle.
    let (conn_tx, conn_rx) = oneshot::channel::<ConnectionTo<Agent>>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let client_task = tokio::spawn(async move {
        let result = Client
            .builder()
            .name("agent-with-fixture-client")
            .connect_with(client_side, async move |conn: ConnectionTo<Agent>| {
                // Hand the cheaply-cloneable connection out to the test.
                // The send may fail if the wrapper was dropped before we got
                // here — in which case we just let the closure return so
                // `connect_with` shuts down cleanly.
                let _ = conn_tx.send(conn);
                // Park until the wrapper's drop fires. A closed channel
                // (sender dropped without sending) and a successful send are
                // both treated as "shut down".
                let _ = shutdown_rx.await;
                Ok(())
            })
            .await;
        if let Err(err) = result {
            tracing::warn!("agent-with-fixture client connect_with error: {}", err);
        }
    });

    // Wait for the client closure to hand us the connection. If it never
    // arrives (e.g. the agent task panicked synchronously), abort
    // everything and surface an error rather than hanging the test forever.
    let connection = match conn_rx.await {
        Ok(conn) => conn,
        Err(_) => {
            agent_task.abort();
            client_task.abort();
            return Err(agent_client_protocol::util::internal_error(
                "agent-with-fixture: client never produced a ConnectionTo<Agent>",
            ));
        }
    };

    Ok((
        connection,
        WrapperTasks {
            agent_task,
            client_task,
            shutdown_tx: Some(shutdown_tx),
        },
    ))
}

// ---------------------------------------------------------------------------
// PlaybackAgentWithFixture
// ---------------------------------------------------------------------------

/// `AgentWithFixture` impl backed by a [`PlaybackAgent`].
///
/// Used by the conformance suite when a recorded fixture exists at
/// `<workspace>/.fixtures/<agent_type>/<test_name>.json`. The wrapper owns
/// the running playback dispatch so tests can drive
/// `connection().send_request(...)` without thinking about transport
/// plumbing.
pub struct PlaybackAgentWithFixture {
    agent_type: &'static str,
    connection: ConnectionTo<Agent>,
    /// Background tasks. Held only for their `Drop` side-effect — when the
    /// wrapper is dropped, the tasks are aborted and the duplex closes.
    _tasks: WrapperTasks,
}

impl PlaybackAgentWithFixture {
    /// Build a `PlaybackAgentWithFixture` from a [`PlaybackAgent`].
    ///
    /// Spawns the playback dispatch loop and stands up the client side of an
    /// in-process duplex transport. The returned wrapper is ready for tests
    /// to call [`AgentWithFixture::connection`] on.
    ///
    /// # Errors
    /// Returns the underlying `agent_client_protocol::Error` if the SDK
    /// could not stand up the in-process connection (extremely rare —
    /// typically only happens if a runtime is missing).
    pub async fn new(agent: PlaybackAgent) -> Result<Self, agent_client_protocol::Error> {
        let agent_type = agent.agent_type();
        let (connection, tasks) = drive_inner_agent(agent).await?;
        Ok(Self {
            agent_type,
            connection,
            _tasks: tasks,
        })
    }

    /// Convenience: load a fixture from disk and wrap it in one call.
    ///
    /// Equivalent to `Self::new(PlaybackAgent::new(path, agent_type)).await`.
    pub async fn from_fixture(
        path: PathBuf,
        agent_type: &'static str,
    ) -> Result<Self, agent_client_protocol::Error> {
        Self::new(PlaybackAgent::new(path, agent_type)).await
    }
}

impl AgentWithFixture for PlaybackAgentWithFixture {
    fn agent_type(&self) -> &'static str {
        self.agent_type
    }

    fn connection(&self) -> ConnectionTo<Agent> {
        self.connection.clone()
    }
}

// ---------------------------------------------------------------------------
// RecordingAgentWithFixture
// ---------------------------------------------------------------------------

/// `AgentWithFixture` impl backed by a [`RecordingAgent`] wrapped around an
/// inner ACP 0.11 component that already implements [`ConnectTo<Client>`].
///
/// Used by the conformance suite when no fixture exists yet — the wrapper
/// records every JSON-RPC message flowing through to the inner agent and
/// flushes the resulting `RecordedSession` to disk on drop. Additional
/// notification sources can be folded in via
/// [`Self::add_mcp_source`] (and the constructor's optional `session`
/// notification rx).
///
/// # Type-erasure
///
/// The wrapper hides the inner agent's concrete type behind the duplex
/// transport, so the public API has no `<A>` parameter. Production agents
/// (`ClaudeAgent`, `AcpServer`, ...) that don't implement `ConnectTo<Client>`
/// directly need a thin per-agent adapter; that adapter lives in the
/// conformance crate (or the per-agent crate) rather than here, because the
/// adapter shape varies with the agent's internal dispatch model.
pub struct RecordingAgentWithFixture {
    agent_type: &'static str,
    connection: ConnectionTo<Agent>,
    /// Recording state shared with the inner `RecordingAgent`'s copy loops.
    /// Held here so [`Self::add_mcp_source`] can register additional
    /// notification drains against the same in-memory recording buffer.
    state: Arc<RecordingState>,
    /// Drainage handles for any extra notification sources (session-side
    /// channel, MCP proxies, ...). Held in a `Mutex` so `add_mcp_source`
    /// can append to it through `&self`.
    extra_sources: Mutex<Vec<SourceHandle>>,
    _tasks: WrapperTasks,
}

impl RecordingAgentWithFixture {
    /// Wrap an inner `ConnectTo<Client>` agent in a [`RecordingAgent`] and
    /// stand up a fixture-recording connection.
    ///
    /// The recording is written to `path` on drop. The `agent_type` is the
    /// static identifier ([`AgentWithFixture::agent_type`]) used in fixture
    /// paths and log lines.
    pub async fn new<A>(
        inner: A,
        path: PathBuf,
        agent_type: &'static str,
    ) -> Result<Self, agent_client_protocol::Error>
    where
        A: ConnectTo<Client> + Send + 'static,
    {
        let state = Arc::new(RecordingState::new(path.clone()));
        let recorder = RecordingAgent::with_state(inner, Arc::clone(&state));
        let (connection, tasks) = drive_inner_agent(recorder).await?;
        Ok(Self {
            agent_type,
            connection,
            state,
            extra_sources: Mutex::new(Vec::new()),
            _tasks: tasks,
        })
    }

    /// Wrap an inner `ConnectTo<Client>` agent and feed an additional
    /// [`SessionNotification`] side channel into the recorded fixture.
    ///
    /// Used by the conformance suite when the inner agent broadcasts
    /// `SessionNotification`s through a `tokio::sync::broadcast` side channel
    /// in addition to (or instead of) the JSON-RPC duplex. The drain runs
    /// until the receiver is closed; notifications it observes are routed
    /// into the recorded fixture exactly the same way wire-side
    /// `session/update` notifications are.
    ///
    /// [`SessionNotification`]: agent_client_protocol::schema::SessionNotification
    pub async fn with_notifications<A>(
        inner: A,
        path: PathBuf,
        agent_type: &'static str,
        notifications: tokio::sync::broadcast::Receiver<
            agent_client_protocol::schema::SessionNotification,
        >,
    ) -> Result<Self, agent_client_protocol::Error>
    where
        A: ConnectTo<Client> + Send + 'static,
    {
        let wrapper = Self::new(inner, path, agent_type).await?;
        let handle = crate::recording::spawn_session_notification_drain(
            Arc::clone(&wrapper.state),
            notifications,
        );
        wrapper.extra_sources.lock().unwrap().push(handle);
        Ok(wrapper)
    }

    /// Register an additional MCP notification source whose notifications
    /// should be folded into the recorded fixture.
    ///
    /// The drain runs until the broadcast receiver is closed. Captured MCP
    /// notifications are encoded as JSON values and routed into the
    /// recorded fixture as additional notification entries on the most
    /// recent prompt call.
    pub fn add_mcp_source(
        &self,
        rx: tokio::sync::broadcast::Receiver<model_context_protocol_extras::McpNotification>,
    ) {
        let handle = crate::recording::spawn_mcp_drain(Arc::clone(&self.state), rx);
        self.extra_sources.lock().unwrap().push(handle);
    }
}

impl AgentWithFixture for RecordingAgentWithFixture {
    fn agent_type(&self) -> &'static str {
        self.agent_type
    }

    fn connection(&self) -> ConnectionTo<Agent> {
        self.connection.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_fixture_path_for_assembles_components() {
        let path = get_fixture_path_for("claude", "test_minimal_initialization");
        // Last three components must be
        // `.fixtures/claude/test_minimal_initialization.json`, independent of
        // where the workspace root lands during test runs.
        let mut components: Vec<&std::ffi::OsStr> =
            path.components().map(|c| c.as_os_str()).collect();
        let leaf = components.pop().unwrap();
        let agent_dir = components.pop().unwrap();
        let fixtures_dir = components.pop().unwrap();
        assert_eq!(
            leaf,
            std::ffi::OsStr::new("test_minimal_initialization.json")
        );
        assert_eq!(agent_dir, std::ffi::OsStr::new("claude"));
        assert_eq!(fixtures_dir, std::ffi::OsStr::new(".fixtures"));
    }

    #[test]
    fn get_fixture_path_for_uses_agent_type_as_subdir() {
        let claude = get_fixture_path_for("claude", "x");
        let llama = get_fixture_path_for("llama", "x");
        assert_ne!(claude, llama);
    }

    #[test]
    fn get_test_name_returns_leaf_component() {
        // We can't easily set the current thread's name from a sync context
        // without spawning a new thread. Spawn one with a known name and
        // assert the leaf-extraction logic on the inside.
        let handle = std::thread::Builder::new()
            .name("integration::initialization::test_minimal_initialization".to_string())
            .spawn(get_test_name_from_thread)
            .expect("spawn named thread");
        let leaf = handle.join().expect("thread join");
        assert_eq!(leaf, "test_minimal_initialization");
    }

    #[test]
    fn get_test_name_handles_unnamed_thread() {
        // Threads spawned without an explicit name fall back to "unknown".
        let handle = std::thread::Builder::new()
            .spawn(get_test_name_from_thread)
            .expect("spawn anonymous thread");
        let name = handle.join().expect("thread join");
        // Unnamed std threads default to no name; we map that to "unknown".
        // Some platforms / executors do supply a synthetic name, so we accept
        // either "unknown" or a name without "::" separators.
        assert!(
            name == "unknown" || !name.contains("::"),
            "expected leaf-style name, got {name:?}"
        );
    }

    #[test]
    fn get_test_name_handles_name_without_separator() {
        let handle = std::thread::Builder::new()
            .name("flat_name".to_string())
            .spawn(get_test_name_from_thread)
            .expect("spawn named thread");
        let name = handle.join().expect("thread join");
        assert_eq!(name, "flat_name");
    }

    #[test]
    fn workspace_root_resolves_to_repo_root() {
        let root = workspace_root();
        // The repo root must contain a Cargo.toml with [workspace] in it.
        let manifest = root.join("Cargo.toml");
        assert!(
            manifest.exists(),
            "workspace_root must point at a directory containing Cargo.toml: {root:?}"
        );
        assert!(
            is_workspace_manifest(&manifest),
            "Cargo.toml at {manifest:?} should be a workspace manifest"
        );
    }

    #[test]
    fn is_workspace_manifest_recognises_workspace_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, "[package]\nname=\"x\"\n").unwrap();
        assert!(!is_workspace_manifest(&path));

        std::fs::write(&path, "[workspace]\nmembers = []\n").unwrap();
        assert!(is_workspace_manifest(&path));
    }

    // ---- PlaybackAgentWithFixture end-to-end ------------------------------

    #[tokio::test]
    async fn playback_with_fixture_roundtrips_initialize_via_connection() {
        // Build a tiny on-disk fixture and load it through PlaybackAgent.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("playback.json");
        let session = serde_json::json!({
            "calls": [
                {
                    "method": "initialize",
                    "request": {"protocolVersion": 1},
                    "response": {
                        "protocolVersion": 1,
                        "agentCapabilities": {},
                        "authMethods": []
                    },
                    "notifications": []
                },
                {
                    "method": "new_session",
                    "request": {"cwd": "/tmp", "mcpServers": []},
                    "response": {"sessionId": "session-A"},
                    "notifications": []
                }
            ]
        });
        std::fs::write(&path, serde_json::to_string(&session).unwrap()).unwrap();

        let wrapper = PlaybackAgentWithFixture::from_fixture(path, "test")
            .await
            .expect("wrapper constructs");

        assert_eq!(wrapper.agent_type(), "test");

        let conn = wrapper.connection();

        use agent_client_protocol::schema::{InitializeRequest, NewSessionRequest, ProtocolVersion};

        let init_resp = conn
            .send_request(InitializeRequest::new(ProtocolVersion::V1))
            .block_task()
            .await
            .expect("initialize response");
        assert_eq!(init_resp.protocol_version, ProtocolVersion::V1);

        // Call 2: new_session. The cursor should now be on the second
        // recorded call.
        let new_session_resp = conn
            .send_request(NewSessionRequest::new(std::path::PathBuf::from("/tmp")))
            .block_task()
            .await
            .expect("new_session response");
        assert_eq!(new_session_resp.session_id.0.as_ref(), "session-A");

        // Drop the wrapper — the duplex tears down and tasks abort cleanly.
        drop(wrapper);
    }
}
