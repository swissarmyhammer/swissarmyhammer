//! Model selection and the AI-agent endpoint command surface.
//!
//! This module is the bridge between the webview's model picker and the
//! in-process ACP agent ([`super::agent_ws::AgentWebSocketServer`]). It does
//! two things:
//!
//! 1. **Enumerate** the models the user can pick — a Claude Code entry plus
//!    every configured local llama chat model — via [`ai_list_models`].
//! 2. **Hand off configuration** for the chosen model — via [`ai_start_agent`],
//!    which prepares the in-process agent endpoint and returns the loopback
//!    `ws://` agent URL plus the board's `http://…/mcp` toolset URL.
//!
//! # Claude-vs-local dispatch is a runtime decision
//!
//! There is no Cargo feature gating local models. `create_agent` dispatches on
//! [`ModelConfig::executor_type`] at runtime, and the set of selectable models
//! is whatever `swissarmyhammer-config` actually defines on this machine
//! (built-in plus project/user overrides). A feature flag would both violate
//! `ARCHITECTURE.md`'s no-feature-flags rule and be inert — the same binary
//! must serve both backends depending on configuration.
//!
//! # Endpoint handoff, not a data channel
//!
//! `ai_start_agent` is a one-time discovery call. The two URLs it returns are
//! consumed by the webview's ACP client directly — Tauri IPC is not on the ACP
//! data path. The `mcpUrl` goes into the ACP client's `newSession.mcpServers`
//! so the agent gets the board's full SwissArmyHammer toolset.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use swissarmyhammer_config::model::{ModelConfig, ModelExecutorType, ModelInfo, ModelManager};
use tokio::sync::RwLock;

use super::agent_ws::AgentWebSocketServer;

/// Environment variable that overrides Claude Code CLI detection.
///
/// When set, its value is treated as the path to the `claude` executable and
/// is used verbatim — bypassing the `PATH` search. A non-existent path makes
/// detection report Claude Code as unavailable, exactly as if `claude` were
/// absent from `PATH`.
const CLAUDE_CLI_ENV: &str = "CLAUDE_CLI";

/// The stable model id of the Claude Code entry.
const CLAUDE_CODE_MODEL_ID: &str = "claude-code";

/// Detect the Claude Code CLI.
///
/// Resolution order:
///
/// 1. If the [`CLAUDE_CLI_ENV`] environment variable is set, its value is the
///    candidate path. It is honored only when it points at an existing file.
/// 2. Otherwise the `claude` executable is looked up on `PATH`.
///
/// Returns the resolved absolute path to the CLI, or `None` when Claude Code
/// is not installed (or the override points nowhere).
pub fn detect_claude_cli() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os(CLAUDE_CLI_ENV) {
        let path = PathBuf::from(override_path);
        // An override that points nowhere means "Claude Code unavailable" —
        // the same observable outcome as `claude` missing from `PATH`. We do
        // not silently fall back to `PATH`: an explicit override wins.
        return path.is_file().then_some(path);
    }
    which::which("claude").ok()
}

/// The backend kind behind a selectable model.
///
/// Serialized in kebab-case so the webview sees `"claude-code"` /
/// `"local-llama"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelKind {
    /// Claude Code — `create_agent` shells out to the `claude` CLI internally.
    ClaudeCode,
    /// A local llama model run in-process by `llama-agent`.
    LocalLlama,
}

/// A model the user can select in the webview.
///
/// Field names are camelCased on the wire so the TypeScript client can consume
/// them directly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Stable identifier — the `swissarmyhammer-config` agent name. Passed back
    /// to [`ai_start_agent`] to select this model.
    pub id: String,
    /// Human-readable label for the picker.
    pub label: String,
    /// Which agent backend this model drives.
    pub kind: ModelKind,
    /// Whether the model can actually be started right now. The Claude Code
    /// entry is always `true` — the agent spawns the `claude` CLI lazily at use
    /// time, so the entry is offered regardless of CLI detection. Local llama
    /// models are also always `true` (the model weights are fetched lazily on
    /// first use).
    pub available: bool,
    /// Optional human-readable note. For Claude Code it carries the resolved
    /// CLI path when the `claude` executable is detected, and is `None`
    /// otherwise; for local llama entries it carries a model description.
    pub hint: Option<String>,
}

/// Build the Claude Code model entry.
///
/// Claude Code is always `available` when offered: the agent spawns the
/// `claude` CLI at use time, so the entry must not be pre-gated on a separate
/// `which("claude")` probe. That probe is unreliable — a GUI app launched from
/// Finder does not inherit the shell `PATH`, so it reports Claude Code
/// unavailable even when `claude` runs fine. CLI detection is now best-effort
/// and only enriches the `hint` with the resolved path when it succeeds.
fn claude_code_model() -> Model {
    let hint = detect_claude_cli().map(|path| format!("Claude Code CLI: {}", path.display()));
    Model {
        id: CLAUDE_CODE_MODEL_ID.to_string(),
        label: "Claude Code".to_string(),
        kind: ModelKind::ClaudeCode,
        available: true,
        hint,
    }
}

/// Parse a discovered agent's config, returning its executor type.
///
/// Returns `None` when the agent content does not parse as a `ModelConfig` or
/// has no executor compatible with the current platform — such an entry cannot
/// be turned into a runnable agent, so it is dropped from enumeration.
fn agent_executor_type(info: &ModelInfo) -> Option<ModelExecutorType> {
    let config = swissarmyhammer_config::model::parse_model_config(&info.content).ok()?;
    // `executor_type()` panics when no executor matches the platform; guard
    // with the fallible `select_executor()` first.
    config.select_executor()?;
    Some(config.executor_type())
}

/// Turn a discovered local-llama agent into a selectable [`Model`].
fn local_llama_model(info: &ModelInfo) -> Model {
    Model {
        id: info.name.clone(),
        label: info.name.clone(),
        kind: ModelKind::LocalLlama,
        // Local models are always selectable — `llama-agent` downloads the
        // GGUF weights lazily on first use, so there is nothing to pre-detect.
        available: true,
        hint: info.description.clone(),
    }
}

/// The tag a model config must carry to surface in the AI panel selector.
const KANBAN_TAG: &str = "kanban";

/// List the models the user can select.
///
/// A model appears **iff** its `swissarmyhammer-config` definition carries the
/// [`KANBAN_TAG`] tag — this lets the built-in model set grow without
/// cluttering the panel. The set is driven entirely by configuration on this
/// machine, never by a compile-time feature.
///
/// - The Claude Code entry is offered when the built-in `claude-code` config is
///   `kanban`-tagged. It is always `available` (the agent spawns the CLI at use
///   time); CLI detection only enriches its `hint`.
/// - Each `kanban`-tagged local llama chat model is offered. Embedding
///   executors (`llama-embedding`, `ane-embedding`) cannot back a chat agent
///   and are excluded regardless of tag.
///
/// # Errors
///
/// Returns an error string only when agent discovery itself fails. A single
/// malformed agent file is skipped, not fatal.
#[tauri::command]
pub fn ai_list_models() -> Result<Vec<Model>, String> {
    let agents = ModelManager::list_agents().map_err(|e| format!("failed to list models: {e}"))?;

    let mut models = Vec::new();

    // The Claude Code entry is synthesized below; it is offered only when the
    // built-in `claude-code` config opts into the panel via the `kanban` tag.
    let claude_code_tagged = agents
        .iter()
        .any(|a| a.name == CLAUDE_CODE_MODEL_ID && a.tags.iter().any(|t| t == KANBAN_TAG));
    if claude_code_tagged {
        models.push(claude_code_model());
    }

    for agent in &agents {
        // The Claude Code entry is synthesized above; skip the built-in
        // `claude-code` agent file so it is not duplicated.
        if agent.name == CLAUDE_CODE_MODEL_ID {
            continue;
        }
        // Only models opted into the panel via the `kanban` tag are offered.
        if !agent.tags.iter().any(|t| t == KANBAN_TAG) {
            continue;
        }
        // Only `llama-agent` executors back a chat agent. `claude-code` is
        // handled above; embedding executors cannot be used as agents.
        if agent_executor_type(agent) == Some(ModelExecutorType::LlamaAgent) {
            models.push(local_llama_model(agent));
        }
    }

    Ok(models)
}

/// Resolve a model id to the [`ModelConfig`] that drives its agent.
///
/// `claude-code` resolves to [`ModelConfig::claude_code`] directly; any other
/// id is looked up among the configured agents and its YAML parsed.
///
/// # Errors
///
/// Returns an error string when the id is unknown, the agent file is
/// malformed, or the resolved config is not a runnable chat agent.
fn resolve_model_config(model_id: &str) -> Result<ModelConfig, String> {
    if model_id == CLAUDE_CODE_MODEL_ID {
        return Ok(ModelConfig::claude_code());
    }

    let info = ModelManager::find_agent_by_name(model_id)
        .map_err(|e| format!("unknown model `{model_id}`: {e}"))?;
    let config = swissarmyhammer_config::model::parse_model_config(&info.content)
        .map_err(|e| format!("model `{model_id}` has an invalid configuration: {e}"))?;

    match config.executor_type() {
        ModelExecutorType::ClaudeCode | ModelExecutorType::LlamaAgent => Ok(config),
        other => Err(format!(
            "model `{model_id}` uses executor {other:?}, which cannot back a chat agent"
        )),
    }
}

/// The two endpoint URLs handed to the webview's ACP client.
///
/// Field names are camelCased on the wire.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpoint {
    /// Loopback WebSocket URL the ACP client `initialize`s the agent over,
    /// e.g. `ws://127.0.0.1:<port>`.
    pub ws_url: String,
    /// The board's full-SAH-toolset MCP URL, e.g.
    /// `http://127.0.0.1:<port>/mcp`. The ACP client puts this in
    /// `newSession.mcpServers`. `None` when the board has no MCP server.
    pub mcp_url: Option<String>,
}

/// A running in-process ACP agent endpoint, tracked for teardown.
///
/// Owns the spawned accept-loop task; [`Drop`] aborts it so the bound loopback
/// port is released when the board (or the app) goes away.
pub struct RunningAgent {
    /// The loopback `ws://` URL the webview connects to.
    ws_url: String,
    /// The accept-loop task. `AgentWebSocketServer::run` only returns on an
    /// irrecoverable accept error, so this is aborted on teardown.
    task: tokio::task::JoinHandle<()>,
}

impl RunningAgent {
    /// The loopback `ws://` URL this agent is served on.
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }
}

impl Drop for RunningAgent {
    fn drop(&mut self) {
        // Abort the accept loop so the loopback listener is dropped and its
        // port released. New connections stop being accepted immediately.
        self.task.abort();
    }
}

/// Registry of running in-process agent endpoints, keyed by board path.
///
/// One agent endpoint per open board: re-selecting a model for a board
/// replaces (and thereby tears down) the previous endpoint. Dropping the
/// registry — or removing an entry — stops the corresponding agent.
#[derive(Default)]
pub struct RunningAgents {
    by_board: RwLock<HashMap<PathBuf, RunningAgent>>,
}

impl RunningAgents {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind and start an in-process agent for `board_path` using `config`,
    /// returning its loopback `ws://` URL.
    ///
    /// Any agent previously running for this board is torn down first — a
    /// board has at most one live agent endpoint, so re-selecting a model
    /// never leaks the prior server.
    ///
    /// # Errors
    ///
    /// Returns an error string when the loopback socket cannot be bound.
    pub async fn start(&self, board_path: &Path, config: ModelConfig) -> Result<String, String> {
        let server = AgentWebSocketServer::bind_with(config)
            .await
            .map_err(|e| format!("failed to bind in-process agent server: {e}"))?;
        let addr = server.local_addr();
        let ws_url = format!("ws://{addr}");
        let task = tokio::spawn(server.run());

        let running = RunningAgent {
            ws_url: ws_url.clone(),
            task,
        };
        // Inserting replaces any prior entry; the displaced `RunningAgent`
        // is dropped here, which aborts its accept loop.
        self.by_board
            .write()
            .await
            .insert(board_path.to_path_buf(), running);
        Ok(ws_url)
    }

    /// Stop and drop the agent endpoint for `board_path`, if one is running.
    ///
    /// Called when a board closes so its agent does not outlive it. A no-op
    /// when no agent is registered for the board.
    pub async fn stop(&self, board_path: &Path) {
        if self.by_board.write().await.remove(board_path).is_some() {
            tracing::info!(board = %board_path.display(), "stopped in-process AI agent");
        }
    }

    /// Stop and drop every running agent endpoint.
    ///
    /// Called on app teardown so no agent server outlives the process.
    pub async fn stop_all(&self) {
        let count = {
            let mut by_board = self.by_board.write().await;
            let count = by_board.len();
            by_board.clear();
            count
        };
        if count > 0 {
            tracing::info!(count, "stopped all in-process AI agents on teardown");
        }
    }
}

/// Prepare the in-process agent endpoint for the chosen model and return the
/// two URLs the webview's ACP client needs.
///
/// This is a one-time configuration handoff:
///
/// - `wsUrl` — a fresh loopback `ws://127.0.0.1:<port>` the ACP client
///   `initialize`s the agent over. The agent is built from the selected
///   model's [`ModelConfig`]; `create_agent` dispatches Claude-vs-local at
///   runtime.
/// - `mcpUrl` — the board's `http://127.0.0.1:<port>/mcp` full-SAH-toolset URL
///   (from the per-board MCP server). The client places it in
///   `newSession.mcpServers`.
///
/// The agent endpoint is registered in [`AppState`] so it is torn down when
/// the board closes or the app exits.
///
/// # Errors
///
/// Returns an error string when the model id is unknown, the board is not
/// open, or the loopback agent server cannot be bound.
#[tauri::command]
pub async fn ai_start_agent(
    model_id: String,
    board_path: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<AgentEndpoint, String> {
    let config = resolve_model_config(&model_id)?;

    let canonical = PathBuf::from(&board_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&board_path));

    // The board must be open: its MCP server is what supplies the toolset URL.
    let mcp_url = {
        let boards = state.boards.read().await;
        let handle = boards
            .get(&canonical)
            .ok_or_else(|| format!("board is not open: {}", canonical.display()))?;
        handle.mcp_url().map(str::to_string)
    };

    let ws_url = state.running_agents.start(&canonical, config).await?;

    tracing::info!(
        model = %model_id,
        board = %canonical.display(),
        ws_url = %ws_url,
        "prepared in-process AI agent endpoint"
    );

    Ok(AgentEndpoint { ws_url, mcp_url })
}

/// Report the AI conversation's streaming status into `UIState`.
///
/// The AI panel's conversation lifecycle lives entirely in the webview
/// (`useConversation`). This command mirrors that transient turn status into
/// the backend `UIState.ai_streaming` flag — exactly the role the `can_undo` /
/// `can_redo` flags play for the undo stack — so the synchronous
/// `Command::available()` check for `ai.cancel` can gate the palette entry
/// (cancellable only mid-stream) without reaching into the webview.
///
/// This is transient-UI-state plumbing, not an entity mutation: it flips a
/// `#[serde(skip)]` availability-cache flag, never persists, and emits no
/// `ui-state-changed` event. It does not belong in `dispatch_command` for the
/// same reason `set_undo_redo_state` is a direct backend call.
#[tauri::command]
pub fn ai_set_streaming(
    streaming: bool,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), String> {
    state.ui_state.set_ai_streaming(streaming);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes every test that mutates the `CLAUDE_CLI` / `PATH` process
    /// environment. `std::env::set_var` is process-global, so these tests must
    /// not run concurrently with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that saves and restores `CLAUDE_CLI` and `PATH` around a
    /// test, holding [`ENV_LOCK`] for the test's whole duration.
    ///
    /// Restoring on `Drop` — even on panic — keeps the real process
    /// environment (and therefore the developer's shell and the rest of the
    /// suite) untouched.
    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev_claude_cli: Option<std::ffi::OsString>,
        prev_path: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn acquire() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            Self {
                _lock: lock,
                prev_claude_cli: std::env::var_os(CLAUDE_CLI_ENV),
                prev_path: std::env::var_os("PATH"),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev_claude_cli {
                Some(v) => std::env::set_var(CLAUDE_CLI_ENV, v),
                None => std::env::remove_var(CLAUDE_CLI_ENV),
            }
            match &self.prev_path {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    /// Create an executable file named `name` inside `dir`.
    fn write_fake_executable(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn claude_detection_finds_binary_on_path() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_executable(dir.path(), "claude");

        // A directory holding a fake `claude` is the only entry on PATH, and
        // no override is set — detection must resolve to that binary.
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", dir.path());

        let detected = detect_claude_cli().expect("claude must be detected on PATH");
        assert_eq!(
            detected.canonicalize().unwrap(),
            fake.canonicalize().unwrap(),
        );
    }

    #[test]
    fn claude_detection_returns_none_when_absent() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();

        // An empty PATH directory and no override — `claude` is nowhere.
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", dir.path());

        assert!(
            detect_claude_cli().is_none(),
            "claude must not be detected when absent from PATH"
        );
    }

    #[test]
    fn claude_detection_honors_cli_override() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        // The override binary is deliberately NOT named `claude` and NOT on
        // PATH — only the explicit override should make detection find it.
        let fake = write_fake_executable(dir.path(), "my-claude-build");

        let empty = tempfile::tempdir().unwrap();
        std::env::set_var("PATH", empty.path());
        std::env::set_var(CLAUDE_CLI_ENV, &fake);

        let detected = detect_claude_cli().expect("CLAUDE_CLI override must be honored");
        assert_eq!(detected, fake);
    }

    #[test]
    fn claude_detection_override_pointing_nowhere_is_unavailable() {
        let _env = EnvGuard::acquire();
        // A non-existent override path means "Claude Code unavailable" — the
        // override wins, so detection must NOT fall back to PATH.
        let dir = tempfile::tempdir().unwrap();
        write_fake_executable(dir.path(), "claude");
        std::env::set_var("PATH", dir.path());
        std::env::set_var(CLAUDE_CLI_ENV, "/no/such/claude/binary");

        assert!(
            detect_claude_cli().is_none(),
            "an override that points nowhere must report Claude Code unavailable"
        );
    }

    #[test]
    fn list_models_includes_claude_code_entry_reflecting_detection() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_executable(dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", dir.path());

        let models = ai_list_models().expect("model enumeration must succeed");

        let claude = models
            .iter()
            .find(|m| m.id == CLAUDE_CODE_MODEL_ID)
            .expect("a Claude Code entry must always be present");
        assert_eq!(claude.kind, ModelKind::ClaudeCode);
        assert!(
            claude.available,
            "Claude Code must be available when `claude` is on PATH"
        );
        assert!(
            claude
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains(&fake.display().to_string()),
            "the available hint should name the detected CLI path"
        );

        // The Claude Code entry must appear exactly once — the synthesized
        // entry must not be duplicated by the built-in `claude-code` agent.
        let claude_count = models
            .iter()
            .filter(|m| m.id == CLAUDE_CODE_MODEL_ID)
            .count();
        assert_eq!(claude_count, 1, "Claude Code must not be listed twice");
    }

    #[test]
    fn list_models_claude_code_available_even_when_cli_not_detected() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var(CLAUDE_CLI_ENV);
        // An empty PATH directory — `claude` is not detectable here.
        std::env::set_var("PATH", dir.path());

        let models = ai_list_models().expect("model enumeration must succeed");
        let claude = models
            .iter()
            .find(|m| m.id == CLAUDE_CODE_MODEL_ID)
            .expect("a Claude Code entry must always be present");
        // Claude Code stays available even when the CLI is not detected — the
        // agent spawns `claude` at use time, so a flaky `which` probe (e.g. a
        // GUI app with a stripped PATH) must not disable the entry.
        assert!(
            claude.available,
            "Claude Code must be available regardless of CLI detection"
        );
        // With no CLI detected there is no resolved path to show.
        assert!(
            claude.hint.is_none(),
            "no CLI path hint when claude is not detected, got {:?}",
            claude.hint
        );
    }

    #[test]
    fn list_models_includes_kanban_tagged_qwen_and_excludes_others() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        write_fake_executable(dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", dir.path());

        let models = ai_list_models().expect("model enumeration must succeed");

        // `claude-code` and `qwen` both carry the `kanban` tag, so both must
        // surface in the panel. `qwen` is a local llama chat model.
        assert!(
            models.iter().any(|m| m.id == CLAUDE_CODE_MODEL_ID),
            "the Claude Code entry must be listed, got {models:?}"
        );
        let qwen = models
            .iter()
            .find(|m| m.id == "qwen")
            .unwrap_or_else(|| panic!("`qwen` must be listed, got {models:?}"));
        assert_eq!(
            qwen.kind,
            ModelKind::LocalLlama,
            "`qwen` must be enumerated as a local llama model, got {qwen:?}"
        );

        // The other qwen variants stay untagged and must not appear.
        for excluded in ["qwen-coder", "qwen-0.6b-test", "qwen-embedding"] {
            assert!(
                !models.iter().any(|m| m.id == excluded),
                "untagged model `{excluded}` must not be listed, got {models:?}"
            );
        }
    }

    #[test]
    fn list_models_returns_exactly_kanban_tagged_models() {
        use std::collections::BTreeSet;

        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        write_fake_executable(dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", dir.path());

        let returned: BTreeSet<String> = ai_list_models()
            .expect("model enumeration must succeed")
            .into_iter()
            .map(|m| m.id)
            .collect();

        // The selector must list exactly the configured agents whose config
        // carries the `kanban` tag — no untagged llama models, no embedding
        // models, and the Claude Code entry only when `claude-code` is tagged.
        let expected: BTreeSet<String> = ModelManager::list_agents()
            .expect("agent discovery must succeed")
            .into_iter()
            .filter(|a| a.tags.iter().any(|t| t == "kanban"))
            .map(|a| a.name)
            .collect();

        assert_eq!(
            returned, expected,
            "the AI panel selector must list exactly the kanban-tagged models"
        );
    }

    #[test]
    fn list_models_excludes_untagged_llama_model() {
        let _env = EnvGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PATH", dir.path());

        let models = ai_list_models().expect("model enumeration must succeed");

        // `qwen-0.6b-test` is a built-in `llama-agent` model with no `kanban`
        // tag — it is a runnable chat executor but must not clutter the panel.
        assert!(
            !models.iter().any(|m| m.id == "qwen-0.6b-test"),
            "an untagged llama model must be excluded from the selector, got {models:?}"
        );
    }

    #[test]
    fn resolve_model_config_for_claude_code() {
        let config = resolve_model_config(CLAUDE_CODE_MODEL_ID)
            .expect("claude-code must resolve to a config");
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
    }

    #[test]
    fn resolve_model_config_rejects_unknown_id() {
        let err = resolve_model_config("definitely-not-a-real-model")
            .expect_err("an unknown model id must be rejected");
        assert!(
            err.contains("unknown model"),
            "error should name the failure, got: {err}"
        );
    }

    #[test]
    fn resolve_model_config_for_local_llama_model() {
        // `qwen-coder` is a built-in `llama-agent` model. Resolving it must
        // yield a runnable chat-agent config.
        let config = resolve_model_config("qwen-coder")
            .expect("a built-in llama model must resolve to a config");
        assert_eq!(config.executor_type(), ModelExecutorType::LlamaAgent);
    }

    /// Open one ACP WebSocket frame and send an `initialize` JSON-RPC request,
    /// returning the parsed JSON-RPC response.
    ///
    /// ACP `initialize` only negotiates protocol capabilities — for the Claude
    /// Code backend it does not spawn the `claude` process — so a fake `claude`
    /// on `PATH` is enough to exercise the full transport round trip.
    async fn initialize_over_ws(ws_url: &str) -> serde_json::Value {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let (mut ws, _resp) = tokio_tungstenite::connect_async(ws_url)
            .await
            .expect("ACP client must connect to the agent's loopback ws:// URL");

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } },
            }
        });
        ws.send(Message::text(request.to_string()))
            .await
            .expect("initialize request frame must send");

        let frame = tokio::time::timeout(std::time::Duration::from_secs(15), ws.next())
            .await
            .expect("the agent must answer initialize before the timeout")
            .expect("the agent must produce a response frame")
            .expect("the response frame must not be a transport error");
        let text = match frame {
            Message::Text(t) => t.to_string(),
            other => panic!("expected a text JSON-RPC frame, got {other:?}"),
        };
        let _ = ws.close(None).await;
        serde_json::from_str(&text).expect("the agent's reply must be valid JSON")
    }

    /// `ai_start_agent`'s round trip: starting an agent for a selected model
    /// yields a loopback `ws://` URL a WebSocket client can `initialize` over,
    /// and stopping it (board teardown) frees the port so the URL stops
    /// answering.
    #[tokio::test]
    async fn start_agent_round_trip_and_teardown() {
        // A fake `claude` on PATH satisfies `create_claude_agent`'s detection
        // without depending on a real install. The env guard is held for the
        // whole test because the WebSocket connection task reads PATH when it
        // builds the agent.
        let _env = EnvGuard::acquire();
        let bin_dir = tempfile::tempdir().unwrap();
        write_fake_executable(bin_dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", bin_dir.path());

        let board_dir = tempfile::tempdir().unwrap();
        let config = resolve_model_config(CLAUDE_CODE_MODEL_ID)
            .expect("claude-code must resolve to a config");

        let agents = RunningAgents::new();
        let ws_url = agents
            .start(board_dir.path(), config)
            .await
            .expect("starting the in-process agent must succeed");
        assert!(
            ws_url.starts_with("ws://127.0.0.1:"),
            "the agent endpoint must be a loopback ws:// URL, got {ws_url}"
        );

        // The selected model's agent answers an ACP `initialize` over the
        // returned WebSocket URL.
        let response = initialize_over_ws(&ws_url).await;
        assert_eq!(response["id"], serde_json::json!(1));
        assert!(
            response.get("result").is_some(),
            "initialize must produce a JSON-RPC result, got {response}"
        );

        // Teardown stops the agent: the loopback server is dropped, so the URL
        // must stop accepting WebSocket connections.
        agents.stop(board_dir.path()).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let probe = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio_tungstenite::connect_async(ws_url.as_str()),
        )
        .await;
        let stopped = match probe {
            Ok(result) => result.is_err(),
            // A timeout also means the listener is gone.
            Err(_) => true,
        };
        assert!(
            stopped,
            "after teardown the agent's ws:// URL must stop accepting connections"
        );
    }

    /// `RunningAgents::stop_all` tears down every registered endpoint — the
    /// app-teardown path.
    #[tokio::test]
    async fn stop_all_tears_down_every_agent() {
        let _env = EnvGuard::acquire();
        let bin_dir = tempfile::tempdir().unwrap();
        write_fake_executable(bin_dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", bin_dir.path());

        let agents = RunningAgents::new();
        let board_a = tempfile::tempdir().unwrap();
        let board_b = tempfile::tempdir().unwrap();
        let config = || resolve_model_config(CLAUDE_CODE_MODEL_ID).unwrap();

        let url_a = agents.start(board_a.path(), config()).await.unwrap();
        let url_b = agents.start(board_b.path(), config()).await.unwrap();
        assert_ne!(url_a, url_b, "each board gets its own loopback endpoint");

        agents.stop_all().await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        for url in [&url_a, &url_b] {
            let probe = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio_tungstenite::connect_async(url.as_str()),
            )
            .await;
            let stopped = matches!(probe, Ok(Err(_)) | Err(_));
            assert!(stopped, "stop_all must stop the agent at {url}");
        }
    }

    /// Re-selecting a model for the same board replaces the endpoint — the old
    /// WebSocket server is torn down so no port leaks.
    #[tokio::test]
    async fn re_selecting_a_model_replaces_the_endpoint() {
        let _env = EnvGuard::acquire();
        let bin_dir = tempfile::tempdir().unwrap();
        write_fake_executable(bin_dir.path(), "claude");
        std::env::remove_var(CLAUDE_CLI_ENV);
        std::env::set_var("PATH", bin_dir.path());

        let agents = RunningAgents::new();
        let board = tempfile::tempdir().unwrap();

        let first = agents
            .start(
                board.path(),
                resolve_model_config(CLAUDE_CODE_MODEL_ID).unwrap(),
            )
            .await
            .unwrap();
        let second = agents
            .start(
                board.path(),
                resolve_model_config(CLAUDE_CODE_MODEL_ID).unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            first, second,
            "a fresh selection must bind a new loopback endpoint"
        );

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let probe = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio_tungstenite::connect_async(first.as_str()),
        )
        .await;
        let first_stopped = matches!(probe, Ok(Err(_)) | Err(_));
        assert!(
            first_stopped,
            "the displaced endpoint {first} must be torn down"
        );

        agents.stop(board.path()).await;
    }
}
