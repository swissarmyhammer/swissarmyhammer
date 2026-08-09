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
    any_lsp_session, context_err, extract_bool_param, extract_file_position, extract_optional_str,
    extract_optional_string_array, extract_optional_usize, extract_required_str,
    extract_required_u32, extract_u32_param, extract_usize_param, lsp_session_for_file,
    open_workspace, DEFAULT_MAX_RESULTS,
};

/// Default call-hierarchy depth for `get inbound_calls`: direct callers only.
const DEFAULT_INBOUND_CALL_DEPTH: u32 = 1;

/// Default for `include_source`: return the definition's source text.
const DEFAULT_INCLUDE_SOURCE: bool = true;

/// Default for `include_declaration`: count the declaration as a reference.
const DEFAULT_INCLUDE_DECLARATION: bool = true;

/// Execute the "get rename_edits" operation.
///
/// Previews a rename at the given position without applying edits.
/// Returns `can_rename: false` when no live LSP is available.
pub(super) async fn execute_get_rename_edits(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let (file_path, line, character) = extract_file_position(args)?;
    let new_name = extract_required_str(args, "new_name")?;

    let opts = swissarmyhammer_code_context::GetRenameEditsOptions {
        file_path: file_path.clone(),
        line,
        character,
        new_name: new_name.to_string(),
    };

    let session = lsp_session_for_file(&file_path);
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
    let file_path = extract_required_str(args, "file_path")?;

    let severity_filter =
        extract_optional_str(args, "severity_filter").map(|s| match s.to_lowercase().as_str() {
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetInboundCallsOptions {
        file_path: file_path.clone(),
        line,
        character,
        depth: extract_u32_param(args, "depth", DEFAULT_INBOUND_CALL_DEPTH),
    };

    let session = lsp_session_for_file(&file_path);
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
    let query = extract_required_str(args, "query")?;

    let opts = WorkspaceSymbolLiveOptions {
        query: query.to_string(),
        max_results: extract_usize_param(args, "max_results", DEFAULT_MAX_RESULTS),
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetDefinitionOptions {
        file_path: file_path.clone(),
        line,
        character,
        include_source: extract_bool_param(args, "include_source", DEFAULT_INCLUDE_SOURCE),
    };

    let session = lsp_session_for_file(&file_path);
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetTypeDefinitionOptions {
        file_path: file_path.clone(),
        line,
        character,
        include_source: extract_bool_param(args, "include_source", DEFAULT_INCLUDE_SOURCE),
    };

    let session = lsp_session_for_file(&file_path);
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetHoverOptions {
        file_path: file_path.clone(),
        line,
        character,
    };

    let session = lsp_session_for_file(&file_path);
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetReferencesOptions {
        file_path: file_path.clone(),
        line,
        character,
        include_declaration: extract_bool_param(
            args,
            "include_declaration",
            DEFAULT_INCLUDE_DECLARATION,
        ),
        max_results: extract_optional_usize(args, "max_results"),
    };

    let session = lsp_session_for_file(&file_path);
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
    let (file_path, line, character) = extract_file_position(args)?;

    let opts = GetImplementationsOptions {
        file_path: file_path.clone(),
        line,
        character,
        max_results: extract_optional_usize(args, "max_results"),
    };

    let session = lsp_session_for_file(&file_path);
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
    let file_path = extract_required_str(args, "file_path")?;

    let opts = GetCodeActionsOptions {
        file_path: file_path.to_string(),
        start_line: extract_required_u32(args, "start_line")?,
        start_character: extract_required_u32(args, "start_character")?,
        end_line: extract_required_u32(args, "end_line")?,
        end_character: extract_required_u32(args, "end_character")?,
        filter_kind: extract_optional_string_array(args, "filter_kind"),
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
