//! code-context CLI -- Standalone MCP code-context tool for AI coding agents.
//!
//! Commands:
//! - `code-context serve`: Run MCP server over stdio, exposing code-context tools
//! - `code-context init [target]`: Install code-context into Claude Code settings
//! - `code-context deinit [target]`: Remove code-context from Claude Code settings
//! - `code-context doctor`: Diagnose code-context setup
//! - `code-context skill`: Deploy code-context skill to agent .skills/ directories
//! - `code-context completion <shell>`: Generate shell completion scripts
//! - `code-context <noun> <verb> ...`: Operations (e.g. `code-context get symbol`),
//!   generated at runtime from the `CodeContextTool` schema.
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error
//!
//! The operation command tree is built in-process from the `CodeContextTool`
//! FULL schema via [`swissarmyhammer_operations::cli_gen`]. The static lifecycle
//! commands declared in `cli.rs` (Serve/Init/Deinit/Doctor/Skill/Completion)
//! exist only for `build.rs` doc/manpage/completion generation; their runtime
//! clap surface is rebuilt here as builder subcommands so the two stay aligned.

use clap::{ArgMatches, Command};
use serde_json::Value;
use swissarmyhammer_cli_completions::lifecycle;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;
use swissarmyhammer_tools::mcp::tool_registry::McpTool;
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;

mod banner;
mod cli;
mod commands;
mod logging;
mod progress;

/// The program name — used for the clap root command, completion target, and
/// completion dispatch. Sourced once so a rename can't desync the three sites.
/// (`CARGO_PKG_NAME` is `code-context-cli`, so a const is needed.)
const PROGRAM_NAME: &str = "code-context";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    // The CLI command tree is built in-process and needs the FULL schema
    // (per-op `x-operation-schemas` + flat properties), not the slim wire form
    // the tool serves to models.
    let schema = CodeContextTool::new().schema_full();
    let cmd = build_cli(&schema);
    let matches = cmd.get_matches();

    logging::init_tracing(matches.get_flag("debug"));

    let exit_code = dispatch(&matches, &schema).await;
    std::process::exit(exit_code);
}

/// Build the clap command tree for `code-context`.
///
/// Starts from the shared [`lifecycle::standard_op_cli`] skeleton (root command +
/// global `--debug` + schema-driven noun/verb operation subcommands + the five
/// lifecycle subcommands serve/init/deinit/doctor/completion), then appends
/// code-context's own pieces: the extra global flags `--json`/`--no-progress`
/// and the `skill` subcommand. The lifecycle commands mirror the surface declared
/// in `cli.rs` (consumed by `build.rs`).
fn build_cli(schema: &Value) -> Command {
    lifecycle::standard_op_cli(
        PROGRAM_NAME,
        "Structural code intelligence for AI coding agents",
        schema,
    )
    .arg(lifecycle::global_flag(
        "json",
        "json",
        Some('j'),
        "Output results as JSON (for operation commands)",
    ))
    .arg(lifecycle::global_flag(
        "no_progress",
        "no-progress",
        None,
        "Disable interactive progress bars for long-running operations",
    ))
    .subcommand(
        Command::new("skill").about("Deploy code-context skill to agent .skills/ directories"),
    )
}

/// Route the parsed CLI invocation to the correct handler and return an exit code.
///
/// Lifecycle subcommands (`serve`/`init`/`deinit`/`doctor`/`skill`/`completion`)
/// are matched by name. Any other subcommand is a schema-driven noun/verb
/// operation: its matches are turned into a `{ "op": ..., ...args }` object by
/// [`swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments`] and
/// dispatched to [`commands::ops::run_operation`].
async fn dispatch(matches: &ArgMatches, schema: &Value) -> i32 {
    use commands::ops::{OutputMode, Progress};
    let output = if matches.get_flag("json") {
        OutputMode::Json
    } else {
        OutputMode::Text
    };
    let progress = if matches.get_flag("no_progress") {
        Progress::Suppressed
    } else {
        Progress::Shown
    };

    match matches.subcommand() {
        Some(("serve", _)) => match commands::serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e:#}");
                1
            }
        },
        Some(("init", sub_m)) => run_init(lifecycle::target_scope(sub_m)),
        Some(("deinit", sub_m)) => run_deinit(lifecycle::target_scope(sub_m)),
        Some(("doctor", sub_m)) => commands::doctor::run_doctor(sub_m.get_flag("verbose")).await,
        Some(("skill", _)) => commands::skill::run_skill(),
        Some(("completion", sub_m)) => {
            lifecycle::run_completion(build_cli(schema), PROGRAM_NAME, sub_m)
        }
        Some(_) => {
            match swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments(matches, schema)
            {
                Ok(args) => commands::ops::run_operation(args, output, progress).await,
                Err(e) => {
                    eprintln!("Error: {e:#}");
                    1
                }
            }
        }
        None => {
            eprintln!("No command specified. Run '{PROGRAM_NAME} --help' for usage information.");
            1
        }
    }
}

/// Return `true` if any `InitResult` has `Error` status.
fn any_init_error(results: &[swissarmyhammer_common::lifecycle::InitResult]) -> bool {
    results
        .iter()
        .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error)
}

/// Install code-context for the given scope and return the exit code.
///
/// Runs the mirdan profile installer (registers the `code-context` MCP server —
/// strategy-aware, so it handles Claude local scope the old hand-rolled loop
/// dropped — and deploys the `code-context` + `explore` + `lsp` +
/// `detected-projects` skills) followed by the genuine tool-lifecycle components
/// (the `.code-context/` directory + `.gitignore`). A single errored result from
/// either phase demotes the run to exit code 1.
fn run_init(scope: InitScope) -> i32 {
    let reporter = CliReporter;

    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let results = mirdan::install::init_profile_with_registry(
        &commands::registry::profile(scope),
        &reg,
        scope,
        None,
        &reporter,
    );

    i32::from(any_init_error(&results))
}

/// Remove code-context for the given scope and return the exit code.
///
/// Mirrors [`run_init`]: deinits the genuine tool-lifecycle components, then runs
/// the mirdan profile deinstaller (unregisters the MCP server and removes the
/// `code-context` + `explore` + `lsp` + `detected-projects` skills).
fn run_deinit(scope: InitScope) -> i32 {
    let reporter = CliReporter;

    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let results = mirdan::install::deinit_profile_with_registry(
        &commands::registry::profile(scope),
        &reg,
        scope,
        None,
        &reporter,
    );

    i32::from(any_init_error(&results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use commands::test_schema_full as schema;

    /// The built command tree must expose every lifecycle command.
    #[test]
    fn build_cli_has_lifecycle_commands() {
        let cmd = build_cli(&schema());
        let names: HashSet<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        for name in ["serve", "init", "deinit", "doctor", "skill", "completion"] {
            assert!(names.contains(name), "build_cli missing command: {name}");
        }
    }

    /// The built command tree must expose every code_context operation as a
    /// reachable `noun verb` path — the generated tree may not drop any op.
    #[test]
    fn build_cli_covers_every_operation() {
        let tool = CodeContextTool::new();
        let cmd = build_cli(&tool.schema_full());

        let generated = swissarmyhammer_operations::cli_gen::test_support::collect_verb_noun_pairs(
            cmd.get_subcommands(),
        );

        for op in tool.operations() {
            let op_str = op.op_string();
            assert!(
                generated.contains(&op_str),
                "build_cli missing operation: {op_str}"
            );
        }
    }

    /// Global flags must be parsed regardless of subcommand position.
    #[test]
    fn build_cli_parses_global_flags() {
        let cmd = build_cli(&schema());
        let matches = cmd
            .try_get_matches_from(["code-context", "--json", "--no-progress", "status", "get"])
            .unwrap();
        assert!(matches.get_flag("json"));
        assert!(matches.get_flag("no_progress"));
    }
}
