//! Unified code context tool for MCP operations
//!
//! This module provides a single `code_context` tool that dispatches between operations:
//! - `get symbol`: Symbol lookup with source text, locations, and multi-tier fuzzy matching
//! - `search symbol`: Fuzzy search across all indexed symbols
//! - `list symbols`: List all symbols in a specific file
//! - `grep code`: Regex search across stored code chunks
//! - `get callgraph`: Call graph traversal from a starting symbol
//! - `get blastradius`: Blast radius analysis for a file or symbol
//! - `get status`: Health report for the code context index
//! - `rebuild index`: Mark files for re-indexing
//! - `clear status`: Wipe all index data
//! - `lsp status`: Show detected languages, LSP servers, and install status
//! - `detect projects`: Detect project types in the workspace and return guidelines
//! - `find commented_code`: Comment blocks that re-parse as code in the file's own language
//!
//! Uses the `swissarmyhammer-code-context` crate for all operations,
//! opening a `CodeContextWorkspace` from the `ToolContext` working directory.
//!
//! # Where each piece lives
//!
//! This file owns the tool itself — the schema, the dispatch table, and the
//! registration. Everything the dispatch reaches sits in a sibling module, so
//! each one stays small enough to read, and to review, on its own:
//!
//! - [`ops`] — one metadata struct per operation, and the roster the schema is
//!   generated from.
//! - [`support`] — what every handler shares: the LSP supervisor, opening a
//!   workspace from the tool context, the readiness gate, and the
//!   tree-sitter-only notice.
//! - [`execute`] — the handlers backed by the stored tree-sitter index.
//! - [`indexing`] — the indexing pass that fills that index.
//! - [`status`] — the index-lifecycle handlers.
//! - [`lsp_ops`] — the handlers backed by a live language server.
//! - [`detect`] — project-type detection.

pub mod detect;
pub mod doctor;
pub mod execute;
pub mod indexing;
pub(crate) mod leader_route;
pub mod lsp_ops;
pub mod ops;
pub mod schema;
pub mod status;
pub mod support;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use indexing::index_discovered_files_async;
pub use ops::*;
pub(crate) use support::{any_lsp_session, lsp_session_for_file, open_workspace, LSP_SUPERVISOR};

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::Operation;

use support::maybe_append_lsp_notice;

/// Unified code context tool providing symbol lookup, search, and graph operations.
#[derive(Clone, Default)]
pub struct CodeContextTool;

impl CodeContextTool {
    /// Creates a new CodeContextTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl swissarmyhammer_common::health::Doctorable for CodeContextTool {
    fn name(&self) -> &str {
        "Code Context"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use swissarmyhammer_common::health::HealthCheck;

        let mut checks = Vec::new();
        let cat = swissarmyhammer_common::health::Doctorable::category(self);

        // Check LSP server availability for detected project type
        let cwd = std::env::current_dir().unwrap_or_default();
        let report = doctor::run_doctor(&cwd);

        if report.project_types.is_empty() {
            checks.push(HealthCheck::ok(
                "LSP servers",
                "No project type detected — no LSP required",
                cat,
            ));
        } else {
            let types_label = report.project_types.join(", ");
            for lsp in &report.lsp_servers {
                if lsp.installed {
                    checks.push(HealthCheck::ok(
                        format!("{} (LSP)", lsp.name),
                        format!("Available at {}", lsp.path.as_deref().unwrap_or("unknown")),
                        cat,
                    ));
                } else if let Some(ref err) = lsp.error {
                    // Binary found on PATH but doesn't actually work
                    let hint = lsp.install_hint.as_deref().unwrap_or("Check installation");
                    checks.push(HealthCheck::error(
                        format!("{} (LSP)", lsp.name),
                        format!(
                            "Found at {} but broken: {}",
                            lsp.path.as_deref().unwrap_or("unknown"),
                            err
                        ),
                        Some(hint.to_string()),
                        cat,
                    ));
                } else {
                    // Not found at all
                    let hint = lsp
                        .install_hint
                        .as_deref()
                        .unwrap_or("Install the LSP server");
                    checks.push(HealthCheck::warning(
                        format!("{} (LSP)", lsp.name),
                        format!("Not found (needed for {} code intelligence)", types_label),
                        Some(hint.to_string()),
                        cat,
                    ));
                }
            }
        }

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
}
impl swissarmyhammer_common::lifecycle::Initializable for CodeContextTool {
    fn name(&self) -> &str {
        "code_context"
    }
    fn category(&self) -> &str {
        "tools"
    }
    fn priority(&self) -> i32 {
        22
    }

    fn init(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        _reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;

        // Create .code-context/ directory if in a git repo
        let root = swissarmyhammer_common::utils::find_git_repository_root();
        match root {
            Some(root) => {
                let cc_dir = root.join(".code-context");
                if !cc_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(&cc_dir) {
                        return vec![InitResult::error(
                            "code-context",
                            format!("failed to create .code-context/: {}", e),
                        )];
                    }
                }
                // Ensure .code-context/ is in .gitignore
                let gitignore = root.join(".gitignore");
                let needs_entry = if gitignore.exists() {
                    match std::fs::read_to_string(&gitignore) {
                        Ok(content) => !content
                            .lines()
                            .any(|l| l.trim() == ".code-context" || l.trim() == ".code-context/"),
                        Err(_) => true,
                    }
                } else {
                    true
                };
                if needs_entry {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&gitignore)
                    {
                        let _ = writeln!(f, ".code-context/");
                    }
                }
                vec![InitResult::ok(
                    "code-context",
                    "Created .code-context/ directory",
                )]
            }
            None => vec![InitResult::skipped(
                "code-context",
                "No git repository found",
            )],
        }
    }

    fn deinit(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        _reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;

        let root = swissarmyhammer_common::utils::find_git_repository_root();
        match root {
            Some(root) => {
                let cc_dir = root.join(".code-context");
                if cc_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&cc_dir) {
                        return vec![InitResult::error(
                            "code-context",
                            format!("failed to remove .code-context/: {}", e),
                        )];
                    }
                    vec![InitResult::ok(
                        "code-context",
                        "Removed .code-context/ directory",
                    )]
                } else {
                    vec![InitResult::skipped(
                        "code-context",
                        ".code-context/ not found",
                    )]
                }
            }
            None => vec![InitResult::skipped(
                "code-context",
                "No git repository found",
            )],
        }
    }

    // start() and stop() left as defaults — background work is currently managed
    // by McpServer::initialize_code_context() which has access to work_dir.
    // Future: when tools receive context at start time, move that logic here.
}

#[async_trait]
impl McpTool for CodeContextTool {
    fn name(&self) -> &'static str {
        "code_context"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_code_context_schema(code_context_operations())
    }

    fn schema_full(&self) -> serde_json::Value {
        schema::generate_code_context_schema_full(code_context_operations())
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("code_context")
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        code_context_operations()
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        let result = match op_str {
            "get symbol" => execute::execute_get_symbol(&arguments, context),
            "search symbol" => execute::execute_search_symbol(&arguments, context),
            "list symbols" => execute::execute_list_symbols(&arguments, context),
            "grep code" => execute::execute_grep_code(&arguments, context),
            "search code" => execute::execute_search_code(&arguments, context).await,
            "find duplicates" => execute::execute_find_duplicates(&arguments, context),
            "query ast" => execute::execute_query_ast(&arguments, context),
            "find commented_code" => execute::execute_find_commented_code(&arguments, context),
            "get callgraph" => execute::execute_get_callgraph(&arguments, context),
            "get blastradius" => execute::execute_get_blastradius(&arguments, context),
            "get status" => status::execute_get_status(context),
            "rebuild index" => status::execute_rebuild_index(&arguments, context).await,
            "clear status" => status::execute_clear_status(context),
            "lsp status" => status::execute_lsp_status(context),
            "detect projects" => detect::execute_detect(&arguments, context).await,
            "get rename_edits" => lsp_ops::execute_get_rename_edits(&arguments, context).await,
            "get diagnostics" => lsp_ops::execute_get_diagnostics(&arguments, context),
            "get inbound_calls" => lsp_ops::execute_get_inbound_calls(&arguments, context).await,
            "search workspace_symbol" => {
                lsp_ops::execute_workspace_symbol_live(&arguments, context).await
            }
            "get definition" => lsp_ops::execute_get_definition(&arguments, context).await,
            "get type_definition" => lsp_ops::execute_get_type_definition(&arguments, context).await,
            "get hover" => lsp_ops::execute_get_hover(&arguments, context).await,
            "get references" => lsp_ops::execute_get_references(&arguments, context).await,
            "get implementations" => lsp_ops::execute_get_implementations(&arguments, context).await,
            "get code_actions" => lsp_ops::execute_get_code_actions(&arguments, context).await,
            "" => Err(McpError::invalid_params(
                "missing 'op' field. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'search code', 'find duplicates', 'query ast', 'find commented_code', 'get callgraph', 'get blastradius', 'get status', 'rebuild index', 'clear status', 'lsp status', 'detect projects', 'get rename_edits', 'get diagnostics', 'get inbound_calls', 'search workspace_symbol', 'get definition', 'get type_definition', 'get hover', 'get references', 'get implementations', 'get code_actions'",
                None,
            )),
            other => Err(McpError::invalid_params(
                format!(
                    "unknown operation '{}'. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'search code', 'find duplicates', 'query ast', 'find commented_code', 'get callgraph', 'get blastradius', 'get status', 'rebuild index', 'clear status', 'lsp status', 'detect projects', 'get rename_edits', 'get diagnostics', 'get inbound_calls', 'search workspace_symbol', 'get definition', 'get type_definition', 'get hover', 'get references', 'get implementations', 'get code_actions'",
                    other
                ),
                None,
            )),
        };

        // Append LSP degradation notice to query operations (not status operations)
        match op_str {
            "get status" | "rebuild index" | "clear status" | "lsp status" | "detect projects"
            | "" => result,
            _ => result.map(|r| maybe_append_lsp_notice(r, context)),
        }
    }
}

/// Register the code_context tool with the registry.
pub fn register_code_context_tools(registry: &mut ToolRegistry) {
    registry.register(CodeContextTool::new());
}
