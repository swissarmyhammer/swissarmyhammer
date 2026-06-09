//! Schema-driven shell operation dispatch for the `shelltool` CLI.
//!
//! The op subcommand tree (`execute command`, `list processes`, `grep history`,
//! `get lines`, `kill process`) is built at runtime in `main.rs` from
//! [`ShellExecuteTool`]'s full schema via
//! [`swissarmyhammer_operations::cli_gen::build_commands_from_schema`]. Once clap
//! has matched a noun/verb invocation,
//! [`swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments`] turns the
//! matches into a `{ "op": "verb noun", ...args }` JSON object. This module is the
//! final hop: it hands that object to [`ShellExecuteTool::execute`] and prints the
//! result.

use std::sync::Arc;

use rmcp::model::RawContent;
use serde_json::{Map, Value};
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;
use tokio::sync::Mutex;

/// Execute a schema-extracted shell operation against [`ShellExecuteTool`].
///
/// `arguments` is the `{ "op": "verb noun", ...args }` map produced by
/// [`swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments`]. A fresh
/// [`ShellExecuteTool`] (in-memory state) and a minimal [`ToolContext`] — the same
/// shape the serve path builds — are created, the tool is executed, and each
/// `Content::Text` item is printed to stdout one per line.
///
/// # Returns
///
/// Exit code: 0 on success, 1 when the tool returns an `McpError` or a result
/// flagged `is_error`.
pub async fn run_operation(arguments: Map<String, Value>) -> i32 {
    let tool = ShellExecuteTool::new();
    let context = ToolContext::new(
        Arc::new(ToolHandlers::new()),
        Arc::new(Mutex::new(None)),
        Arc::new(ModelConfig::default()),
    );

    let result = match tool.execute(arguments, &context).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return 1;
        }
    };

    for content in &result.content {
        if let RawContent::Text(t) = &content.raw {
            println!("{}", t.text);
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
    use swissarmyhammer_operations::cli_gen::build_commands_from_schema;
    use swissarmyhammer_operations::cli_gen::test_support::{collect_verb_noun_pairs, parse_argv};

    /// Build the full shell schema the runtime command tree is generated from.
    ///
    /// Mirrors `main.rs`: the in-process command tree consumes the FULL schema
    /// (per-op `x-operation-schemas` + flat properties), so these tests use
    /// `schema_full()` rather than the slim wire schema.
    fn shell_schema() -> Value {
        ShellExecuteTool::new().schema_full()
    }

    /// Parse an argv slice through the schema-built tree and extract the args.
    ///
    /// Delegates the build-then-extract round-trip to the shared
    /// `cli_gen::test_support::parse_argv` so the helper is not re-declared here.
    fn parse_and_extract(argv: &[&str]) -> Map<String, Value> {
        parse_argv("shelltool", &shell_schema(), argv)
    }

    /// Every `SHELL_OPERATIONS` op string must appear as a `noun → verb` pair in
    /// the generated command tree.
    ///
    /// The expected set is DERIVED from the canonical `SHELL_OPERATIONS` table
    /// (the same source the schema is generated from), not a hardcoded list —
    /// so adding an operation is covered mechanically without editing this test.
    #[test]
    fn command_tree_covers_all_operations() {
        use std::collections::HashSet;
        use swissarmyhammer_tools::mcp::tools::shell::SHELL_OPERATIONS;

        let schema = shell_schema();
        let commands = build_commands_from_schema(&schema);

        // Collect "verb noun" strings from the generated noun → verb tree.
        let generated = collect_verb_noun_pairs(&commands);

        // The tree must cover every op in the canonical table, and invent none.
        let expected: HashSet<String> = SHELL_OPERATIONS.iter().map(|op| op.op_string()).collect();
        assert_eq!(
            generated, expected,
            "generated command tree and SHELL_OPERATIONS diverge"
        );
    }

    /// The schema groups ops by noun (the second token of `"verb noun"`), so the
    /// `execute command` op surfaces as `command execute` on the CLI. Parsing it
    /// must round-trip into `{ "op": "execute command", "command": "..." }`.
    #[test]
    fn execute_command_extracts_args() {
        let args = parse_and_extract(&["shelltool", "command", "execute", "--command", "echo hi"]);
        assert_eq!(args.get("op").unwrap(), "execute command");
        assert_eq!(args.get("command").unwrap(), "echo hi");
    }

    /// End-to-end: a generated `execute command` invocation (`command execute`
    /// on the CLI) reaches `ShellExecuteTool::execute` and returns success.
    #[tokio::test]
    async fn run_operation_execute_command_succeeds() {
        let args = parse_and_extract(&[
            "shelltool",
            "command",
            "execute",
            "--command",
            "echo run_operation_test",
        ]);
        let exit_code = run_operation(args).await;
        assert_eq!(exit_code, 0, "execute command should succeed");
    }

    /// End-to-end: `list processes` (`processes list` on the CLI) reaches the
    /// tool and returns success even with no prior commands run.
    #[tokio::test]
    async fn run_operation_list_processes_succeeds() {
        let args = parse_and_extract(&["shelltool", "processes", "list"]);
        let exit_code = run_operation(args).await;
        assert_eq!(exit_code, 0, "list processes should succeed");
    }
}
