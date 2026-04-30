//! AVP Context - Manages the AVP directory and agent access.
//!
//! The AVP directory (configured via `AvpConfig::DIR_NAME`) is created at the
//! git repository root and contains:
//! - `validators/` - Project-specific validators
//! - `.gitignore` - Excludes log files from version control
//!
//! Logging is handled by `tracing` — the CLI sets up a file layer that writes
//! to `.avp/log` at info level so all tracing output from every crate
//! (agents, validators, hooks) flows into the log automatically.
//!
//! User-level validators can be placed in `$XDG_DATA_HOME/avp/validators/ (defaults to ~/.local/share/avp/validators/)`.
//!
//! The context also provides access to an ACP Agent for validator execution.
//! In production, this is a ClaudeAgent created lazily. In tests, a PlaybackAgent
//! can be injected via `with_agent()`.
//!
//! ## Recording validator agent sessions
//!
//! Validator agent sessions are always recorded under `.avp/recordings/`;
//! transcripts double as audit trails and as `PlaybackAgent` fixtures for the
//! integration tests. The validator agent is transparently wrapped with
//! [`agent_client_protocol_extras::RecordingAgent`] before being handed to the
//! runner. Every call (`initialize`, `new_session`, `prompt`) and the
//! notifications streamed during those calls are written as a `RecordedSession`
//! JSON file under `<AVP_DIR>/recordings/`. There is no opt-in flag — recording
//! is unconditional.
//!
//! ### Session id resolution
//!
//! The recording filename embeds an AVP-level session id. Production hook
//! entry points should call [`AvpContext::set_session_id`] right after
//! constructing the context (whether via [`AvpContext::init`] or
//! [`AvpContext::with_agent`]) and before the first call to
//! [`AvpContext::agent`]. The recording wrap is applied lazily on first agent
//! use for *both* construction paths, so the session id installed between
//! construction and first use propagates into the recording filename in
//! either case. The `AVP_SESSION_ID` env var is honored as a fallback for
//! tests and scripts that can't easily call the explicit API; when neither
//! is set, the literal string `"no-session"` is used.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use agent_client_protocol::{ConnectTo, ConnectionTo, DynConnectTo};
use agent_client_protocol_extras::{RecordingAgent, RecordingFlushHandle};
use swissarmyhammer_directory::{AvpConfig, DirectoryConfig, ManagedDirectory};
use swissarmyhammer_tools::mcp::unified_server::{
    start_mcp_server_with_options, McpServerHandle, McpServerMode,
};
use tokio::sync::Mutex;

use swissarmyhammer_config::model::{ModelConfig, ModelExecutorType, ModelManager, ModelPaths};

use crate::error::AvpError;
use crate::turn::TurnStateManager;
use crate::types::HookType;
use crate::validator::{ExecutedRuleSet, ExecutedValidator, RuleSet, Validator, ValidatorRunner};

/// Capacity for the broadcast channel used for session notifications.
/// Capacity for notification broadcast channels.
///
/// This needs to be large enough to handle multi-turn validators that may
/// generate many streaming notifications. A 43-turn conversation can easily
/// generate thousands of content deltas.
pub const NOTIFICATION_CHANNEL_CAPACITY: usize = 4096;

/// Result type for directory initialization
type InitDirectoriesResult = (
    ManagedDirectory<AvpConfig>,
    Option<ManagedDirectory<AvpConfig>>,
);

/// Decision outcome for a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Hook allowed the action to proceed.
    Allow,
    /// Hook blocked the action.
    Block,
    /// Hook encountered an error.
    Error,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Block => write!(f, "block"),
            Decision::Error => write!(f, "error"),
        }
    }
}

/// A hook event to log.
#[derive(Debug)]
pub struct HookEvent<'a> {
    /// The hook type (e.g., "PreToolUse", "PostToolUse").
    pub hook_type: &'a str,
    /// The decision outcome.
    pub decision: Decision,
    /// Optional details (tool name, reason, etc.).
    pub details: Option<String>,
}

/// A validator execution event to log.
#[derive(Debug)]
pub struct ValidatorEvent<'a> {
    /// The validator name.
    pub name: &'a str,
    /// Whether the validator passed.
    pub passed: bool,
    /// The validator message.
    pub message: &'a str,
    /// The hook type that triggered this validator.
    pub hook_type: &'a str,
}

/// State of an agent connection that has not yet been "armed" — i.e. the
/// recording wrap and the in-process connection have not yet been built.
///
/// Both the lazy [`AvpContext::init`] path and the eager
/// [`AvpContext::with_agent`]/[`AvpContext::with_agent_and_model`] paths
/// install a `Pending` handle and rely on the deferred-arm mechanism in
/// [`AvpContext::agent`] to materialise the live connection. Deferring the
/// arm is what makes [`AvpContext::set_session_id`] take effect on both
/// construction paths — the recording filename is computed at arm-time
/// (first `agent()` call), not at construction time, so a session id
/// installed between the constructor returning and the first `agent()` call
/// propagates into the recording filename.
///
/// The lazy variant carries no inner agent — it builds one on first use via
/// [`swissarmyhammer_agent::create_agent_with_options`]. The eager variant
/// carries the externally-supplied inner agent component, type-erased into
/// [`DynConnectTo<Client>`] so the eager-path constructors don't have to be
/// generic over the concrete agent type.
///
/// In ACP 0.11 there is no side-channel broadcast for notifications — they
/// flow through the JSON-RPC connection itself, captured by the
/// `on_receive_notification` handler installed during arm. This is why the
/// eager variant carries only the inner agent; no separate notifications
/// receiver is needed.
///
/// [`PlaybackAgent`]: agent_client_protocol_extras::PlaybackAgent
enum PendingAgent {
    /// Lazy path: no inner agent yet. The first call to [`AvpContext::agent`]
    /// will build one from `model_config` via swissarmyhammer-agent.
    Lazy,
    /// Eager path: caller has supplied the inner agent. The first call to
    /// [`AvpContext::agent`] will wrap the inner in [`RecordingAgent`] and
    /// run the connection-establishment dance.
    Eager {
        /// The externally-supplied inner agent component (type-erased so the
        /// eager-path constructors don't have to be generic over the concrete
        /// agent type).
        inner: DynConnectTo<agent_client_protocol::Client>,
    },
}

/// State of an agent connection that has been wired up — recording wrap
/// applied, in-process connection running, client-side handle and per-session
/// notifier ready for use.
struct ActiveAgent {
    /// Client-side handle for sending requests to the wrapped inner agent.
    /// Cheap to clone (it is a shared message-routing handle backed by mpsc
    /// channels — see the SDK's [`ConnectionTo`] docs).
    connection: ConnectionTo<agent_client_protocol::Agent>,
    /// Per-session notification fan-out used by the validator runner.
    notifier: Arc<claude_agent::NotificationSender>,
    /// Synchronous flush handle for the [`RecordingAgent`] wrapping the
    /// validator agent. The wrapper itself is owned by the spawned
    /// [`Self::_task`] (it was moved into `connect_with`), so we cannot
    /// reach into it from a synchronous teardown path. The flush handle
    /// holds an `Arc` clone of the recording state, lets us call
    /// `flush()` from [`Drop`], and ensures the recording reaches disk
    /// before [`Self::_task`]'s `abort()` races teardown — `abort` is a
    /// signal, not a synchronous join, and a parked task may be dropped
    /// strictly *after* the caller observes its side-effects.
    recording_flush: RecordingFlushHandle,
    /// Background task driving `Client.builder().connect_with(...)`.
    ///
    /// Held so the SDK's actor loops keep running for the context's
    /// lifetime; aborted on drop so dropping [`AvpContext`] tears the
    /// connection down. We can't rely on `JoinHandle`'s drop alone — tokio
    /// deliberately keeps detached tasks running — so the abort is wired up
    /// via [`AbortOnDrop`].
    _task: AbortOnDrop,
}

impl Drop for ActiveAgent {
    /// Flush the recording before the connection task is aborted.
    ///
    /// The recording lives inside the spawned task's future (it was moved
    /// into `connect_with`). Aborting that task is asynchronous: it sets a
    /// cancellation flag and the task's drop chain only runs the next time
    /// the runtime polls it. On a single-threaded runtime, a synchronous
    /// caller (e.g. a test that drops [`AvpContext`] then immediately
    /// reads the recordings directory) can race past the abort and observe
    /// no file on disk.
    ///
    /// We close that gap here by explicitly flushing through the held
    /// [`RecordingFlushHandle`] before the field-drop chain reaches
    /// [`Self::_task`] (which performs the abort). Because [`Self::_task`]
    /// is the *last* field declared in the struct, it drops last —
    /// `recording_flush` has already pushed the latest snapshot to disk by
    /// the time the task is signalled.
    fn drop(&mut self) {
        self.recording_flush.flush();
    }
}

/// RAII guard that aborts the wrapped tokio task on drop.
///
/// Used by [`ActiveAgent`] to ensure dropping [`AvpContext`] also stops the
/// background `connect_with` task driving the validator agent connection.
/// Plain [`tokio::task::JoinHandle`] drop merely detaches; `AbortOnDrop`
/// gives us cancel-on-drop semantics without pulling in `tokio_util`.
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Holds the connection-related state for the validator agent.
///
/// The handle starts in [`AgentHandle::Pending`] form (either `Lazy` or
/// `Eager`) and transitions to [`AgentHandle::Active`] on the first call to
/// [`AvpContext::agent`]. This deferred-arming is what lets
/// [`AvpContext::set_session_id`] take effect on both construction paths:
/// the recording filename is computed at arm-time, not at construction time.
enum AgentHandle {
    /// Not yet wired to a live connection.
    Pending(PendingAgent),
    /// Live connection ready to serve validator requests.
    Active(ActiveAgent),
}

/// AVP Context - manages the AVP directory, logging, agent access, turn state, and validator execution.
///
/// All AVP directory logic is centralized here. The directory is created
/// at the git repository root using the shared `swissarmyhammer-directory` crate.
///
/// The context tracks both project-level and user-level directories:
/// - Project: `./<AVP_DIR>/` at git root
/// - User: XDG data directory (e.g., `$XDG_DATA_HOME/avp/` or `~/.local/share/avp/`)
///
/// The context also provides:
/// - Access to an ACP Agent for validator execution (lazy or injected)
/// - Turn state management for tracking file changes across tool calls
/// - Cached validator runner for efficient repeated validation
pub struct AvpContext {
    /// Managed directory at git root (<AVP_DIR>)
    project_dir: ManagedDirectory<AvpConfig>,

    /// Managed directory at the XDG data directory (e.g., `$XDG_DATA_HOME/avp/`), if available
    home_dir: Option<ManagedDirectory<AvpConfig>>,

    /// Resolved model configuration (defaults to claude-code)
    model_config: ModelConfig,

    /// Agent handle. Starts in [`AgentHandle::Pending`] (lazy or eager) and
    /// transitions to [`AgentHandle::Active`] on the first call to
    /// [`Self::agent`].
    agent_handle: Arc<Mutex<AgentHandle>>,

    /// Turn state manager for tracking file changes during a turn
    turn_state: Arc<TurnStateManager>,

    /// Cached validator runner (lazily initialized from agent)
    runner_cache: Mutex<Option<ValidatorRunner>>,

    /// Optional AVP-level session id used to name validator recording files.
    ///
    /// This is the explicit path for threading a session id through to
    /// [`Self::wrap_with_recording`]. When set via [`Self::set_session_id`]
    /// it takes precedence over the [`Self::SESSION_ID_ENV`] env var; when
    /// neither is set, the recording filename falls back to `"no-session"`.
    ///
    /// We use [`OnceLock`] to make the "set once before [`Self::agent`]"
    /// invariant explicit in the type. The reader and writer use the same
    /// non-blocking primitive — there is no asymmetric `try_lock` dance and no
    /// possibility of a reader silently fooling itself into thinking the value
    /// was never set. Calling [`Self::set_session_id`] more than once is a
    /// silent no-op (the first value wins), which matches the "name the
    /// recording after the first session id we see" semantics.
    session_id: OnceLock<String>,

    /// Handle for the in-process sah MCP server that backs the validator agent.
    ///
    /// Every `AvpContext` owns exactly one validator MCP server. The server is
    /// started lazily on the first call to [`Self::agent`] (which awaits
    /// [`Self::resolve_validator_mcp_config`]), bound to `127.0.0.1:0`, and
    /// held here for the lifetime of the context. There is no env-var
    /// short-circuit and no "fallback" path — the validator agent always talks
    /// to this in-process server.
    ///
    /// The `Option` is `None` only between context construction and the first
    /// `agent()` call; once populated it stays populated until the context is
    /// dropped.
    ///
    /// Lifecycle: dropping `AvpContext` drops the inner [`McpServerHandle`],
    /// which drops its `shutdown_tx: oneshot::Sender<()>`. The server task's
    /// `with_graceful_shutdown` future then resolves (the receiver errors), the
    /// axum serve loop exits, and the listener is closed — freeing the port.
    /// No async teardown is required from `AvpContext`'s side.
    mcp_server_handle: Mutex<Option<McpServerHandle>>,
}

impl std::fmt::Debug for AvpContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvpContext")
            .field("project_dir", &self.project_dir.root())
            .field("home_dir", &self.home_dir.as_ref().map(|d| d.root()))
            .field("model_config", &self.model_config)
            .field("has_agent", &"<async>")
            .field("turn_state", &"<manager>")
            .field("runner_cache", &"<cached>")
            .field("session_id", &self.session_id.get())
            .field("mcp_server_handle", &"<async>")
            .finish()
    }
}

impl AvpContext {
    /// Initialize AVP context by finding git root and creating the AVP directory.
    ///
    /// This will:
    /// 1. Create AVP directory at git root (via swissarmyhammer-directory)
    /// 2. Create .gitignore in the AVP directory if it doesn't exist
    /// 3. Open log file for appending
    /// 4. Optionally connect to user AVP directory
    ///
    /// The agent and the in-process sah MCP server that backs its tool surface
    /// are both created lazily on the first call to [`Self::agent`]. Every
    /// `AvpContext` owns exactly one validator MCP server for its lifetime —
    /// there is no env-var short-circuit and no opt-out. The handle is held
    /// on `self.mcp_server_handle` and released when the context is dropped.
    ///
    /// Returns Err if not in a git repository.
    pub fn init() -> Result<Self, AvpError> {
        let (project_dir, home_dir) = Self::init_directories()?;
        let model_config = Self::resolve_model_config();
        Ok(Self::new_without_agent(project_dir, home_dir, model_config))
    }

    /// Create an AVP context with an injected inner agent.
    ///
    /// This is primarily for testing with [`PlaybackAgent`] or other leaf
    /// agents implementing [`ConnectTo<Client>`]. The inner agent is stashed
    /// in [`AgentHandle::Pending::Eager`] and the connection-establishment
    /// dance (recording wrap + builder + `connect_with`) is deferred until
    /// the first call to [`Self::agent`] so [`Self::set_session_id`] can run
    /// between this constructor returning and the first agent use.
    ///
    /// # Arguments
    ///
    /// * `inner` - Any [`ConnectTo<Client>`] component (typically a
    ///   [`PlaybackAgent`]). Recording wrap and the in-process connection are
    ///   layered on top during [`Self::agent`]. Notifications flow through
    ///   the JSON-RPC connection itself in ACP 0.11, captured by the
    ///   `on_receive_notification` handler installed during arm — there is
    ///   no longer a separate broadcast receiver to thread through.
    ///
    /// [`PlaybackAgent`]: agent_client_protocol_extras::PlaybackAgent
    /// [`ConnectTo<Client>`]: agent_client_protocol::ConnectTo
    ///
    /// # Example
    ///
    /// ```ignore
    /// let playback = PlaybackAgent::new(fixture_path, "test");
    /// let context = AvpContext::with_agent(playback)?;
    /// ```
    pub fn with_agent<A>(inner: A) -> Result<Self, AvpError>
    where
        A: ConnectTo<agent_client_protocol::Client> + Send + 'static,
    {
        let model_config = Self::resolve_model_config();
        Self::init_with_agent_and_model(DynConnectTo::new(inner), model_config)
    }

    /// Create an AVP context with an injected inner agent and explicit model
    /// configuration.
    ///
    /// Like [`Self::with_agent`], but allows specifying the model config
    /// directly instead of resolving it from the project config file. This is
    /// useful for testing the full pipeline with a specific model
    /// configuration.
    pub fn with_agent_and_model<A>(inner: A, model_config: ModelConfig) -> Result<Self, AvpError>
    where
        A: ConnectTo<agent_client_protocol::Client> + Send + 'static,
    {
        Self::init_with_agent_and_model(DynConnectTo::new(inner), model_config)
    }

    /// Build a context whose agent handle starts in [`PendingAgent::Lazy`]
    /// (the [`Self::init`] path).
    ///
    /// Factored out so [`Self::init`] and [`Self::init_with_agent_and_model`]
    /// share a single source of truth for field initialization. Callers must
    /// supply the directories and model config; everything else is derived.
    fn new_without_agent(
        project_dir: ManagedDirectory<AvpConfig>,
        home_dir: Option<ManagedDirectory<AvpConfig>>,
        model_config: ModelConfig,
    ) -> Self {
        // Create turn state manager - uses parent of avp_dir (project root)
        let project_root = project_dir.root().parent().unwrap_or(project_dir.root());
        let turn_state = Arc::new(TurnStateManager::new(project_root));

        Self {
            project_dir,
            home_dir,
            model_config,
            agent_handle: Arc::new(Mutex::new(AgentHandle::Pending(PendingAgent::Lazy))),
            turn_state,
            runner_cache: Mutex::new(None),
            session_id: OnceLock::new(),
            mcp_server_handle: Mutex::new(None),
        }
    }

    /// Shared constructor for the eager-agent paths ([`Self::with_agent`] and
    /// [`Self::with_agent_and_model`]).
    ///
    /// Builds the context and installs the externally-supplied inner agent
    /// (and its notification receiver) in [`PendingAgent::Eager`]. The
    /// recording wrap and the in-process connection are layered on top
    /// during the first call to [`Self::agent`], not here. This deferred-arm
    /// invariant is what lets [`Self::set_session_id`] take effect on the
    /// eager path: a session id installed between this constructor returning
    /// and the first `agent()` call still propagates into the recording
    /// filename (same shape as the lazy [`Self::init`] path).
    ///
    /// The two public eager-path entry points only differ in how
    /// `model_config` is resolved.
    fn init_with_agent_and_model(
        inner: DynConnectTo<agent_client_protocol::Client>,
        model_config: ModelConfig,
    ) -> Result<Self, AvpError> {
        let (project_dir, home_dir) = Self::init_directories()?;

        let ctx = Self::new_without_agent(project_dir, home_dir, model_config);

        // Install the inner agent in the Pending::Eager state. The arm-time
        // logic in `agent()` will wrap this with `RecordingAgent` and run the
        // connection-establishment dance on first observation, which is what
        // gives `set_session_id` its window of effect.
        // No lock contention here: the mutex is fresh and uncontended.
        *ctx.agent_handle
            .try_lock()
            .expect("fresh agent_handle is uncontended") =
            AgentHandle::Pending(PendingAgent::Eager { inner });

        Ok(ctx)
    }

    /// Initialize directories (shared by init and with_agent).
    fn init_directories() -> Result<InitDirectoriesResult, AvpError> {
        let project_dir = ManagedDirectory::<AvpConfig>::from_git_root().map_err(|e| {
            AvpError::Context(format!(
                "failed to create {} directory: {}",
                AvpConfig::DIR_NAME,
                e
            ))
        })?;
        let home_dir = ManagedDirectory::<AvpConfig>::xdg_data().ok();
        Ok((project_dir, home_dir))
    }

    /// Resolve model configuration from project config.
    ///
    /// Uses `ModelManager::resolve_agent_config()` to read the configured model
    /// from the project config file. Falls back to the default claude-code config
    /// if resolution fails (e.g., no config file, invalid model name).
    fn resolve_model_config() -> ModelConfig {
        match ModelManager::resolve_agent_config(&ModelPaths::avp()) {
            Ok(config) => {
                tracing::debug!("Resolved model config: {:?}", config.executor());
                config
            }
            Err(e) => {
                tracing::debug!("Using default model config (claude-code): {}", e);
                ModelConfig::claude_code()
            }
        }
    }

    /// Get the resolved model configuration.
    pub fn model_config(&self) -> &ModelConfig {
        &self.model_config
    }

    /// Environment variable that carries the AVP-level session id into the
    /// recording filename, used as a fallback when [`Self::set_session_id`]
    /// has not been called.
    ///
    /// Prefer [`Self::set_session_id`] in new code — env-based control flow
    /// invites silent drift if the caller forgets to set it. The env var is
    /// kept for backwards compatibility and as an escape hatch for
    /// scripts/tests that can't easily call the explicit API.
    const SESSION_ID_ENV: &'static str = "AVP_SESSION_ID";

    /// Set the AVP-level session id used to name validator recording files.
    ///
    /// This is the explicit, preferred path for threading a session id into
    /// [`RecordingAgent`]'s output filename. Production hook entry points
    /// should call this immediately after [`Self::init`] (and *before* the
    /// first call to [`Self::agent`], since the recording wrap is applied on
    /// first agent use and snapshots the session id at that moment) so
    /// recordings are named after the same session id the hook input carries.
    ///
    /// The session id is stored in a [`OnceLock`], which makes the "set once"
    /// invariant explicit in the type. Calling this method more than once is
    /// a silent no-op — the first value wins. This matches the underlying
    /// recording semantics: a single [`AvpContext`] produces a single
    /// `RecordedSession` file with a single filename, so a second session id
    /// has no place to land anyway.
    ///
    /// When set, this value takes precedence over [`Self::SESSION_ID_ENV`].
    /// When neither is set, the recording filename uses the literal string
    /// `"no-session"`.
    pub fn set_session_id(&self, session_id: impl Into<String>) {
        // OnceLock::set returns Err on second call. We intentionally ignore
        // it — the first id wins, and a duplicate setter call from the same
        // hook entry point is a benign retry, not a programming error.
        let _ = self.session_id.set(session_id.into());
    }

    /// Resolve the AVP-level session id used in recording filenames.
    ///
    /// Order of precedence:
    /// 1. Value set via [`Self::set_session_id`] (the explicit, preferred path).
    /// 2. The [`Self::SESSION_ID_ENV`] env var (legacy fallback).
    /// 3. `None` (recording filename falls back to `"no-session"`).
    ///
    /// Reader and writer share the same [`OnceLock`] primitive, so there is
    /// no asymmetry between "set" and "get" — if `set_session_id` was called,
    /// `get` sees it; if not, the env var is consulted; if not, `None`.
    fn resolved_session_id(&self) -> Option<String> {
        if let Some(id) = self.session_id.get() {
            return Some(id.clone());
        }
        std::env::var(Self::SESSION_ID_ENV).ok()
    }

    /// Resolve the directory where validator recordings are written.
    ///
    /// Always `<AVP_DIR>/recordings/`. There is no env-var override — if the
    /// user wants recordings to live elsewhere they can move the directory
    /// after the fact.
    fn recording_dir(&self) -> PathBuf {
        self.project_dir.subdir("recordings")
    }

    /// Build a recording file path for a validator agent invocation.
    ///
    /// **Layout: `<AVP_DIR>/recordings/<session_id>-<unix_micros>.json`.**
    ///
    /// The original task spec called for a nested layout
    /// (`<session_id>/<hook_type>/<ruleset>/<rule>.json`, one file per rule),
    /// but the implementation flattens it into one JSON file per
    /// [`AvpContext`] lifetime. The reasoning:
    ///
    /// - [`RecordingAgent`] aggregates an entire trait-call sequence
    ///   (`initialize` → `new_session` → `prompt` → ...) into a single
    ///   [`agent_client_protocol_extras::recording::RecordedSession`] and
    ///   flushes it on drop. There is exactly one [`RecordingAgent`] per
    ///   [`AvpContext`] (the wrapper is installed in the lazy `agent()` path
    ///   and reused for every rule), so a single output file matches the
    ///   abstraction's natural granularity.
    ///
    /// - To get one file per rule we'd have to instantiate a new
    ///   [`RecordingAgent`] per rule and tear it down between rules, which
    ///   would force the runner (not the context) to own the wrapper's
    ///   lifecycle — and lose the `initialize` recording for every rule
    ///   except the first.
    ///
    /// - The microsecond suffix prevents collisions when the same AVP session
    ///   triggers multiple hooks (e.g. several PostToolUse calls during one
    ///   Stop session) and keeps each file scoped to a single agent lifetime.
    ///
    /// Replay tests load these aggregated `RecordedSession` files via
    /// [`agent_client_protocol_extras::PlaybackAgent`], which iterates the
    /// recorded calls in order — exactly what the test corpus needs.
    ///
    /// `session_id` is `None` for hook events that don't carry a session
    /// (the rare InstructionsLoaded / WorktreeCreate paths). In that case we
    /// fall back to the literal string `"no-session"`.
    fn recording_path(&self, session_id: Option<&str>) -> PathBuf {
        let session = session_id.unwrap_or("no-session");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        self.recording_dir()
            .join(format!("{}-{}.json", session, stamp))
    }

    /// Wrap an inner validator agent component with [`RecordingAgent`].
    ///
    /// Recording is unconditional — every JSON-RPC message flowing through
    /// the returned wrapper is captured to a JSON file. The recording is
    /// flushed at every prompt response and again when the wrapper is
    /// dropped (i.e. when the connection driving [`AvpContext`] is torn
    /// down).
    ///
    /// In ACP 0.11 [`RecordingAgent<A>`] is itself a [`ConnectTo<Client>`]
    /// middleware, not an [`Agent`]-trait wrapper, so it is composed at
    /// connection-setup time rather than around an already-built agent
    /// object. The notification routing that used to be threaded through a
    /// side-channel is handled internally by `RecordingAgent` from the
    /// JSON-RPC stream — there is no longer a `with_notifications`
    /// constructor.
    ///
    /// The session id used in the filename is resolved by
    /// [`Self::resolved_session_id`] — explicit setter first, env var second.
    fn wrap_with_recording<A>(&self, inner: A) -> RecordingAgent<A>
    where
        A: ConnectTo<agent_client_protocol::Client> + Send + 'static,
    {
        let session_id = self.resolved_session_id();
        let path = self.recording_path(session_id.as_deref());

        tracing::info!(
            "Wrapping validator agent with RecordingAgent (path={})",
            path.display()
        );

        RecordingAgent::new(inner, path)
    }

    /// Decide whether the in-process validator MCP server should register agent tools.
    ///
    /// `agent_mode` controls which tool surface the in-process sah server
    /// exposes. ClaudeCode brings its own Read/Glob/Grep, so registering the
    /// agent tool set on top would just create duplicates and confuse the
    /// model — Claude only needs sah's domain tools (kanban, etc.), which the
    /// `agent_mode: false` registration provides. LlamaAgent (qwen, etc.) has
    /// no built-in tools, so it relies on the agent tool set.
    fn agent_mode_for_validator(&self) -> bool {
        matches!(
            self.model_config.executor_type(),
            ModelExecutorType::LlamaAgent
        )
    }

    /// Start an in-process sah MCP server for validator tools.
    ///
    /// Binds an HTTP listener on `127.0.0.1:0` (random ephemeral port) using
    /// [`start_mcp_server_with_options`]. Returns the [`McpServerConfig`]
    /// pointing at `/mcp/validator` (the validator-only sub-route exposed by
    /// the unified server) and the [`McpServerHandle`] whose `Drop` triggers
    /// graceful shutdown of the spawned server task.
    ///
    /// `agent_mode` is determined by [`Self::agent_mode_for_validator`]:
    /// `LlamaAgent` → `true` (qwen needs Read/Glob/Grep/code_context),
    /// `ClaudeCode` → `false` (claude already has its own).
    ///
    /// Working directory is set to the repo root (parent of `<AVP_DIR>/`) so
    /// the in-process tools see the project, not the AVP bookkeeping dir.
    async fn start_in_process_mcp_server(
        &self,
    ) -> Result<(swissarmyhammer_agent::McpServerConfig, McpServerHandle), AvpError> {
        // project_dir is `<root>/.avp`, so its parent is the repository root.
        // Fall back to project_dir itself if for some reason there's no parent
        // (shouldn't happen in practice — git root always has a parent), to
        // avoid panicking out of the validator hot path.
        let project_root = self
            .project_dir
            .root()
            .parent()
            .unwrap_or_else(|| self.project_dir.root())
            .to_path_buf();

        let agent_mode = self.agent_mode_for_validator();

        tracing::debug!(
            agent_mode,
            working_dir = %project_root.display(),
            "Starting in-process sah MCP server for validator tools"
        );

        let handle = start_mcp_server_with_options(
            McpServerMode::Http { port: None }, // bind 127.0.0.1:0
            None,                               // default PromptLibrary
            None,                               // no model override
            Some(project_root),
            agent_mode,
        )
        .await
        .map_err(|e| {
            AvpError::Context(format!(
                "Failed to start in-process MCP server for validator tools: {}",
                e
            ))
        })?;

        // The server's `connection_url` is `http://127.0.0.1:{port}/mcp` —
        // the validator agent talks to the `/mcp/validator` sub-route which
        // exposes a filtered tool set. We construct that URL from the port
        // rather than parsing/rewriting the connection URL string.
        let port = handle.port().ok_or_else(|| {
            AvpError::Context(
                "In-process MCP server returned no port — cannot route validator agent".to_string(),
            )
        })?;
        let validator_url = format!("http://127.0.0.1:{}/mcp/validator", port);

        tracing::info!(
            url = %validator_url,
            agent_mode,
            "Validator agent in-process MCP server bound; agent will use this endpoint for tool calls"
        );

        Ok((
            swissarmyhammer_agent::McpServerConfig::new(validator_url),
            handle,
        ))
    }

    /// Resolve MCP config for the validator agent.
    ///
    /// Always starts an in-process sah MCP server bound to `127.0.0.1:0`
    /// (random ephemeral port) on first call, holds the resulting
    /// [`McpServerHandle`] on `self`, and returns an MCP config pointing at
    /// the `/mcp/validator` sub-route. Subsequent calls return a config
    /// pointing at the same already-bound URL (defensive against re-entry,
    /// even though `agent()` only calls this once per `AvpContext`).
    ///
    /// `tools_override` is always `String::new()` to ensure the validator
    /// agent only sees MCP-provided tools — claude-code's built-in
    /// Read/Grep/etc. would otherwise mask the qwen-as-validator path's tool
    /// requirements.
    ///
    /// If the in-process server fails to start, the error is propagated. The
    /// validator agent is never constructed without tools.
    async fn resolve_validator_mcp_config(
        &self,
    ) -> Result<(swissarmyhammer_agent::McpServerConfig, String), AvpError> {
        let mut guard = self.mcp_server_handle.lock().await;
        if let Some(existing) = guard.as_ref() {
            // Already started — re-resolve against the existing handle's
            // URL rather than spawn a duplicate. In practice `agent()` only
            // calls this once per `AvpContext`; this branch exists purely as
            // a defensive guard against re-entry.
            let port = existing.port().ok_or_else(|| {
                AvpError::Context(
                    "Existing in-process MCP handle has no port — cannot reuse".to_string(),
                )
            })?;
            let url = format!("http://127.0.0.1:{}/mcp/validator", port);
            return Ok((
                swissarmyhammer_agent::McpServerConfig::new(url),
                String::new(),
            ));
        }

        let (mcp_config, handle) = self.start_in_process_mcp_server().await?;
        *guard = Some(handle);

        Ok((mcp_config, String::new()))
    }

    /// Get the agent for validator execution.
    ///
    /// Returns a [`ConnectionTo<agent_client_protocol::Agent>`] (the client-side
    /// handle for issuing typed requests against the wrapped inner agent) and
    /// the [`claude_agent::NotificationSender`] used by callers to subscribe
    /// to per-session streaming updates.
    ///
    /// On first access, this method **arms** the handle:
    /// - For the lazy [`Self::init`] path it builds the inner agent via
    ///   [`swissarmyhammer_agent::create_agent_with_options`].
    /// - For the eager [`Self::with_agent`]/[`Self::with_agent_and_model`]
    ///   paths it consumes the inner agent stashed at construction time.
    /// - In both cases the inner agent is wrapped with [`RecordingAgent`] and
    ///   wired into a background `Client.builder().connect_with(...)` task,
    ///   yielding a `ConnectionTo<Agent>` handle and a per-session notifier.
    ///
    /// This deferred-arm is what makes [`Self::set_session_id`] take effect
    /// in *both* construction paths: the recording filename is computed at
    /// arm-time (here), never at construction time.
    pub async fn agent(
        &self,
    ) -> Result<
        (
            ConnectionTo<agent_client_protocol::Agent>,
            Arc<claude_agent::NotificationSender>,
        ),
        AvpError,
    > {
        let mut guard = self.agent_handle.lock().await;

        // Fast path: already armed. Just clone out the handles.
        if let AgentHandle::Active(active) = &*guard {
            return Ok((active.connection.clone(), Arc::clone(&active.notifier)));
        }

        // Take the pending state so we own its inner agent / receiver.
        // Replace with a sentinel `Lazy` while we work — if we panic before
        // installing the Active state the handle reverts to "lazy", which is
        // a safe (if expensive) recovery: the next caller will re-attempt.
        let pending = std::mem::replace(&mut *guard, AgentHandle::Pending(PendingAgent::Lazy));
        let pending = match pending {
            AgentHandle::Pending(p) => p,
            AgentHandle::Active(_) => unreachable!("checked above"),
        };

        // Materialise an inner agent regardless of which Pending variant we
        // started in.
        let inner = match pending {
            PendingAgent::Lazy => self.build_lazy_inner_agent().await?,
            PendingAgent::Eager { inner } => inner,
        };

        // Arm the connection: wrap with RecordingAgent, spawn the Client
        // builder task, and capture the resulting `ConnectionTo<Agent>`
        // handle. Notifications are routed by the
        // `on_receive_notification` handler installed inside arm.
        let active = self.arm_agent_connection(inner).await?;

        let result = (active.connection.clone(), Arc::clone(&active.notifier));
        *guard = AgentHandle::Active(active);
        Ok(result)
    }

    /// Build the inner agent for the lazy [`Self::init`] path.
    ///
    /// Resolves the validator MCP config (starting the in-process sah server
    /// on first call, held on `self.mcp_server_handle` for the context's
    /// lifetime), then dispatches to
    /// [`swissarmyhammer_agent::create_agent_with_options`] for the configured
    /// model. Returns the unwrapped inner agent component, type-erased into
    /// [`DynConnectTo<Client>`] so `agent()` can treat the lazy and eager
    /// paths uniformly downstream.
    ///
    /// In ACP 0.11 notifications flow through the JSON-RPC connection itself
    /// rather than a side-channel broadcast, so this helper no longer
    /// returns a separate `broadcast::Receiver<SessionNotification>` — the
    /// `on_receive_notification` handler installed during arm captures
    /// everything we need.
    async fn build_lazy_inner_agent(
        &self,
    ) -> Result<DynConnectTo<agent_client_protocol::Client>, AvpError> {
        tracing::debug!(
            "Creating {:?} agent for validator execution...",
            self.model_config.executor()
        );
        let start = std::time::Instant::now();

        // Point the validator agent at `/mcp/validator` and disable
        // built-in tools so it only has code_context + read-only files.
        // This starts the in-process sah MCP server (held on
        // `self.mcp_server_handle` for the lifetime of the context) so the
        // validator agent — particularly llama-agent (qwen) which has no
        // built-in tools — always has tools to call. There is no env-var
        // short-circuit and no fallback path: the in-process server is the
        // validator agent's only tool surface.
        let (mcp_config, tools_override) = self.resolve_validator_mcp_config().await?;

        let options = swissarmyhammer_agent::CreateAgentOptions {
            ephemeral: true,
            tools_override: Some(tools_override),
        };
        let handle = swissarmyhammer_agent::create_agent_with_options(
            &self.model_config,
            Some(mcp_config),
            options,
        )
        .await
        .map_err(|e| AvpError::Agent(format!("Failed to create agent: {}", e)))?;

        tracing::debug!("Agent created in {:.2}s", start.elapsed().as_secs_f64());

        // After the upstream `swissarmyhammer-agent` migration to ACP 0.11,
        // `AcpAgentHandle.agent` is a `DynConnectTo<Client>` value (the
        // unwrapped inner agent component) rather than the now-removed
        // `Arc<dyn Agent + Send + Sync>`. This file's compile-time
        // dependency on that shape is the design contract D2 lays down for
        // the swissarmyhammer-agent task to satisfy.
        Ok(handle.agent)
    }

    /// Arm the agent handle: wrap with [`RecordingAgent`], spawn the
    /// `Client.builder().connect_with(...)` task, and assemble the
    /// [`ActiveAgent`].
    ///
    /// The connection is driven by a background tokio task whose handle is
    /// held on `ActiveAgent::_task` (via [`AbortOnDrop`]) so dropping
    /// [`AvpContext`] tears the connection down. The task hands its
    /// `ConnectionTo<Agent>` back via a `oneshot` channel before parking on
    /// `pending()` — keeping the connection alive until the task is aborted.
    ///
    /// Notifications flowing from the agent to the client are forwarded into
    /// a freshly-built [`claude_agent::NotificationSender`] via an
    /// `on_receive_notification` handler installed on the builder. In ACP
    /// 0.11 this is the only path notifications travel — there is no
    /// side-channel broadcast.
    async fn arm_agent_connection(
        &self,
        inner: DynConnectTo<agent_client_protocol::Client>,
    ) -> Result<ActiveAgent, AvpError> {
        // Build a fresh per-session notifier. In ACP 0.11 notifications
        // flow through the JSON-RPC channel, so the notifier is fed by the
        // `on_receive_notification` handler below — there is no separate
        // broadcast receiver to bridge.
        let (notifier, _global_rx) =
            claude_agent::NotificationSender::new(NOTIFICATION_CHANNEL_CAPACITY);
        let notifier = Arc::new(notifier);

        // Wrap the inner agent with RecordingAgent at connection-setup time.
        // RecordingAgent is itself ConnectTo<Client> middleware, so the
        // resulting type is what we hand to the builder's connect_with(...).
        let recording = self.wrap_with_recording(inner);

        // Grab a flush handle *before* `recording` is moved into the spawned
        // `connect_with` task below. The handle is an `Arc` clone of the
        // recording state; it stays valid for the entire context lifetime
        // and lets `ActiveAgent`'s `Drop` impl push a final snapshot to
        // disk synchronously, ahead of the asynchronous task abort.
        let recording_flush = recording.flush_handle();

        // Install an `on_receive_notification` handler so SessionNotifications
        // flowing agent→client are routed into the per-session NotificationSender
        // for the validator runner to consume. Other notification variants
        // (extension notifications) are ignored — they are not part of the
        // validator agent's contract.
        let notifier_for_handler = Arc::clone(&notifier);

        // Channel for the spawned task to publish its ConnectionTo<Agent>
        // handle back to us before parking. Using oneshot rather than mpsc:
        // exactly one cx is yielded per task lifetime.
        let (cx_tx, cx_rx) = tokio::sync::oneshot::channel();

        let task = tokio::spawn(async move {
            let result = agent_client_protocol::Client
                .builder()
                .name("avp-validator")
                .on_receive_notification(
                    move |notif: agent_client_protocol::AgentNotification, _cx| {
                        let notifier = Arc::clone(&notifier_for_handler);
                        async move {
                            if let agent_client_protocol::AgentNotification::SessionNotification(
                                update,
                            ) = notif
                            {
                                if let Err(e) = notifier.send_update(update).await {
                                    tracing::warn!(
                                        error = %e,
                                        "validator notifier failed to forward session/update"
                                    );
                                }
                            }
                            Ok(())
                        }
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_with(recording, async move |cx| {
                    // Publish the connection handle so the caller can issue
                    // requests, then park: the connection lives until this
                    // task is aborted (which happens when AvpContext drops).
                    let _ = cx_tx.send(cx);
                    std::future::pending::<Result<(), agent_client_protocol::Error>>().await
                })
                .await;

            if let Err(e) = result {
                tracing::warn!(error = %e, "validator agent connection ended with error");
            }
        });

        // Wait for the spawned task to publish its ConnectionTo<Agent> handle.
        // If the task failed before reaching the main_fn closure (e.g. the
        // builder rejected our handler signature), the oneshot will be
        // dropped and we report a setup error.
        let connection = cx_rx.await.map_err(|_| {
            AvpError::Agent(
                "validator agent connection task aborted before yielding handle".to_string(),
            )
        })?;

        Ok(ActiveAgent {
            connection,
            notifier,
            recording_flush,
            _task: AbortOnDrop(task),
        })
    }

    /// Get the project AVP directory path.
    pub fn avp_dir(&self) -> &Path {
        self.project_dir.root()
    }

    /// Get the turn state manager for tracking file changes.
    pub fn turn_state(&self) -> Arc<TurnStateManager> {
        Arc::clone(&self.turn_state)
    }

    /// Get the project validators directory path (./<AVP_DIR>/validators).
    ///
    /// Returns the path even if it doesn't exist yet.
    pub fn project_validators_dir(&self) -> PathBuf {
        self.project_dir.subdir("validators")
    }

    /// Get the XDG data validators directory path (e.g., `$XDG_DATA_HOME/avp/validators`).
    ///
    /// Returns None if the XDG data directory is not available.
    pub fn home_validators_dir(&self) -> Option<PathBuf> {
        self.home_dir.as_ref().map(|d| d.subdir("validators"))
    }

    /// Ensure the project validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn ensure_project_validators_dir(&self) -> Result<PathBuf, AvpError> {
        self.project_dir
            .ensure_subdir("validators")
            .map_err(|e| AvpError::Context(format!("failed to create validators directory: {}", e)))
    }

    /// Ensure the XDG data validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    /// Returns None if the XDG data directory is not available.
    pub fn ensure_home_validators_dir(&self) -> Option<Result<PathBuf, AvpError>> {
        self.home_dir.as_ref().map(|d| {
            d.ensure_subdir("validators").map_err(|e| {
                AvpError::Context(format!("failed to create user validators directory: {}", e))
            })
        })
    }

    /// Get all validator directories that exist.
    ///
    /// Returns directories in precedence order (user first, then project).
    pub fn existing_validator_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User directory (lower precedence)
        if let Some(home_dir) = self.home_validators_dir() {
            if home_dir.exists() {
                dirs.push(home_dir);
            }
        }

        // Project directory (higher precedence)
        let project_dir = self.project_validators_dir();
        if project_dir.exists() {
            dirs.push(project_dir);
        }

        dirs
    }

    /// Log a hook event via tracing.
    pub fn log_event(&self, event: &HookEvent) {
        tracing::info!(
            hook_type = event.hook_type,
            decision = %event.decision,
            details = ?event.details,
            "hook event"
        );
    }

    /// Log a validator execution event via tracing.
    pub fn log_validator(&self, event: &ValidatorEvent) {
        crate::validator::emit_validator_result_log(
            event.name,
            event.passed,
            event.hook_type,
            event.message,
        );
    }

    // =========================================================================
    // Validator Execution
    // =========================================================================

    /// Execute validators using the cached runner.
    ///
    /// The runner is created lazily on first access and reused for subsequent calls.
    /// If the agent is unavailable, placeholder pass results are returned.
    pub async fn execute_validators(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedValidator> {
        if validators.is_empty() {
            return Vec::new();
        }

        if self.is_agent_skipped() {
            return self.placeholder_validator_results(validators, hook_type);
        }

        let results = self
            .run_validators_with_fallback(validators, hook_type, input, changed_files)
            .await;

        self.log_validator_results(&results, hook_type);
        results
    }

    /// Check if agent execution is disabled via environment variable.
    fn is_agent_skipped(&self) -> bool {
        if std::env::var("AVP_SKIP_AGENT").is_ok() {
            tracing::debug!("AVP_SKIP_AGENT set - skipping agent execution");
            return true;
        }
        false
    }

    /// Run validators with cached runner, falling back to placeholders on error.
    async fn run_validators_with_fallback(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Vec<ExecutedValidator> {
        match self
            .execute_with_cached_runner(validators, hook_type, input, changed_files)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to execute validators: {} - using placeholders", e);
                self.placeholder_validator_results(validators, hook_type)
            }
        }
    }

    /// Log results for each executed validator.
    fn log_validator_results(&self, results: &[ExecutedValidator], hook_type: HookType) {
        let hook_type_str = hook_type.to_string();
        for result in results {
            self.log_validator(&ValidatorEvent {
                name: &result.name,
                passed: result.result.passed(),
                message: result.result.message(),
                hook_type: &hook_type_str,
            });
        }
    }

    /// Execute validators with the cached runner.
    async fn execute_with_cached_runner(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
    ) -> Result<Vec<ExecutedValidator>, AvpError> {
        let mut guard = self.runner_cache.lock().await;

        // Create runner if not cached
        if guard.is_none() {
            tracing::debug!("Creating cached ValidatorRunner...");
            let (agent, notifications) = self.agent().await?;
            let runner = ValidatorRunner::new(agent, notifications)?;
            *guard = Some(runner);
            tracing::debug!("ValidatorRunner cached successfully");
        }

        // Execute with the cached runner
        let runner = guard.as_ref().unwrap();
        tracing::debug!(
            "Executing {} validators via cached ACP runner for hook {}",
            validators.len(),
            hook_type
        );
        Ok(runner
            .execute_validators(validators, hook_type, input, changed_files)
            .await)
    }

    /// Generate placeholder pass results when agent is unavailable.
    fn placeholder_validator_results(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
    ) -> Vec<ExecutedValidator> {
        validators
            .iter()
            .map(|validator| {
                tracing::debug!(
                    "Would execute validator '{}' ({}) for hook {}",
                    validator.name(),
                    validator.source,
                    hook_type
                );

                ExecutedValidator {
                    name: validator.name().to_string(),
                    severity: validator.severity(),
                    result: crate::validator::ValidatorResult::pass(format!(
                        "Validator '{}' matched (runner unavailable)",
                        validator.name()
                    )),
                }
            })
            .collect()
    }

    // ========================================================================
    // RuleSet Execution (New Architecture)
    // ========================================================================

    /// Execute RuleSets using the cached runner.
    ///
    /// Each RuleSet runs in a single agent session with rules evaluated sequentially.
    /// RuleSets execute in parallel with adaptive concurrency control.
    ///
    /// The runner is created lazily on first access and reused for subsequent calls.
    /// If the agent is unavailable, placeholder pass results are returned.
    pub async fn execute_rulesets(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
        raw_diffs: Option<&[crate::turn::FileDiff]>,
    ) -> Vec<ExecutedRuleSet> {
        if rulesets.is_empty() {
            return Vec::new();
        }

        if self.is_agent_skipped() {
            return self.placeholder_ruleset_results(rulesets, hook_type);
        }

        // Note: per-rule `validator result` log lines are emitted eagerly
        // by `ValidatorRunner::execute_ruleset` as each rule's verdict is
        // known, so we do NOT batch-emit them here. Batching would either
        // duplicate the eager emit (two lines per rule) or — worse — be the
        // only place a Stop hook ever logged (and thus drop everything when
        // the run timed out before this awaited future completed). See
        // kanban task `01KQAFE5WGYJK3HZ8WE3B8N86K` for the regression.
        self.run_rulesets_with_fallback(rulesets, hook_type, input, changed_files, raw_diffs)
            .await
    }

    /// Run RuleSets with cached runner, falling back to placeholders on error.
    async fn run_rulesets_with_fallback(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
        raw_diffs: Option<&[crate::turn::FileDiff]>,
    ) -> Vec<ExecutedRuleSet> {
        match self
            .execute_rulesets_with_cached_runner(
                rulesets,
                hook_type,
                input,
                changed_files,
                raw_diffs,
            )
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to execute RuleSets: {} - using placeholders", e);
                self.placeholder_ruleset_results(rulesets, hook_type)
            }
        }
    }

    /// Execute RuleSets with the cached runner.
    async fn execute_rulesets_with_cached_runner(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
        input: &serde_json::Value,
        changed_files: Option<&[String]>,
        raw_diffs: Option<&[crate::turn::FileDiff]>,
    ) -> Result<Vec<ExecutedRuleSet>, AvpError> {
        let mut guard = self.runner_cache.lock().await;

        // Create runner if not cached
        if guard.is_none() {
            tracing::debug!("Creating cached ValidatorRunner...");
            let (agent, notifications) = self.agent().await?;
            let runner = ValidatorRunner::new(agent, notifications)?;
            *guard = Some(runner);
            tracing::debug!("ValidatorRunner cached successfully");
        }

        // Execute with the cached runner
        let runner = guard.as_ref().unwrap();
        tracing::debug!(
            "Executing {} RuleSets via cached ACP runner for hook {}",
            rulesets.len(),
            hook_type
        );
        Ok(runner
            .execute_rulesets(rulesets, hook_type, input, changed_files, raw_diffs)
            .await)
    }

    /// Generate placeholder pass results when agent is unavailable.
    fn placeholder_ruleset_results(
        &self,
        rulesets: &[&RuleSet],
        hook_type: HookType,
    ) -> Vec<ExecutedRuleSet> {
        rulesets
            .iter()
            .map(|ruleset| {
                tracing::debug!(
                    "Would execute RuleSet '{}' ({}) with {} rules for hook {}",
                    ruleset.name(),
                    ruleset.source,
                    ruleset.rules.len(),
                    hook_type
                );

                let rule_results = ruleset
                    .rules
                    .iter()
                    .map(|rule| crate::validator::RuleResult {
                        rule_name: rule.name.clone(),
                        severity: rule.effective_severity(ruleset),
                        result: crate::validator::ValidatorResult::pass(format!(
                            "Rule '{}' matched (runner unavailable)",
                            rule.name
                        )),
                    })
                    .collect();

                crate::validator::ExecutedRuleSet {
                    ruleset_name: ruleset.name().to_string(),
                    rule_results,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_decision_equality() {
        assert_eq!(Decision::Allow, Decision::Allow);
        assert_eq!(Decision::Block, Decision::Block);
        assert_eq!(Decision::Error, Decision::Error);
        assert_ne!(Decision::Allow, Decision::Block);
        assert_ne!(Decision::Allow, Decision::Error);
        assert_ne!(Decision::Block, Decision::Error);
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_with_git_root() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        // Restore original directory
        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok());
        let ctx = result.unwrap();
        assert!(ctx.avp_dir().exists());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_validators_dir() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Validators dir path should be returned even if it doesn't exist
        let validators_path = ctx.project_validators_dir();
        assert!(validators_path.ends_with("validators"));

        // Ensure creates it
        let ensured_path = ctx.ensure_project_validators_dir().unwrap();
        assert!(ensured_path.exists());

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_not_in_git_repo() {
        let temp = TempDir::new().unwrap();
        // No .git directory

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_err());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_log_event_does_not_panic() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // log_event emits tracing::info! — should not panic
        let event = HookEvent {
            hook_type: "PreToolUse",
            decision: Decision::Allow,
            details: Some("tool=Bash".to_string()),
        };
        ctx.log_event(&event);

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_log_validator_does_not_panic() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // log_validator emits tracing::info! — should not panic
        let event = ValidatorEvent {
            name: "test-validator",
            hook_type: "PostToolUse",
            passed: true,
            message: "All checks passed",
        };
        ctx.log_validator(&event);

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_agent_returns_injected_agent() {
        use agent_client_protocol_extras::PlaybackAgent;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        // Create a fixture file for the playback agent. The on-disk schema
        // changed in 0.11 — fixtures use `{"calls": []}` rather than the
        // legacy `{"messages": []}`.
        let fixture_dir = temp.path().join("fixtures");
        fs::create_dir_all(&fixture_dir).unwrap();
        fs::write(fixture_dir.join("test.json"), r#"{"calls": []}"#).unwrap();

        // Inject a PlaybackAgent. In ACP 0.11 it implements `ConnectTo<Client>`
        // directly — there is no separate notifications broadcast to thread.
        let playback = PlaybackAgent::new(fixture_dir.join("test.json"), "test");

        let ctx = AvpContext::with_agent(playback).unwrap();

        // agent() should arm the connection and return the live handle.
        let result = ctx.agent().await;

        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok(), "Should return injected agent");
    }

    #[test]
    fn test_hook_event_construction() {
        // Test HookEvent struct construction and fields
        let event = HookEvent {
            hook_type: "PreToolUse",
            decision: Decision::Allow,
            details: Some("tool=Bash".to_string()),
        };

        assert_eq!(event.hook_type, "PreToolUse");
        assert_eq!(event.decision, Decision::Allow);
        assert_eq!(event.details, Some("tool=Bash".to_string()));

        // Test without details
        let event_no_details = HookEvent {
            hook_type: "Stop",
            decision: Decision::Block,
            details: None,
        };

        assert_eq!(event_no_details.hook_type, "Stop");
        assert_eq!(event_no_details.decision, Decision::Block);
        assert!(event_no_details.details.is_none());
    }

    #[test]
    fn test_validator_event_construction() {
        // Test ValidatorEvent struct construction and fields
        let event = ValidatorEvent {
            name: "no-secrets",
            passed: true,
            message: "No secrets found",
            hook_type: "PostToolUse",
        };

        assert_eq!(event.name, "no-secrets");
        assert!(event.passed);
        assert_eq!(event.message, "No secrets found");
        assert_eq!(event.hook_type, "PostToolUse");

        // Test failed validator
        let failed_event = ValidatorEvent {
            name: "input-validation",
            passed: false,
            message: "Dangerous command detected",
            hook_type: "PreToolUse",
        };

        assert_eq!(failed_event.name, "input-validation");
        assert!(!failed_event.passed);
        assert_eq!(failed_event.message, "Dangerous command detected");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_turn_state() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // turn_state() should return a valid Arc<TurnStateManager>
        let turn_state = ctx.turn_state();
        // Verify we can clone it (Arc functionality)
        let _cloned = Arc::clone(&turn_state);

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_model_config_defaults_to_claude_code() {
        use swissarmyhammer_config::model::ModelExecutorConfig;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();
        let config = ctx.model_config();

        // Default should be claude-code
        assert!(
            matches!(config.executor(), ModelExecutorConfig::ClaudeCode(_)),
            "Default model config should be ClaudeCode, got {:?}",
            config.executor()
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    fn test_resolve_model_config_returns_default_without_config() {
        use swissarmyhammer_config::model::ModelExecutorConfig;

        // When no config file exists, resolve should return claude-code default
        let config = AvpContext::resolve_model_config();
        assert!(matches!(
            config.executor(),
            ModelExecutorConfig::ClaudeCode(_)
        ));
    }

    // ========================================================================
    // Recording configuration tests
    // ========================================================================

    /// The recordings directory is always `<AVP_DIR>/recordings/`. There is
    /// no env-var override.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_recording_dir_is_avp_subdir() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();
        let dir = ctx.recording_dir();

        assert!(
            dir.ends_with("recordings"),
            "recording dir should be <AVP_DIR>/recordings, got {}",
            dir.display()
        );
        assert!(
            dir.starts_with(ctx.avp_dir()),
            "recording dir should live under the AVP project dir"
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_recording_path_includes_session_id_and_extension() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();
        let path = ctx.recording_path(Some("abc123"));

        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("abc123-"),
            "recording filename should start with session id, got {}",
            name
        );
        assert!(
            name.ends_with(".json"),
            "recording filename should end with .json, got {}",
            name
        );
        assert_eq!(path.parent().unwrap(), ctx.recording_dir());

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_recording_path_falls_back_to_no_session() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();
        let path = ctx.recording_path(None);

        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("no-session-"),
            "recording filename without session should use no-session prefix, got {}",
            name
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// `resolved_session_id` should pick up the value installed via
    /// `set_session_id`, in preference to any env var fallback.
    #[test]
    #[serial_test::serial(cwd, env)]
    fn test_resolved_session_id_prefers_explicit_setter_over_env() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        std::env::set_var(AvpContext::SESSION_ID_ENV, "from-env");

        let ctx = AvpContext::init().unwrap();
        ctx.set_session_id("from-setter");

        assert_eq!(
            ctx.resolved_session_id(),
            Some("from-setter".to_string()),
            "explicit setter should win over env"
        );

        std::env::remove_var(AvpContext::SESSION_ID_ENV);
        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// When `set_session_id` has not been called, the env var is the fallback.
    #[test]
    #[serial_test::serial(cwd, env)]
    fn test_resolved_session_id_falls_back_to_env() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        std::env::set_var(AvpContext::SESSION_ID_ENV, "env-only");

        let ctx = AvpContext::init().unwrap();
        assert_eq!(
            ctx.resolved_session_id(),
            Some("env-only".to_string()),
            "env var should be used when setter has not been called"
        );

        std::env::remove_var(AvpContext::SESSION_ID_ENV);
        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// Without the setter or the env var, `resolved_session_id` returns None
    /// (which `recording_path` then renders as "no-session").
    #[test]
    #[serial_test::serial(cwd, env)]
    fn test_resolved_session_id_returns_none_when_unset() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        std::env::remove_var(AvpContext::SESSION_ID_ENV);

        let ctx = AvpContext::init().unwrap();
        assert_eq!(ctx.resolved_session_id(), None);

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// `set_session_id` is set-once: the first call wins, subsequent calls
    /// are silent no-ops. This locks in the [`OnceLock`] semantics — a second
    /// caller (e.g. a buggy retry path) can't quietly mutate the recording
    /// filename out from under the first observer.
    #[test]
    #[serial_test::serial(cwd, env)]
    fn test_set_session_id_is_set_once() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        std::env::remove_var(AvpContext::SESSION_ID_ENV);

        let ctx = AvpContext::init().unwrap();
        ctx.set_session_id("first");
        ctx.set_session_id("second-should-be-ignored");

        assert_eq!(
            ctx.resolved_session_id(),
            Some("first".to_string()),
            "first set wins; second is a silent no-op"
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// `set_session_id` works on the eager-agent path
    /// ([`AvpContext::with_agent`]): the recording wrap is applied lazily on
    /// the first [`AvpContext::agent`] call, so a session id installed
    /// between `with_agent(...)` and `agent().await` propagates into the
    /// recording filename. This is the regression test for the second nit
    /// from the 2026-04-26 21:10 review round.
    ///
    /// Strategy: pass a playback agent through `with_agent`, install a known
    /// session id via `set_session_id`, drive `agent()` once to materialise
    /// the wrap, then drop the context to flush the recording. The on-disk
    /// filename must start with the session id we installed — *not* with
    /// `no-session-`, which would indicate the wrap was applied at
    /// construction time (before the setter ran).
    #[tokio::test]
    #[serial_test::serial(cwd, env)]
    async fn test_set_session_id_propagates_through_eager_with_agent() {
        use agent_client_protocol_extras::PlaybackAgent;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        // Make sure neither the env-var fallback nor any prior setter call
        // is shadowing what we're testing.
        std::env::remove_var(AvpContext::SESSION_ID_ENV);

        // Build an empty fixture for PlaybackAgent — we never actually drive
        // the recorded wrapper through any real calls, we only need to push
        // it past the deferred-wrap point. The on-disk schema in 0.11 is
        // `{"calls": []}`.
        let fixture_dir = temp.path().join("fixtures");
        fs::create_dir_all(&fixture_dir).unwrap();
        let fixture = fixture_dir.join("empty.json");
        fs::write(&fixture, r#"{"calls": []}"#).unwrap();

        let record_dir = {
            let agent = PlaybackAgent::new(fixture, "test");

            let ctx = AvpContext::with_agent(agent).unwrap();

            // Installed *after* the eager constructor returned, *before* the
            // first agent() call. Under the deferred-arm design this must
            // make it into the recording filename.
            ctx.set_session_id("eager-session-id");

            let _ = ctx.agent().await.unwrap();
            let dir = ctx.recording_dir();
            // ctx is dropped here, flushing the recording to disk.
            dir
        };

        let entries: Vec<_> = std::fs::read_dir(&record_dir)
            .expect("read recording dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        assert!(
            entries
                .iter()
                .any(|name| name.starts_with("eager-session-id-") && name.ends_with(".json")),
            "recording filename should embed the session id installed via \
             set_session_id() on the eager path; entries={:?}",
            entries
        );
        assert!(
            !entries.iter().any(|name| name.starts_with("no-session-")),
            "no recording should fall back to 'no-session' when set_session_id \
             was called before agent(); entries={:?}",
            entries
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// Constructing and dropping an `AvpContext` (with no env vars set, no
    /// custom output dir) produces at least one recording file under the
    /// project's `<AVP_DIR>/recordings/` directory. This is the
    /// always-on-recording acceptance test for the env-var-gate removal.
    #[tokio::test]
    #[serial_test::serial(cwd, env)]
    async fn test_recording_is_always_on_with_no_env_vars() {
        use agent_client_protocol_extras::PlaybackAgent;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        // No env-var manipulation: assert recording happens by default.
        std::env::remove_var(AvpContext::SESSION_ID_ENV);

        let fixture_dir = temp.path().join("fixtures");
        fs::create_dir_all(&fixture_dir).unwrap();
        let fixture = fixture_dir.join("empty.json");
        fs::write(&fixture, r#"{"calls": []}"#).unwrap();

        let record_dir = {
            let agent = PlaybackAgent::new(fixture, "test");

            let ctx = AvpContext::with_agent(agent).unwrap();
            let _ = ctx.agent().await.unwrap();
            let dir = ctx.recording_dir();
            // ctx is dropped here, flushing the recording to disk.
            dir
        };

        assert!(
            record_dir.exists(),
            "recordings dir must exist after a context lifetime, looked at {}",
            record_dir.display()
        );

        // Canonicalize both sides because on macOS `/var` is a symlink to
        // `/private/var`, so a raw `starts_with` against the unmodified
        // `temp.path()` would not match a `recording_dir()` resolved through
        // `git_root()`.
        let temp_avp_canonical = temp
            .path()
            .join(".avp")
            .canonicalize()
            .expect("canonicalize avp dir");
        let record_dir_canonical = record_dir.canonicalize().expect("canonicalize record dir");
        assert!(
            record_dir_canonical.starts_with(&temp_avp_canonical),
            "recordings dir must live under <AVP_DIR>, got {} (expected under {})",
            record_dir_canonical.display(),
            temp_avp_canonical.display(),
        );

        let entries: Vec<_> = std::fs::read_dir(&record_dir)
            .expect("read recording dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        assert!(
            !entries.is_empty(),
            "at least one recording file must be written by default, dir={}",
            record_dir.display()
        );
        assert!(
            entries.iter().all(|name| name.ends_with(".json")),
            "every recording file must be JSON, entries={:?}",
            entries
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    // ========================================================================
    // Validator MCP config / in-process MCP server lifecycle
    // ========================================================================

    /// Helper: probe whether `127.0.0.1:port` is currently listening.
    ///
    /// Uses a short connect timeout so a closed port returns quickly rather
    /// than hanging the test. Returns `true` when a TCP connection
    /// succeeds, `false` otherwise (refused, timeout, or any I/O error).
    async fn is_port_listening(port: u16) -> bool {
        let addr = format!("127.0.0.1:{}", port);
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_millis(200),
                tokio::net::TcpStream::connect(&addr),
            )
            .await,
            Ok(Ok(_))
        )
    }

    /// Helper: parse the `:<port>/...` segment out of a URL like
    /// `http://127.0.0.1:54321/mcp/validator`. Returns `None` when no port
    /// is present or it doesn't parse as `u16`.
    fn extract_port_from_url(url: &str) -> Option<u16> {
        let after_colon = url.rsplit("://").next()?.split('/').next()?;
        after_colon.rsplit(':').next()?.parse::<u16>().ok()
    }

    /// `resolve_validator_mcp_config` must always start an in-process sah MCP
    /// server, return a config pointing at its `/mcp/validator` URL, and hold
    /// the handle on the context. Dropping the context must release the
    /// bound port.
    ///
    /// Acceptance criterion: "verify with a unit test that constructs an
    /// AvpContext, reads the URL, drops the context, then asserts the port
    /// is no longer listening".
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_in_process_mcp_server_lifecycle_on_drop() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let port = {
            let ctx = AvpContext::init().unwrap();

            // Resolve the validator MCP config — this should start the
            // in-process server and store the handle on `ctx`.
            let (mcp_config, tools_override) = ctx
                .resolve_validator_mcp_config()
                .await
                .expect("resolve_validator_mcp_config should succeed");

            // Tools must be disabled so claude-code's built-in tools don't
            // mask qwen-as-validator's MCP-only tool surface.
            assert_eq!(
                tools_override, "",
                "tools_override must be the empty string on the validator path"
            );

            let url = mcp_config.url.clone();

            // The URL must point at the validator sub-route on a local port.
            assert!(
                url.starts_with("http://127.0.0.1:"),
                "in-process URL should bind to 127.0.0.1, got: {}",
                url
            );
            assert!(
                url.ends_with("/mcp/validator"),
                "in-process URL should target the /mcp/validator sub-route, got: {}",
                url
            );

            let port = extract_port_from_url(&url)
                .unwrap_or_else(|| panic!("could not parse port from URL: {}", url));

            // While the context is alive, the port must be listening.
            assert!(
                is_port_listening(port).await,
                "port {} should be listening while AvpContext is alive",
                port
            );

            // The mcp_server_handle must hold the handle while ctx is alive.
            {
                let guard = ctx.mcp_server_handle.lock().await;
                assert!(
                    guard.is_some(),
                    "mcp_server_handle must hold the handle once resolve_validator_mcp_config succeeds"
                );
            }

            port
            // ctx is dropped at the end of this block, which drops the
            // McpServerHandle, which drops the shutdown_tx oneshot::Sender,
            // which causes the server task's graceful_shutdown future to
            // resolve and the listener to be released.
        };

        // Allow the spawned shutdown task a moment to actually release the
        // listener. Drop is synchronous from our side, but axum's
        // `with_graceful_shutdown` finishes asynchronously on its task —
        // poll for up to a few seconds rather than sleeping a fixed amount.
        let mut released = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !is_port_listening(port).await {
                released = true;
                break;
            }
        }

        assert!(
            released,
            "port {} should no longer be listening after AvpContext is dropped",
            port
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// `agent_mode_for_validator` must be `false` for the default ClaudeCode
    /// model (claude has its own Read/Glob/Grep — registering the agent tool
    /// set would just confuse the model).
    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_mode_for_validator_defaults_to_false_for_claude() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Default config is ClaudeCode → no agent tools needed.
        assert!(
            !ctx.agent_mode_for_validator(),
            "agent_mode must be false for ClaudeCode validator"
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }

    /// `agent_mode_for_validator` must be `true` for a LlamaAgent model
    /// (qwen has no built-in tools — the in-process sah server must register
    /// the agent tool set so Read/Glob/Grep/code_context are available).
    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_mode_for_validator_is_true_for_llama_agent() {
        use swissarmyhammer_config::model::LlamaAgentConfig;

        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let mut ctx = AvpContext::init().unwrap();
        // Swap in a LlamaAgent config so executor_type() returns LlamaAgent.
        // Direct field mutation is permitted within the same module's test
        // submodule and avoids adding a public test-only constructor.
        ctx.model_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());

        assert!(
            ctx.agent_mode_for_validator(),
            "agent_mode must be true for LlamaAgent validator"
        );

        std::env::set_current_dir(&original_dir).unwrap();
    }
}
