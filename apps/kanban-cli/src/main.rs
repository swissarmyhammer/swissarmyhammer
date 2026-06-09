//! Kanban CLI — standalone command-line interface for SwissArmyHammer Kanban.
//!
//! Exposes all kanban operations as direct subcommands (`kanban task add --title "foo"`)
//! and can open the GUI app via deep-link (`kanban .` or `kanban /path/to/project`).

mod banner;
mod cli;
mod commands;
mod logging;
mod merge;

use clap::Command;
use serde_json::Value;
use std::path::PathBuf;
use swissarmyhammer_cli_completions::lifecycle;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, InitStatus};
use swissarmyhammer_common::reporter::CliReporter;
use tracing::error;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    // TODO: wire a `--debug` / `-v` CLI flag through the clap tree and pass it
    // here. The `debug` parameter is intentionally hardcoded to `false` until
    // that flag lands in a follow-up card.
    logging::init_tracing(false);

    let operations = swissarmyhammer_kanban::schema::kanban_operations();
    // The CLI command tree is built in-process and needs the FULL schema
    // (per-op `x-operation-schemas` + flat properties), not the slim wire form.
    let schema = swissarmyhammer_kanban::schema::generate_kanban_mcp_schema_full(operations);
    let cmd = build_cli(&schema);
    let matches = cmd.get_matches();

    dispatch(&matches, &schema);
}

/// The program name — used for the clap root command, completion target, and
/// completion dispatch. Sourced once so a rename can't desync the three sites.
const PROGRAM: &str = "kanban";

/// Build the clap command tree for `kanban`.
///
/// Starts from the shared [`lifecycle::standard_op_cli`] skeleton (root command +
/// global `--debug` + schema-driven noun/verb subcommands + the five lifecycle
/// subcommands serve/init/deinit/doctor/completion) and appends kanban's own
/// app-specific subcommands `open` and `merge`. The lifecycle surface mirrors the
/// static `cli.rs` definition consumed by `build.rs`.
fn build_cli(schema: &Value) -> Command {
    lifecycle::standard_op_cli(
        PROGRAM,
        "Kanban board CLI — manage tasks, boards, and columns",
        schema,
    )
    .subcommand(open_subcommand())
    .subcommand(merge::merge_command())
}

/// `open` — launch the GUI app via deep-link for a project directory.
fn open_subcommand() -> Command {
    Command::new("open")
        .about("Open the kanban GUI for a project")
        .arg(
            clap::Arg::new("path")
                .help("Path to the project directory (default: current directory)")
                .default_value("."),
        )
}

/// Route the parsed CLI invocation to the correct handler and exit.
///
/// Lifecycle subcommands (`serve`/`init`/`deinit`/`doctor`) and `merge`
/// terminate the process with their own exit code. `open` invokes the GUI
/// deep-link and returns. Any other subcommand is treated as either a
/// schema-driven noun/verb operation (when it has its own subcommand) or a
/// path argument to `open` (when it looks like a path).
fn dispatch(matches: &clap::ArgMatches, schema: &Value) -> ! {
    match matches.subcommand() {
        Some(("open", sub_m)) => {
            let path = sub_m
                .get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            handle_open(path);
            std::process::exit(0);
        }
        Some(("merge", sub_m)) => std::process::exit(merge::handle_merge(sub_m)),
        Some(("serve", _)) => std::process::exit(run_serve()),
        Some(("init", sub_m)) => std::process::exit(run_init(lifecycle::target_scope(sub_m))),
        Some(("deinit", sub_m)) => std::process::exit(run_deinit(lifecycle::target_scope(sub_m))),
        Some(("doctor", sub_m)) => {
            std::process::exit(commands::doctor::run_doctor(sub_m.get_flag("verbose")))
        }
        Some(("completion", sub_m)) => {
            std::process::exit(lifecycle::run_completion(build_cli(schema), PROGRAM, sub_m))
        }
        Some((name, sub_m)) => {
            if sub_m.subcommand().is_some() {
                handle_kanban_command(matches, schema);
            } else if looks_like_path(name) {
                handle_open(name);
                std::process::exit(0);
            } else {
                error!("Unknown command or missing verb: {}", name);
                error!("Run '{PROGRAM} {name} --help' or '{PROGRAM} --help' for usage.");
                std::process::exit(1);
            }
            std::process::exit(0)
        }
        None => {
            error!("No command specified. Run '{PROGRAM} --help' for usage information.");
            std::process::exit(1);
        }
    }
}

/// Return `true` if any [`InitResult`] has `Error` status.
///
/// Used to translate the per-component registry results into a top-level
/// process exit code — a single errored component demotes the whole run
/// to exit 1.
fn any_init_error(results: &[InitResult]) -> bool {
    results.iter().any(|r| r.status == InitStatus::Error)
}

/// Run the MCP `serve` loop and return a process exit code.
///
/// Constructs a fresh tokio runtime — the top-level `fn main` is
/// synchronous, matching the existing `execute_kanban_operation` pattern —
/// and blocks on [`commands::serve::run_serve`] until the MCP client
/// disconnects. Returns 0 on clean shutdown, 1 on fatal error.
fn run_serve() -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime: {}", e);
            return 1;
        }
    };
    match rt.block_on(commands::serve::run_serve()) {
        Ok(()) => 0,
        Err(e) => {
            error!("Error: {}", e);
            1
        }
    }
}

/// Install kanban for the given target scope.
///
/// Runs the mirdan profile installer (registers the `kanban` MCP server and
/// deploys the `kanban`-profile builtin skills) followed by the genuine
/// tool-lifecycle components (`.kanban/` git merge drivers). Prints progress
/// through [`CliReporter`] and returns 0 on full success or 1 if any result
/// reported an error.
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

    if any_init_error(&results) {
        1
    } else {
        0
    }
}

/// Remove kanban for the given target scope.
///
/// Mirrors [`run_init`]: deinits the genuine tool-lifecycle components, then
/// runs the mirdan profile deinstaller (unregisters the MCP server and removes
/// the `kanban`-profile skills).
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

    if any_init_error(&results) {
        1
    } else {
        0
    }
}

fn handle_kanban_command(matches: &clap::ArgMatches, schema: &Value) {
    let arguments =
        match swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments(matches, schema) {
            Ok(args) => args,
            Err(e) => {
                error!("Error: {}", e);
                std::process::exit(1);
            }
        };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime: {}", e);
            std::process::exit(1);
        }
    };
    let exit_code = rt.block_on(execute_kanban_operation(arguments));
    std::process::exit(exit_code);
}

async fn execute_kanban_operation(arguments: serde_json::Map<String, Value>) -> i32 {
    let cwd = std::env::current_dir().expect("Cannot determine current directory");
    let kanban_dir = cwd.join(".kanban");
    let ctx = swissarmyhammer_kanban::KanbanContext::new(kanban_dir);

    let input = Value::Object(arguments);
    let operations = match swissarmyhammer_kanban::parse::parse_input(input) {
        Ok(ops) => ops,
        Err(e) => {
            error!("Error parsing operation: {}", e);
            return 1;
        }
    };

    for op in &operations {
        match swissarmyhammer_kanban::dispatch::execute_operation(&ctx, op).await {
            Ok(result) => {
                let output =
                    serde_yaml_ng::to_string(&result).unwrap_or_else(|_| result.to_string());
                println!("{}", output);
            }
            Err(e) => {
                error!("Error: {}", e);
                return 1;
            }
        }
    }

    0
}

fn looks_like_path(s: &str) -> bool {
    s == "."
        || s == ".."
        || s.starts_with('/')
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('~')
}

fn handle_open(path: &str) {
    let abs_path = resolve_path(path);
    let path_str = abs_path.to_string_lossy();
    let encoded = urlencoding::encode(&path_str);
    let url = format!("kanban://open/{}", encoded);

    if let Err(e) = open::that(&url) {
        error!("Failed to open kanban app: {}", e);
        error!("URL: {}", url);
        std::process::exit(1);
    }
}

fn resolve_path(path: &str) -> PathBuf {
    if path == "." || path == ".." || path.starts_with("./") || path.starts_with("../") {
        let cwd = std::env::current_dir().expect("Cannot determine current directory");
        let resolved = cwd.join(path);
        resolved.canonicalize().unwrap_or(resolved)
    } else if path.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            home.join(path.strip_prefix("~/").unwrap_or(path))
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    }
}
