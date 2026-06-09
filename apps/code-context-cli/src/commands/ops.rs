//! MCP tool operation execution for code-context.
//!
//! The CLI operation commands (`get symbol`, `search code`, etc.) are generated
//! at runtime from the `CodeContextTool` full schema by
//! [`swissarmyhammer_operations::cli_gen`], and the parsed clap matches are
//! turned into a `{ "op": "verb noun", ...args }` JSON object by
//! [`swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments`]. This
//! module takes that argument map and dispatches it to
//! `CodeContextTool::execute`, printing the result.

use std::sync::Arc;

use rmcp::model::RawContent;
use serde_json::{Map, Value};
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;
use tokio::sync::Mutex;

/// How [`run_operation`] renders the tool result.
///
/// An explicit two-variant enum replaces a bare `bool` at the call site so the
/// intent (`OutputMode::Json` vs `OutputMode::Text`) is legible and cannot be
/// transposed with the unrelated [`Progress`] flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Print the full `CallToolResult` as pretty JSON.
    Json,
    /// Extract `Content::Text` items and print them one per line.
    Text,
}

/// Whether [`run_operation`] shows interactive progress chrome.
///
/// An explicit enum replaces a bare `bool` so the call site reads
/// `Progress::Suppressed` rather than an opaque `true`/`false` that could be
/// swapped with [`OutputMode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Drive the default `indicatif` renderer (auto-degrades on non-TTY).
    Shown,
    /// Install a null renderer so the tool emits no progress chrome.
    Suppressed,
}

/// Execute a code-context operation against the `CodeContextTool`.
///
/// Creates a minimal `ToolContext`, executes the tool with the supplied
/// argument map (a `{ "op": "verb noun", ...args }` object produced by
/// [`swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments`]), and
/// prints the result. Long-running ops (those that emit MCP
/// `notifications/progress`) drive a CLI-side renderer through an in-process
/// notification sink wired into the `ToolContext`.
///
/// # Arguments
///
/// * `args` - The `{ "op": ..., ...args }` argument map to dispatch.
/// * `output` - [`OutputMode::Json`] prints the full `CallToolResult` as JSON;
///   [`OutputMode::Text`] extracts text content and prints one item per line.
/// * `progress` - [`Progress::Suppressed`] installs a
///   [`crate::progress::NullRenderer`] so the tool emits no progress chrome
///   (intended for CI / piped scripts). [`Progress::Shown`] uses the default
///   [`crate::progress::IndicatifRenderer`] — `indicatif` auto-degrades to plain
///   output on non-TTY stdout, so it is safe to leave on by default.
///
/// # Returns
///
/// Exit code: 0 on success, 1 on error.
pub async fn run_operation(
    args: Map<String, Value>,
    output: OutputMode,
    progress: Progress,
) -> i32 {
    let tool = CodeContextTool::new();

    // Build the progress wiring up front so the renderer task is alive the
    // moment the tool starts emitting. We always wire a renderer — even for
    // ops that never emit progress — because the channel is cheap and the
    // renderer task exits cleanly the moment the sink drops at the end of
    // the call. `Progress::Suppressed` swaps the indicatif renderer for the
    // null renderer; the rest of the path is identical.
    let wiring = match progress {
        Progress::Suppressed => {
            crate::progress::build_progress_wiring(crate::progress::NullRenderer)
        }
        Progress::Shown => {
            crate::progress::build_progress_wiring(crate::progress::IndicatifRenderer::new())
        }
    };

    let context = {
        let tool_handlers = Arc::new(ToolHandlers::new());
        let git_ops = Arc::new(Mutex::new(None));
        let agent_config = Arc::new(ModelConfig::default());
        let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config)
            .with_progress_token(wiring.token.clone())
            .with_progress_sink(wiring.sink.clone());
        // Use cwd so the tool discovers the index in the current project
        ctx.working_dir = std::env::current_dir().ok();
        ctx
    };

    let result = tool.execute(args, &context).await;

    // Close the sink and wait for the renderer to flush before printing any
    // tool output. Without this the bar's final tick can race the result
    // line and corrupt terminal state. Dropping `wiring.sink` (the original
    // sender) is necessary because `with_progress_sink` cloned it into the
    // context — both senders must drop before the receiver returns `None`
    // and the renderer task can finish.
    drop(context);
    drop(wiring.sink);
    if let Err(err) = wiring.renderer_handle.await {
        tracing::debug!(error = ?err, "progress renderer task did not join cleanly");
    }

    let result = match result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return 1;
        }
    };

    match output {
        OutputMode::Json => match serde_json::to_string_pretty(&result) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Error serializing result: {e}");
                return 1;
            }
        },
        OutputMode::Text => {
            for content in &result.content {
                if let RawContent::Text(t) = &content.raw {
                    println!("{}", t.text);
                }
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
    use crate::commands::test_schema_full;
    use std::collections::HashSet;
    use swissarmyhammer_operations::cli_gen::build_commands_from_schema;
    use swissarmyhammer_operations::cli_gen::test_support::{collect_verb_noun_pairs, parse_argv};

    /// Build the runtime clap command tree and parse one invocation into the
    /// `{ "op": ..., ...args }` argument map, mirroring `main.rs::dispatch`.
    ///
    /// Delegates the build-then-extract round-trip to the shared
    /// `cli_gen::test_support::parse_argv` so the helper is not re-declared here.
    fn parse_args(argv: &[&str]) -> Map<String, Value> {
        parse_argv("code-context", &test_schema_full(), argv)
    }

    // -- Command-tree coverage: every op string is reachable --

    /// The generated noun/verb command tree must cover every operation the
    /// `CodeContextTool` exposes — no op may be dropped or renamed silently.
    #[test]
    fn generated_tree_covers_every_operation() {
        let tool = CodeContextTool::new();
        let schema = tool.schema_full();
        let commands = build_commands_from_schema(&schema);

        // Collect "verb noun" pairs the generated tree can produce.
        let generated = collect_verb_noun_pairs(&commands);

        // Every op the tool exposes must be present in the tree.
        for op in tool.operations() {
            let op_str = op.op_string();
            assert!(
                generated.contains(&op_str),
                "generated command tree missing operation: {op_str}"
            );
        }

        // And the tree must not invent ops the tool does not expose.
        let expected: HashSet<String> = tool.operations().iter().map(|op| op.op_string()).collect();
        assert_eq!(
            generated, expected,
            "generated command tree and tool operations diverge"
        );
    }

    // -- Argument extraction through the schema-built tree --

    // The shared generator builds a `noun → verb` tree, so `get status` is
    // invoked as `code-context status get` (and round-trips back to "get status").

    #[test]
    fn parses_get_status() {
        let args = parse_args(&["code-context", "status", "get"]);
        assert_eq!(args.get("op").unwrap(), "get status");
    }

    #[test]
    fn parses_get_symbol_with_max_results() {
        let args = parse_args(&[
            "code-context",
            "symbol",
            "get",
            "--query",
            "MyStruct::new",
            "--max_results",
            "5",
        ]);
        assert_eq!(args.get("op").unwrap(), "get symbol");
        assert_eq!(args.get("query").unwrap(), "MyStruct::new");
        assert_eq!(args.get("max_results").unwrap(), 5);
    }

    #[test]
    fn parses_grep_code_with_array() {
        let args = parse_args(&[
            "code-context",
            "code",
            "grep",
            "--pattern",
            "TODO|FIXME",
            "--language",
            "rs",
            "--max_results",
            "20",
        ]);
        assert_eq!(args.get("op").unwrap(), "grep code");
        assert_eq!(args.get("pattern").unwrap(), "TODO|FIXME");
        assert_eq!(args.get("language").unwrap(), &serde_json::json!(["rs"]));
        assert_eq!(args.get("max_results").unwrap(), 20);
    }

    #[test]
    fn parses_search_symbol() {
        let args = parse_args(&[
            "code-context",
            "symbol",
            "search",
            "--query",
            "handler",
            "--kind",
            "function",
        ]);
        assert_eq!(args.get("op").unwrap(), "search symbol");
        assert_eq!(args.get("query").unwrap(), "handler");
        assert_eq!(args.get("kind").unwrap(), "function");
    }

    // -- Integration tests for run_operation end-to-end --

    /// End-to-end test: `get status` with text output through the full pipeline.
    ///
    /// Verifies the complete path: schema-built command tree -> arg parsing ->
    /// extract_noun_verb_arguments -> ToolContext creation ->
    /// CodeContextTool::execute -> Content::Text extraction -> exit code. Uses a
    /// temporary directory as the working directory so the tool creates a fresh
    /// (empty) workspace and returns status with zero counts.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn run_operation_get_status() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(tmp.path()).unwrap();

        let args = parse_args(&["code-context", "status", "get"]);
        let exit_code = run_operation(args, OutputMode::Text, Progress::Suppressed).await;
        assert_eq!(
            exit_code, 0,
            "get status should succeed even with an empty index"
        );
    }

    /// End-to-end test: `get status` with JSON output mode.
    ///
    /// Verifies the JSON serialization path through run_operation: the result is
    /// serialized via `serde_json::to_string_pretty` and printed to stdout, and
    /// the exit code is 0.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn run_operation_get_status_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(tmp.path()).unwrap();

        let args = parse_args(&["code-context", "status", "get"]);
        let exit_code = run_operation(args, OutputMode::Json, Progress::Suppressed).await;
        assert_eq!(
            exit_code, 0,
            "get status with json output should succeed even with an empty index"
        );
    }

    /// An unknown operation reaches `execute` and returns a non-zero exit code.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn run_operation_unknown_op_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(tmp.path()).unwrap();

        let mut args = Map::new();
        args.insert("op".to_string(), Value::String("bogus op".to_string()));
        let exit_code = run_operation(args, OutputMode::Text, Progress::Suppressed).await;
        assert_eq!(exit_code, 1, "an unknown op should return exit code 1");
    }
}
