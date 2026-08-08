//! The `code_context` handlers backed by a live language server.
//!
//! Each op asks the process-wide supervisor for the session that handles the
//! file's extension (see [`lsp_session_for_file`](super::support::lsp_session_for_file)),
//! and answers with a "no live LSP" result rather than an error when none is
//! running — the tree-sitter layer still works, and a missing server is a
//! degradation the caller is told about, not a failure.

use crate::mcp::op_tool_helpers::json_result;
use crate::mcp::tool_registry::ToolContext;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_code_context::{
    DiagnosticSeverity, GetCodeActionsOptions, GetDefinitionOptions, GetDiagnosticsOptions,
    GetHoverOptions, GetImplementationsOptions, GetInboundCallsOptions, GetReferencesOptions,
    GetTypeDefinitionOptions, LayeredContext, WorkspaceSymbolLiveOptions,
};

use super::leader_route;
use super::support::{
    any_lsp_session, context_err, lsp_session_for_file, open_workspace, DEFAULT_MAX_RESULTS,
};

/// Execute the "get rename_edits" operation.
///
/// Previews a rename at the given position without applying edits.
/// Returns `can_rename: false` when no live LSP is available.
pub(super) async fn execute_get_rename_edits(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let new_name = args
        .get("new_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'new_name'", None))?;

    let opts = swissarmyhammer_code_context::GetRenameEditsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        new_name: new_name.to_string(),
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::get_rename_edits(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get diagnostics" operation.
///
/// Returns errors and warnings for a file via live LSP pull diagnostics.
/// Returns empty when no live LSP is available.
pub(super) fn execute_get_diagnostics(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let severity_filter = args
        .get("severity_filter")
        .and_then(|v| v.as_str())
        .map(|s| match s.to_lowercase().as_str() {
            "error" => DiagnosticSeverity::Error,
            "warning" => DiagnosticSeverity::Warning,
            "info" => DiagnosticSeverity::Info,
            "hint" => DiagnosticSeverity::Hint,
            _ => DiagnosticSeverity::Hint,
        });

    let opts = GetDiagnosticsOptions {
        file_path: file_path.to_string(),
        severity_filter,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let session = lsp_session_for_file(file_path);
    let ctx = LayeredContext::new(&db, session);

    let result = swissarmyhammer_code_context::get_diagnostics(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get inbound_calls" operation.
///
/// Finds all callers of a function at the given position using layered
/// resolution (live LSP call hierarchy, then LSP index, then tree-sitter).
pub(super) async fn execute_get_inbound_calls(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let opts = GetInboundCallsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        depth,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::get_inbound_calls(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "search workspace_symbol" operation.
///
/// Live workspace symbol search with layered resolution: live LSP
/// workspace/symbol, then LSP index, then tree-sitter chunks.
pub(super) async fn execute_workspace_symbol_live(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'query'", None))?;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_MAX_RESULTS);

    let opts = WorkspaceSymbolLiveOptions {
        query: query.to_string(),
        max_results,
    };

    let session = any_lsp_session();
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::workspace_symbol_live(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get definition" operation.
///
/// Go-to-definition with layered resolution: live LSP, LSP index, tree-sitter.
pub(super) async fn execute_get_definition(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let opts = GetDefinitionOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_source,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result = swissarmyhammer_code_context::get_definition(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get type_definition" operation.
///
/// Go-to-type-definition via live LSP only. Returns empty when no LSP is available.
pub(super) async fn execute_get_type_definition(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let opts = GetTypeDefinitionOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_source,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::get_type_definition(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get hover" operation.
///
/// Returns hover information (type signature, docs) with layered resolution.
pub(super) async fn execute_get_hover(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let opts = GetHoverOptions {
        file_path: file_path.to_string(),
        line,
        character,
    };

    let session = lsp_session_for_file(file_path);
    tracing::debug!(file_path = %file_path, client = session.is_some(), "get_hover: session lookup");
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);
    tracing::debug!(
        has_live_lsp = ctx.has_live_lsp(),
        "get_hover: context created"
    );

    let result = swissarmyhammer_code_context::get_hover(&ctx, &opts).map_err(context_err)?;
    tracing::debug!(source_layer = ?result.as_ref().map(|r| &r.source_layer), "get_hover: result");
    json_result(&result)
}

/// Execute the "get references" operation.
///
/// Finds all references to a symbol with layered resolution.
pub(super) async fn execute_get_references(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let include_declaration = args
        .get("include_declaration")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let opts = GetReferencesOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_declaration,
        max_results,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result = swissarmyhammer_code_context::get_references(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get implementations" operation.
///
/// Finds implementations of a trait/interface with layered resolution.
pub(super) async fn execute_get_implementations(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'character'", None))?
        as u32;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let opts = GetImplementationsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        max_results,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::get_implementations(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get code_actions" operation.
///
/// Returns code actions (quickfixes, refactors) for a range via live LSP.
/// Returns empty when no LSP is available.
pub(super) async fn execute_get_code_actions(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

    let start_line = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'start_line'", None))?
        as u32;

    let start_character = args
        .get("start_character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            McpError::invalid_params("missing required parameter 'start_character'", None)
        })? as u32;

    let end_line = args
        .get("end_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'end_line'", None))?
        as u32;

    let end_character = args
        .get("end_character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            McpError::invalid_params("missing required parameter 'end_character'", None)
        })? as u32;

    let filter_kind = args.get("filter_kind").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
    });

    let opts = GetCodeActionsOptions {
        file_path: file_path.to_string(),
        start_line,
        start_character,
        end_line,
        end_character,
        filter_kind,
    };

    let session = lsp_session_for_file(file_path);
    let routers = leader_route::follower_route_for_op(&session, context).await;
    let ws = open_workspace(context)?;
    let db = ws.db();
    let ctx = leader_route::build_layered_context(&db, session, routers);

    let result =
        swissarmyhammer_code_context::get_code_actions(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}
