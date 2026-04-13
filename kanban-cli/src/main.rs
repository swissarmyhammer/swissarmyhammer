//! Kanban CLI — standalone command-line interface for SwissArmyHammer Kanban.
//!
//! Exposes all kanban operations as direct subcommands (`kanban task add --title "foo"`)
//! and can open the GUI app via deep-link (`kanban .` or `kanban /path/to/project`).

mod banner;
mod cli;
mod cli_gen;
mod commands;
mod logging;
mod merge;

use clap::Command;
use serde_json::Value;
use std::path::PathBuf;
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
    let schema = swissarmyhammer_kanban::schema::generate_kanban_mcp_schema(operations);
    let cmd = build_cli(&schema);
    let matches = cmd.get_matches();

    dispatch(&matches, &schema);
}

/// Build the clap command tree for `kanban`.
///
/// Combines the schema-driven noun/verb subcommands generated from the kanban
/// operation schema with the hand-rolled subcommands (`open`, `merge`,
/// `serve`, `init`, `deinit`, `doctor`). The lifecycle commands mirror the
/// surface declared in `cli.rs` (consumed by `build.rs` for doc/manpage/
/// completion generation).
fn build_cli(schema: &Value) -> Command {
    let mut cmd = Command::new("kanban")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Kanban board CLI — manage tasks, boards, and columns")
        .allow_external_subcommands(true);

    for subcmd in cli_gen::build_commands_from_schema(schema) {
        cmd = cmd.subcommand(subcmd);
    }
    cmd = cmd.subcommand(open_subcommand());
    cmd = cmd.subcommand(merge::merge_command());
    cmd = cmd.subcommand(serve_subcommand());
    cmd = cmd.subcommand(init_subcommand());
    cmd = cmd.subcommand(deinit_subcommand());
    cmd = cmd.subcommand(doctor_subcommand());
    cmd
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

/// `serve` — run the MCP server over stdio.
fn serve_subcommand() -> Command {
    Command::new("serve").about("Run MCP server over stdio, exposing kanban tools")
}

/// `init` — install the kanban MCP server into detected agent configs.
fn init_subcommand() -> Command {
    Command::new("init")
        .about("Install kanban MCP server into detected agent configs")
        .arg(install_target_arg(
            "Where to install the server configuration",
        ))
}

/// `deinit` — remove the kanban MCP server from detected agent configs.
fn deinit_subcommand() -> Command {
    Command::new("deinit")
        .about("Remove kanban MCP server from detected agent configs")
        .arg(install_target_arg(
            "Where to remove the server configuration from",
        ))
}

/// `doctor` — diagnose kanban setup with optional verbose output.
fn doctor_subcommand() -> Command {
    Command::new("doctor")
        .about("Diagnose kanban configuration and setup")
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show detailed output including fix suggestions")
                .action(clap::ArgAction::SetTrue),
        )
}

/// Shared `[TARGET]` positional argument used by both `init` and `deinit`.
///
/// Restricts inputs to the three valid lifecycle scopes and defaults to
/// `project` so a bare `kanban init` installs into the current project.
fn install_target_arg(help: &'static str) -> clap::Arg {
    clap::Arg::new("target")
        .help(help)
        .value_parser(["project", "local", "user"])
        .default_value("project")
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
        Some(("init", sub_m)) => std::process::exit(run_init(target_value(sub_m))),
        Some(("deinit", sub_m)) => std::process::exit(run_deinit(target_value(sub_m))),
        Some(("doctor", sub_m)) => {
            std::process::exit(commands::doctor::run_doctor(sub_m.get_flag("verbose")))
        }
        Some((name, sub_m)) => {
            if sub_m.subcommand().is_some() {
                handle_kanban_command(matches, schema);
            } else if looks_like_path(name) {
                handle_open(name);
                std::process::exit(0);
            } else {
                error!("Unknown command or missing verb: {}", name);
                error!("Run 'kanban {} --help' or 'kanban --help' for usage.", name);
                std::process::exit(1);
            }
            std::process::exit(0)
        }
        None => {
            error!("No command specified. Run 'kanban --help' for usage information.");
            std::process::exit(1);
        }
    }
}

/// Read the `--target` argument from a lifecycle subcommand's matches.
///
/// Falls back to `"project"` only when clap has stripped the default for
/// some reason — under normal use `default_value("project")` guarantees a
/// value is present.
fn target_value(matches: &clap::ArgMatches) -> &str {
    matches
        .get_one::<String>("target")
        .map(|s| s.as_str())
        .unwrap_or("project")
}

/// Map a `--target` string from the CLI to a lifecycle [`InitScope`].
///
/// The clap `value_parser` restricts inputs to `"project"`, `"local"`, and
/// `"user"`, so any other value is a programmer error — the function panics
/// rather than silently defaulting to `Project`.
fn target_to_scope(target: &str) -> InitScope {
    match target {
        "project" => InitScope::Project,
        "local" => InitScope::Local,
        "user" => InitScope::User,
        other => panic!("unexpected target value: {other}"),
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
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    match rt.block_on(commands::serve::run_serve()) {
        Ok(()) => 0,
        Err(e) => {
            error!("Error: {}", e);
            1
        }
    }
}

/// Run all registered init components for the given target scope.
///
/// Builds a fresh [`InitRegistry`] via [`commands::registry::register_all`],
/// runs every registered component, prints progress through [`CliReporter`],
/// and returns 0 on full success or 1 if any component reported an error.
fn run_init(target: &str) -> i32 {
    let scope = target_to_scope(target);
    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let reporter = CliReporter;
    let results = reg.run_all_init(&scope, &reporter);
    if any_init_error(&results) {
        1
    } else {
        0
    }
}

/// Run all registered deinit components for the given target scope.
///
/// Mirrors [`run_init`] but drives the registry's deinit path — components
/// run in reverse priority order so higher-priority steps (skill deployment)
/// unwind before lower-priority ones (MCP registration).
fn run_deinit(target: &str) -> i32 {
    let scope = target_to_scope(target);
    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let reporter = CliReporter;
    let results = reg.run_all_deinit(&scope, &reporter);
    if any_init_error(&results) {
        1
    } else {
        0
    }
}

fn handle_kanban_command(matches: &clap::ArgMatches, schema: &Value) {
    let arguments = match cli_gen::extract_noun_verb_arguments(matches, schema) {
        Ok(args) => args,
        Err(e) => {
            error!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
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
