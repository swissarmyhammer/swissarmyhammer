//! Kanban CLI — standalone command-line interface for SwissArmyHammer Kanban.
//!
//! Exposes all kanban operations as direct subcommands (`kanban task add --title "foo"`)
//! and can open the GUI app via deep-link (`kanban .` or `kanban /path/to/project`).

mod banner;
mod cli_gen;
mod merge;

use clap::Command;
use serde_json::Value;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    // Build the clap command tree from kanban schema
    let operations = swissarmyhammer_kanban::schema::kanban_operations();
    let schema = swissarmyhammer_kanban::schema::generate_kanban_mcp_schema(operations);

    let subcommands = cli_gen::build_commands_from_schema(&schema);

    let mut cmd = Command::new("kanban")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Kanban board CLI — manage tasks, boards, and columns")
        .allow_external_subcommands(true);

    for subcmd in subcommands {
        cmd = cmd.subcommand(subcmd);
    }

    // Add the explicit "open" subcommand
    cmd = cmd.subcommand(
        Command::new("open")
            .about("Open the kanban GUI for a project")
            .arg(
                clap::Arg::new("path")
                    .help("Path to the project directory (default: current directory)")
                    .default_value("."),
            ),
    );

    // Add the merge subcommand (git merge drivers for .kanban/ files)
    cmd = cmd.subcommand(merge::merge_command());

    let matches = cmd.get_matches();

    match matches.subcommand() {
        Some(("open", sub_m)) => {
            let path = sub_m
                .get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            handle_open(path);
        }
        Some(("merge", sub_m)) => {
            let code = merge::handle_merge(sub_m);
            std::process::exit(code);
        }
        Some((name, sub_m)) => {
            // Check if it has a verb subcommand (noun-verb structure)
            if sub_m.subcommand().is_some() {
                handle_kanban_command(&matches, &schema);
            } else if looks_like_path(name) {
                // External subcommand that looks like a path
                handle_open(name);
            } else {
                eprintln!("Unknown command or missing verb: {}", name);
                eprintln!("Run 'kanban {} --help' or 'kanban --help' for usage.", name);
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("No command specified. Run 'kanban --help' for usage information.");
            std::process::exit(1);
        }
    }
}

fn handle_kanban_command(matches: &clap::ArgMatches, schema: &Value) {
    let arguments = match cli_gen::extract_noun_verb_arguments(matches, schema) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error: {}", e);
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
            eprintln!("Error parsing operation: {}", e);
            return 1;
        }
    };

    for op in &operations {
        match swissarmyhammer_kanban::dispatch::execute_operation(&ctx, op).await {
            Ok(result) => {
                let output =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                println!("{}", output);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
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
        eprintln!("Failed to open kanban app: {}", e);
        eprintln!("URL: {}", url);
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
