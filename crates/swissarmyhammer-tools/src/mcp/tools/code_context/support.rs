//! The pieces every `code_context` handler shares.
//!
//! The process-wide LSP supervisor and the sessions read off it, opening a
//! [`CodeContextWorkspace`] from the tool context, the tree-sitter readiness
//! gate, and the notice a caller gets when code intelligence is running on
//! tree-sitter alone.

use crate::mcp::tool_registry::ToolContext;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use swissarmyhammer_code_context::{BlockingStatus, CodeContextWorkspace, IndexLayer};
use swissarmyhammer_common::utils::find_git_repository_root_from;

use super::doctor;

/// Default cap on result count for ops that take an optional `max_results`
/// (`query ast`, `search workspace_symbol`) when the caller omits it. Surfaced
/// in those ops' `max_results` parameter descriptions ("default: 50").
pub(super) const DEFAULT_MAX_RESULTS: usize = 50;

/// Global LSP supervisor handle, initialized once at MCP startup.
/// Used by `get status` to report LSP server state and by `server.rs` for init.
pub(crate) static LSP_SUPERVISOR: std::sync::OnceLock<
    std::sync::Arc<tokio::sync::Mutex<swissarmyhammer_lsp::LspSupervisorManager>>,
> = std::sync::OnceLock::new();

/// Look up the shared [`SharedLspSession`](swissarmyhammer_code_context::SharedLspSession)
/// for a file by matching its extension against the running LSP daemons in the
/// global supervisor.
///
/// Returns the daemon-owned session (not a fresh wrapper), so the layered ops
/// share the one open-document set with the indexing worker and the diagnostics
/// path. Returns `None` when the supervisor is not initialised, no daemon
/// handles the file's extension, or the supervisor lock cannot be acquired
/// (e.g. contention).
pub(crate) fn lsp_session_for_file(
    file_path: &str,
) -> Option<swissarmyhammer_code_context::SharedLspSession> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())?;
    let sup = LSP_SUPERVISOR.get()?;
    let guard = sup.try_lock().ok()?;
    for name in guard.daemon_names() {
        if let Some(daemon) = guard.get_daemon(&name) {
            if daemon
                .file_extensions()
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
            {
                return Some(daemon.session());
            }
        }
    }
    None
}

/// Return the shared session of the first running daemon.
///
/// Useful for workspace-wide LSP requests (e.g. `workspace/symbol`) that are
/// not scoped to a single file extension.
pub(crate) fn any_lsp_session() -> Option<swissarmyhammer_code_context::SharedLspSession> {
    let sup = LSP_SUPERVISOR.get()?;
    let guard = sup.try_lock().ok()?;
    for name in guard.daemon_names() {
        if let Some(daemon) = guard.get_daemon(&name) {
            if matches!(
                daemon.state(),
                swissarmyhammer_lsp::LspDaemonState::Running { .. }
            ) {
                return Some(daemon.session());
            }
        }
    }
    None
}

/// Open a CodeContextWorkspace from the tool context's working directory.
///
/// Falls back to the current directory if no working_dir is set.
pub(crate) fn open_workspace(context: &ToolContext) -> Result<CodeContextWorkspace, McpError> {
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    // Find the git repository root from the working directory
    let workspace_root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);

    CodeContextWorkspace::open(&workspace_root).map_err(|e| {
        McpError::internal_error(
            format!("failed to open code context workspace: {}", e),
            None,
        )
    })
}

/// Convert a CodeContextError into an McpError.
///
/// Most errors become generic `internal_error`s. `ReadOnlyFollower` is special:
/// it's a user-actionable misconfiguration (writes attempted from a non-leader
/// process), so we surface it as `invalid_request` with the typed diagnostic
/// message instead of an opaque "-32603: database error".
pub(super) fn context_err(e: swissarmyhammer_code_context::CodeContextError) -> McpError {
    use swissarmyhammer_code_context::CodeContextError;
    match e {
        e @ CodeContextError::ReadOnlyFollower { .. } => {
            McpError::invalid_request(format!("{}", e), None)
        }
        other => McpError::internal_error(format!("{}", other), None),
    }
}

/// Check if tree-sitter indexing is complete; if not, return a progress message.
///
/// Returns `Ok(None)` when ready, `Ok(Some(result))` with a progress message when not.
pub(super) fn check_ts_readiness(
    ws: &CodeContextWorkspace,
) -> Result<Option<CallToolResult>, McpError> {
    let status =
        swissarmyhammer_code_context::check_blocking_status(&ws.db(), IndexLayer::TreeSitter)
            .map_err(context_err)?;
    match status {
        BlockingStatus::Ready => Ok(None),
        BlockingStatus::NotReady {
            total_files,
            indexed_files,
            progress_percent,
        } => {
            let msg = format!(
                "Index not ready — {}/{} files indexed ({:.0}% complete). Please retry shortly.",
                indexed_files, total_files, progress_percent
            );
            Ok(Some(CallToolResult::success(vec![Content::text(msg)])))
        }
    }
}

/// Check if any LSP servers are missing and return a notice string if so.
///
/// Checks the global LSP_SUPERVISOR for daemons in NotFound state.
/// Falls back to the doctor check if the supervisor isn't initialized.
/// Returns None if all LSP servers are available (no noise).
pub(super) fn lsp_degradation_notice(workspace_root: &std::path::Path) -> Option<String> {
    // Try the supervisor first (it has live state)
    if let Some(sup) = LSP_SUPERVISOR.get() {
        if let Ok(guard) = sup.try_lock() {
            let statuses = guard.status();
            let missing: Vec<_> = statuses
                .iter()
                .filter(|s| matches!(s.state, swissarmyhammer_lsp::LspDaemonState::NotFound))
                .collect();
            if missing.is_empty() {
                return None;
            }
            // Get install hints from the doctor module since DaemonStatus doesn't have them
            let report = doctor::run_doctor(workspace_root);
            let mut lines = vec![
                "\n---".to_string(),
                "Note: Code intelligence is limited to tree-sitter only.".to_string(),
            ];
            for daemon in &missing {
                let hint = report
                    .lsp_servers
                    .iter()
                    .find(|s| s.name == daemon.command)
                    .and_then(|s| s.install_hint.as_deref())
                    .unwrap_or("see project documentation");
                lines.push(format!("  {}: NOT INSTALLED — {}", daemon.command, hint));
            }
            return Some(lines.join("\n"));
        }
    }

    // Supervisor not yet initialized — fall back to doctor check
    let report = doctor::run_doctor(workspace_root);
    let missing: Vec<_> = report.lsp_servers.iter().filter(|s| !s.installed).collect();
    if missing.is_empty() {
        return None;
    }
    let mut lines = vec![
        "\n---".to_string(),
        "Note: Code intelligence is limited to tree-sitter only.".to_string(),
    ];
    for server in &missing {
        let hint = server
            .install_hint
            .as_deref()
            .unwrap_or("see project documentation");
        lines.push(format!("  {}: NOT INSTALLED — {}", server.name, hint));
    }
    Some(lines.join("\n"))
}

/// Append an LSP degradation notice to a successful tool result if applicable.
///
/// Resolves the workspace root from the tool context and checks for missing LSP
/// servers. If any are missing, a second text content item is appended to the result
/// so the caller knows results are tree-sitter only.
pub(super) fn maybe_append_lsp_notice(
    mut result: CallToolResult,
    context: &ToolContext,
) -> CallToolResult {
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let workspace_root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);

    if let Some(notice) = lsp_degradation_notice(&workspace_root) {
        result.content.push(Content::text(notice));
    }
    result
}
