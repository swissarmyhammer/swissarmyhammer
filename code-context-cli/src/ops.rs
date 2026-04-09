//! MCP tool operation implementations for code-context.
//!
//! Translates parsed CLI commands into `CodeContextTool` MCP calls and prints
//! results. Each CLI operation variant builds a `serde_json::Map` matching the
//! MCP parameter schema, then calls `CodeContextTool::execute(args, &context).await`.

use std::sync::Arc;

use rmcp::model::RawContent;
use serde_json::{json, Map, Value};
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;
use tokio::sync::Mutex;

use crate::cli::{
    BuildCommands, ClearCommands, Commands, DetectCommands, FindCommands, GetCommands,
    GrepCommands, ListCommands, LspCommands, QueryCommands, SearchCommands,
};

/// Convenience: insert an optional value into args if present.
fn insert_opt<V: Into<Value> + Clone>(args: &mut Map<String, Value>, key: &str, val: &Option<V>) {
    if let Some(v) = val {
        args.insert(key.into(), v.clone().into());
    }
}

/// Insert file_path + line + character (common to many LSP-style get commands).
fn insert_position(args: &mut Map<String, Value>, file_path: &str, line: &u64, character: &u64) {
    args.insert("file_path".into(), json!(file_path));
    args.insert("line".into(), json!(line));
    args.insert("character".into(), json!(character));
}

/// Build args for position-based LSP get operations (definition, hover, references, etc.).
fn build_get_lsp_args(cmd: &GetCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        GetCommands::Definition {
            file_path,
            line,
            character,
        } => {
            args.insert("op".into(), json!("get definition"));
            insert_position(&mut args, file_path, line, character);
        }
        GetCommands::TypeDefinition {
            file_path,
            line,
            character,
        } => {
            args.insert("op".into(), json!("get type_definition"));
            insert_position(&mut args, file_path, line, character);
        }
        GetCommands::Hover {
            file_path,
            line,
            character,
        } => {
            args.insert("op".into(), json!("get hover"));
            insert_position(&mut args, file_path, line, character);
        }
        GetCommands::References {
            file_path,
            line,
            character,
            include_declaration,
            max_results,
        } => {
            args.insert("op".into(), json!("get references"));
            insert_position(&mut args, file_path, line, character);
            insert_opt(&mut args, "include_declaration", include_declaration);
            insert_opt(&mut args, "max_results", max_results);
        }
        GetCommands::Implementations {
            file_path,
            line,
            character,
            max_results,
        } => {
            args.insert("op".into(), json!("get implementations"));
            insert_position(&mut args, file_path, line, character);
            insert_opt(&mut args, "max_results", max_results);
        }
        GetCommands::InboundCalls {
            file_path,
            line,
            character,
            depth,
        } => {
            args.insert("op".into(), json!("get inbound_calls"));
            insert_position(&mut args, file_path, line, character);
            insert_opt(&mut args, "depth", depth);
        }
        GetCommands::RenameEdits {
            file_path,
            line,
            character,
            new_name,
        } => {
            args.insert("op".into(), json!("get rename_edits"));
            insert_position(&mut args, file_path, line, character);
            args.insert("new_name".into(), json!(new_name));
        }
        GetCommands::CodeActions {
            file_path,
            start_line,
            start_character,
            end_line,
            end_character,
            filter_kind,
        } => {
            args.insert("op".into(), json!("get code_actions"));
            args.insert("file_path".into(), json!(file_path));
            args.insert("start_line".into(), json!(start_line));
            args.insert("start_character".into(), json!(start_character));
            args.insert("end_line".into(), json!(end_line));
            args.insert("end_character".into(), json!(end_character));
            insert_opt(&mut args, "filter_kind", filter_kind);
        }
        _ => unreachable!("non-LSP get command passed to build_get_lsp_args"),
    }
    args
}

/// Build args for analysis and index get operations (symbol, callgraph, blastradius, etc.).
fn build_get_analysis_args(cmd: &GetCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        GetCommands::Symbol { query, max_results } => {
            args.insert("op".into(), json!("get symbol"));
            args.insert("query".into(), json!(query));
            insert_opt(&mut args, "max_results", max_results);
        }
        GetCommands::Callgraph {
            symbol,
            direction,
            max_depth,
        } => {
            args.insert("op".into(), json!("get callgraph"));
            args.insert("symbol".into(), json!(symbol));
            insert_opt(&mut args, "direction", direction);
            insert_opt(&mut args, "max_depth", max_depth);
        }
        GetCommands::Blastradius {
            file_path,
            symbol,
            max_hops,
        } => {
            args.insert("op".into(), json!("get blastradius"));
            args.insert("file_path".into(), json!(file_path));
            insert_opt(&mut args, "symbol", symbol);
            insert_opt(&mut args, "max_hops", max_hops);
        }
        GetCommands::Status => {
            args.insert("op".into(), json!("get status"));
        }
        GetCommands::Diagnostics {
            file_path,
            severity_filter,
        } => {
            args.insert("op".into(), json!("get diagnostics"));
            args.insert("file_path".into(), json!(file_path));
            insert_opt(&mut args, "severity_filter", severity_filter);
        }
        _ => unreachable!("non-analysis get command passed to build_get_analysis_args"),
    }
    args
}

/// Build args for any get subcommand, dispatching to the appropriate builder.
fn build_get_args(cmd: &GetCommands) -> Map<String, Value> {
    match cmd {
        GetCommands::Symbol { .. }
        | GetCommands::Callgraph { .. }
        | GetCommands::Blastradius { .. }
        | GetCommands::Status
        | GetCommands::Diagnostics { .. } => build_get_analysis_args(cmd),
        _ => build_get_lsp_args(cmd),
    }
}

fn build_search_args(cmd: &SearchCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        SearchCommands::Symbol {
            query,
            kind,
            max_results,
        } => {
            args.insert("op".into(), json!("search symbol"));
            args.insert("query".into(), json!(query));
            insert_opt(&mut args, "kind", kind);
            insert_opt(&mut args, "max_results", max_results);
        }
        SearchCommands::Code {
            query,
            top_k,
            min_similarity,
            file_pattern,
            language,
        } => {
            args.insert("op".into(), json!("search code"));
            args.insert("query".into(), json!(query));
            insert_opt(&mut args, "top_k", top_k);
            insert_opt(&mut args, "min_similarity", min_similarity);
            insert_opt(&mut args, "file_pattern", file_pattern);
            insert_opt(&mut args, "language", language);
        }
        SearchCommands::WorkspaceSymbol { query, max_results } => {
            args.insert("op".into(), json!("search workspace_symbol"));
            args.insert("query".into(), json!(query));
            insert_opt(&mut args, "max_results", max_results);
        }
    }
    args
}

fn build_grep_args(cmd: &GrepCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        GrepCommands::Code {
            pattern,
            language,
            files,
            max_results,
        } => {
            args.insert("op".into(), json!("grep code"));
            args.insert("pattern".into(), json!(pattern));
            insert_opt(&mut args, "language", language);
            insert_opt(&mut args, "files", files);
            insert_opt(&mut args, "max_results", max_results);
        }
    }
    args
}

fn build_query_args(cmd: &QueryCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        QueryCommands::Ast {
            query,
            language,
            files,
            max_results,
        } => {
            args.insert("op".into(), json!("query ast"));
            args.insert("query".into(), json!(query));
            args.insert("language".into(), json!(language));
            insert_opt(&mut args, "files", files);
            insert_opt(&mut args, "max_results", max_results);
        }
    }
    args
}

fn build_find_args(cmd: &FindCommands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        FindCommands::Duplicates {
            file_path,
            min_similarity,
            max_per_chunk,
            min_chunk_bytes,
        } => {
            args.insert("op".into(), json!("find duplicates"));
            args.insert("file_path".into(), json!(file_path));
            insert_opt(&mut args, "min_similarity", min_similarity);
            insert_opt(&mut args, "max_per_chunk", max_per_chunk);
            insert_opt(&mut args, "min_chunk_bytes", min_chunk_bytes);
        }
    }
    args
}

fn build_simple_args(cmd: &Commands) -> Map<String, Value> {
    let mut args = Map::new();
    match cmd {
        Commands::List { command } => match command {
            ListCommands::Symbols { file_path } => {
                args.insert("op".into(), json!("list symbols"));
                args.insert("file_path".into(), json!(file_path));
            }
        },
        Commands::Build { command } => match command {
            BuildCommands::Status { layer } => {
                args.insert("op".into(), json!("build status"));
                insert_opt(&mut args, "layer", layer);
            }
        },
        Commands::Clear { command } => match command {
            ClearCommands::Status => {
                args.insert("op".into(), json!("clear status"));
            }
        },
        Commands::Lsp { command } => match command {
            LspCommands::Status => {
                args.insert("op".into(), json!("lsp status"));
            }
        },
        Commands::Detect { command } => match command {
            DetectCommands::Projects {
                path,
                max_depth,
                include_guidelines,
            } => {
                args.insert("op".into(), json!("detect projects"));
                insert_opt(&mut args, "path", path);
                insert_opt(&mut args, "max_depth", max_depth);
                insert_opt(&mut args, "include_guidelines", include_guidelines);
            }
        },
        _ => {}
    }
    args
}

/// Build the MCP argument map from a parsed CLI command.
///
/// Returns `Some(map)` for operation commands that should be dispatched to the
/// `CodeContextTool`, or `None` for lifecycle commands (`Serve`, `Init`, etc.)
/// that are handled elsewhere.
pub fn build_args(command: &Commands) -> Option<Map<String, Value>> {
    match command {
        Commands::Serve
        | Commands::Init { .. }
        | Commands::Deinit { .. }
        | Commands::Doctor { .. }
        | Commands::Skill => None,
        Commands::Get { command } => Some(build_get_args(command)),
        Commands::Search { command } => Some(build_search_args(command)),
        Commands::Grep { command } => Some(build_grep_args(command)),
        Commands::Query { command } => Some(build_query_args(command)),
        Commands::Find { command } => Some(build_find_args(command)),
        _ => Some(build_simple_args(command)),
    }
}

/// Execute a CLI operation command against the `CodeContextTool`.
///
/// Creates a minimal `ToolContext`, builds the MCP argument map from the parsed
/// CLI command, executes the tool, and prints the result.
///
/// # Output modes
///
/// - `json_output = true`: prints the full `CallToolResult` as JSON to stdout.
/// - `json_output = false`: extracts text from each `Content::Text` item and
///   prints it to stdout, one per line.
///
/// # Returns
///
/// Exit code: 0 on success, 1 on error.
pub async fn run_operation(command: &Commands, json_output: bool) -> i32 {
    let args = match build_args(command) {
        Some(a) => a,
        None => {
            // Lifecycle commands should not reach here; if they do, bail.
            eprintln!("Error: not a tool operation command");
            return 1;
        }
    };

    let tool = CodeContextTool::new();

    let context = {
        let tool_handlers = Arc::new(ToolHandlers::new());
        let git_ops = Arc::new(Mutex::new(None));
        let agent_config = Arc::new(ModelConfig::default());
        let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
        // Use cwd so the tool discovers the index in the current project
        ctx.working_dir = std::env::current_dir().ok();
        ctx
    };

    let result = match tool.execute(args, &context).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return 1;
        }
    };

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Error serializing result: {e}");
                return 1;
            }
        }
    } else {
        for content in &result.content {
            if let RawContent::Text(t) = &content.raw {
                println!("{}", t.text);
            }
        }
    }

    if result.is_error == Some(true) {
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse CLI args into a `Commands` variant.
    fn parse_command(args: &[&str]) -> Commands {
        use crate::cli::Cli;
        use clap::Parser;
        let mut full = vec!["code-context"];
        full.extend_from_slice(args);
        let cli = Cli::try_parse_from(full).unwrap();
        cli.command
    }

    #[test]
    fn test_get_status_builds_correct_args() {
        let cmd = parse_command(&["get", "status"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get status");
        assert_eq!(args.len(), 1, "get status takes no extra parameters");
    }

    #[test]
    fn test_grep_code_builds_correct_args() {
        let cmd = parse_command(&[
            "grep",
            "code",
            "--pattern",
            "TODO|FIXME",
            "--language",
            "rs",
            "--max-results",
            "20",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "grep code");
        assert_eq!(args.get("pattern").unwrap(), "TODO|FIXME");
        assert_eq!(args.get("language").unwrap(), &json!(["rs"]));
        assert_eq!(args.get("max_results").unwrap(), 20);
    }

    #[test]
    fn test_search_symbol_builds_correct_args() {
        let cmd = parse_command(&[
            "search", "symbol", "--query", "handler", "--kind", "function",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "search symbol");
        assert_eq!(args.get("query").unwrap(), "handler");
        assert_eq!(args.get("kind").unwrap(), "function");
    }

    #[test]
    fn test_search_symbol_without_optional_kind() {
        let cmd = parse_command(&["search", "symbol", "--query", "Config"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "search symbol");
        assert_eq!(args.get("query").unwrap(), "Config");
        assert!(
            args.get("kind").is_none(),
            "kind should be absent when not provided"
        );
    }

    #[test]
    fn test_get_symbol_with_max_results() {
        let cmd = parse_command(&[
            "get",
            "symbol",
            "--query",
            "MyStruct::new",
            "--max-results",
            "5",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get symbol");
        assert_eq!(args.get("query").unwrap(), "MyStruct::new");
        assert_eq!(args.get("max_results").unwrap(), 5);
    }

    #[test]
    fn test_get_callgraph_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "callgraph",
            "--symbol",
            "process_request",
            "--direction",
            "inbound",
            "--max-depth",
            "3",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get callgraph");
        assert_eq!(args.get("symbol").unwrap(), "process_request");
        assert_eq!(args.get("direction").unwrap(), "inbound");
        assert_eq!(args.get("max_depth").unwrap(), 3);
    }

    #[test]
    fn test_get_blastradius_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "blastradius",
            "--file-path",
            "src/server.rs",
            "--symbol",
            "handle",
            "--max-hops",
            "5",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get blastradius");
        assert_eq!(args.get("file_path").unwrap(), "src/server.rs");
        assert_eq!(args.get("symbol").unwrap(), "handle");
        assert_eq!(args.get("max_hops").unwrap(), 5);
    }

    #[test]
    fn test_get_definition_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "definition",
            "--file-path",
            "src/main.rs",
            "--line",
            "10",
            "--character",
            "5",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get definition");
        assert_eq!(args.get("file_path").unwrap(), "src/main.rs");
        assert_eq!(args.get("line").unwrap(), 10);
        assert_eq!(args.get("character").unwrap(), 5);
    }

    #[test]
    fn test_get_references_with_options() {
        let cmd = parse_command(&[
            "get",
            "references",
            "--file-path",
            "f.rs",
            "--line",
            "1",
            "--character",
            "2",
            "--include-declaration",
            "false",
            "--max-results",
            "10",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get references");
        assert_eq!(args.get("include_declaration").unwrap(), false);
        assert_eq!(args.get("max_results").unwrap(), 10);
    }

    #[test]
    fn test_get_code_actions_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "code-actions",
            "--file-path",
            "f.rs",
            "--start-line",
            "1",
            "--start-character",
            "0",
            "--end-line",
            "5",
            "--end-character",
            "10",
            "--filter-kind",
            "quickfix",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get code_actions");
        assert_eq!(args.get("start_line").unwrap(), 1);
        assert_eq!(args.get("end_line").unwrap(), 5);
        assert_eq!(args.get("filter_kind").unwrap(), &json!(["quickfix"]));
    }

    #[test]
    fn test_get_rename_edits_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "rename-edits",
            "--file-path",
            "f.rs",
            "--line",
            "1",
            "--character",
            "4",
            "--new-name",
            "better_name",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get rename_edits");
        assert_eq!(args.get("new_name").unwrap(), "better_name");
    }

    #[test]
    fn test_get_diagnostics_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "diagnostics",
            "--file-path",
            "f.rs",
            "--severity-filter",
            "error",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get diagnostics");
        assert_eq!(args.get("severity_filter").unwrap(), "error");
    }

    #[test]
    fn test_search_code_builds_correct_args() {
        let cmd = parse_command(&[
            "search",
            "code",
            "--query",
            "auth handler",
            "--top-k",
            "5",
            "--min-similarity",
            "0.8",
            "--file-pattern",
            "*.rs",
            "--language",
            "rs",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "search code");
        assert_eq!(args.get("query").unwrap(), "auth handler");
        assert_eq!(args.get("top_k").unwrap(), 5);
        assert_eq!(args.get("min_similarity").unwrap(), 0.8);
        assert_eq!(args.get("file_pattern").unwrap(), "*.rs");
        assert_eq!(args.get("language").unwrap(), &json!(["rs"]));
    }

    #[test]
    fn test_list_symbols_builds_correct_args() {
        let cmd = parse_command(&["list", "symbols", "--file-path", "src/main.rs"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "list symbols");
        assert_eq!(args.get("file_path").unwrap(), "src/main.rs");
    }

    #[test]
    fn test_query_ast_builds_correct_args() {
        let cmd = parse_command(&[
            "query",
            "ast",
            "--query",
            "(function_item)",
            "--language",
            "rust",
            "--files",
            "src/main.rs",
            "--max-results",
            "50",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "query ast");
        assert_eq!(args.get("query").unwrap(), "(function_item)");
        assert_eq!(args.get("language").unwrap(), "rust");
        assert_eq!(args.get("files").unwrap(), &json!(["src/main.rs"]));
        assert_eq!(args.get("max_results").unwrap(), 50);
    }

    #[test]
    fn test_find_duplicates_builds_correct_args() {
        let cmd = parse_command(&[
            "find",
            "duplicates",
            "--file-path",
            "src/handlers.rs",
            "--min-similarity",
            "0.85",
            "--max-per-chunk",
            "3",
            "--min-chunk-bytes",
            "200",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "find duplicates");
        assert_eq!(args.get("file_path").unwrap(), "src/handlers.rs");
        assert_eq!(args.get("min_similarity").unwrap(), 0.85);
        assert_eq!(args.get("max_per_chunk").unwrap(), 3);
        assert_eq!(args.get("min_chunk_bytes").unwrap(), 200);
    }

    #[test]
    fn test_build_status_builds_correct_args() {
        let cmd = parse_command(&["build", "status", "--layer", "treesitter"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "build status");
        assert_eq!(args.get("layer").unwrap(), "treesitter");
    }

    #[test]
    fn test_clear_status_builds_correct_args() {
        let cmd = parse_command(&["clear", "status"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "clear status");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn test_lsp_status_builds_correct_args() {
        let cmd = parse_command(&["lsp", "status"]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "lsp status");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn test_detect_projects_builds_correct_args() {
        let cmd = parse_command(&[
            "detect",
            "projects",
            "--path",
            "/tmp",
            "--max-depth",
            "3",
            "--include-guidelines",
            "true",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "detect projects");
        assert_eq!(args.get("path").unwrap(), "/tmp");
        assert_eq!(args.get("max_depth").unwrap(), 3);
        assert_eq!(args.get("include_guidelines").unwrap(), true);
    }

    #[test]
    fn test_lifecycle_commands_return_none() {
        assert!(build_args(&Commands::Serve).is_none());
        assert!(build_args(&Commands::Skill).is_none());
    }

    #[test]
    fn test_get_inbound_calls_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "inbound-calls",
            "--file-path",
            "f.rs",
            "--line",
            "3",
            "--character",
            "7",
            "--depth",
            "2",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get inbound_calls");
        assert_eq!(args.get("depth").unwrap(), 2);
    }

    #[test]
    fn test_get_type_definition_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "type-definition",
            "--file-path",
            "src/lib.rs",
            "--line",
            "5",
            "--character",
            "10",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get type_definition");
        assert_eq!(args.get("file_path").unwrap(), "src/lib.rs");
        assert_eq!(args.get("line").unwrap(), 5);
        assert_eq!(args.get("character").unwrap(), 10);
    }

    #[test]
    fn test_get_hover_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "hover",
            "--file-path",
            "lib.rs",
            "--line",
            "0",
            "--character",
            "0",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get hover");
        assert_eq!(args.get("file_path").unwrap(), "lib.rs");
    }

    #[test]
    fn test_get_implementations_builds_correct_args() {
        let cmd = parse_command(&[
            "get",
            "implementations",
            "--file-path",
            "f.rs",
            "--line",
            "1",
            "--character",
            "2",
            "--max-results",
            "10",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "get implementations");
        assert_eq!(args.get("max_results").unwrap(), 10);
    }

    #[test]
    fn test_search_workspace_symbol_builds_correct_args() {
        let cmd = parse_command(&[
            "search",
            "workspace-symbol",
            "--query",
            "Config",
            "--max-results",
            "25",
        ]);
        let args = build_args(&cmd).expect("should produce args");
        assert_eq!(args.get("op").unwrap(), "search workspace_symbol");
        assert_eq!(args.get("query").unwrap(), "Config");
        assert_eq!(args.get("max_results").unwrap(), 25);
    }
}
