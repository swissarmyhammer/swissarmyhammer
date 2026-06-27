//! "Expose this board to your agent" — register the board's `kanban` MCP
//! server into every mirdan-detected agent's project-scope config.
//!
//! This is the self-contained core of the board-exposure feature. It depends
//! ONLY on external crates (`mirdan`, `swissarmyhammer-common`, `serde`, std)
//! and never references other `kanban-app` modules, so the integration test can
//! compile it standalone via `#[path = "../src/expose.rs"]` (the `kanban-app`
//! binary has no library target — the same pattern `tests/cli_install.rs` uses).
//!
//! The Tauri command wrapper that resolves the board root and the bundled CLI
//! path lives in [`crate::commands`]; this module owns the registration logic
//! and its per-agent result mapping so that logic is testable without Tauri.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use mirdan::install::register_mcp_server_at;
use mirdan::mcp_config::McpServerEntry;
use serde::Serialize;
use swissarmyhammer_common::lifecycle::{InitResult, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

/// MCP server name the board registers itself under in each agent's config.
const KANBAN_SERVER_NAME: &str = "kanban";

/// Per-agent outcome of exposing a board, returned to the frontend.
///
/// `register_mcp_server_at` surfaces per-agent detail only through reporter
/// events (one `Action` per agent that changed, one `Warning`/`Error` per
/// failure) and returns a single summary `InitResult`. So each result here is
/// one captured event: `ok` distinguishes a successful registration from a
/// failure, and `message` is the human-readable description — which already
/// names the agent (e.g. "kanban MCP server for Claude Code"). The agent
/// identity therefore rides in `message` rather than a separately parsed field.
#[derive(Debug, Clone, Serialize)]
pub struct AgentExposeResult {
    /// `true` when the server was registered into this agent's config,
    /// `false` for a per-agent failure (or an agent-detection error).
    pub ok: bool,
    /// Human-readable description of what happened, naming the agent.
    pub message: String,
}

/// An [`InitReporter`] that captures `register_mcp_server_at`'s per-agent events
/// as [`AgentExposeResult`]s.
///
/// `register_mcp_server_at` emits one `Action` per agent it changed and one
/// `Warning`/`Error` per failure; `Header`/`Skipped`/`Finished` carry no
/// per-agent outcome and are ignored. Interior mutability (`Mutex`) is required
/// because [`InitReporter::emit`] takes `&self`.
#[derive(Default)]
struct ResultCollector {
    results: Mutex<Vec<AgentExposeResult>>,
}

impl ResultCollector {
    /// Drain the captured per-agent results.
    fn into_results(self) -> Vec<AgentExposeResult> {
        self.results.into_inner().unwrap_or_default()
    }
}

impl InitReporter for ResultCollector {
    fn emit(&self, event: &InitEvent) {
        let captured = match event {
            InitEvent::Action { message, .. } => Some(AgentExposeResult {
                ok: true,
                message: message.clone(),
            }),
            InitEvent::Warning { message } | InitEvent::Error { message } => {
                Some(AgentExposeResult {
                    ok: false,
                    message: message.clone(),
                })
            }
            InitEvent::Header { .. } | InitEvent::Skipped { .. } | InitEvent::Finished { .. } => {
                None
            }
        };
        if let Some(result) = captured {
            self.results.lock().unwrap().push(result);
        }
    }
}

/// Register the board's `kanban` MCP server into every detected agent's
/// project-scope config, rooted at `board_root`.
///
/// The registered entry is `{ command: <cli_path>, args: ["serve"], env: {} }`:
/// `kanban serve` resolves the board from the process working directory, and
/// project-scope registration means the external agent runs with its CWD at the
/// board root — so no `--board` flag is needed.
///
/// Registration is delegated to [`register_mcp_server_at`], which is rooted at
/// the explicit `board_root` and never reads `current_dir()` — essential in a
/// multi-board GUI launched with a read-only CWD of `/`. It writes each detected
/// agent's project config (`.mcp.json`, `.codex/config.toml`, …) under
/// `board_root` and emits one reporter event per agent.
///
/// Returns one [`AgentExposeResult`] per agent that registered or failed. When
/// no per-agent event fired, returns an EMPTY list for the "no agent detected"
/// case (so the caller can render an informational "no agents" message rather
/// than a misleading 0-agent success), and a single error result only when agent
/// detection itself failed.
pub fn expose_board_to_agents_inner(board_root: &Path, cli_path: &Path) -> Vec<AgentExposeResult> {
    let entry = McpServerEntry {
        command: cli_path.display().to_string(),
        args: vec!["serve".to_string()],
        env: BTreeMap::new(),
    };

    let collector = ResultCollector::default();
    let summary = register_mcp_server_at(
        board_root,
        KANBAN_SERVER_NAME,
        &entry,
        InitScope::Project,
        &collector,
    );

    let per_agent = collector.into_results();
    if !per_agent.is_empty() {
        return per_agent;
    }

    // No per-agent event fired. `register_mcp_server_at` still returns an
    // aggregate summary `InitResult`: a success "applied to 0 agent(s)" when no
    // agent was detected, or an error when agent detection failed. Surface ONLY
    // the error — a 0-agent success means nothing was registered, which the
    // caller renders as "no agents detected", not as a (misleading) success.
    summary
        .iter()
        .filter(|r| r.status == InitStatus::Error)
        .map(summary_to_error_result)
        .collect()
}

/// Map an aggregate agent-detection-failure [`InitResult`] to a failed
/// [`AgentExposeResult`].
fn summary_to_error_result(result: &InitResult) -> AgentExposeResult {
    AgentExposeResult {
        ok: false,
        message: result.message.clone(),
    }
}
