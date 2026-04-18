//! CLI subcommands for the kanban app binary.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use swissarmyhammer_kanban::{
    board::{GetBoard, InitBoard},
    task::ListTasks,
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};

/// Top-level clap parser for the `kanban-app` binary. When `command` is
/// `None`, the binary launches the Tauri GUI; otherwise it dispatches to a
/// CLI subcommand defined by [`Command`].
#[derive(Parser)]
#[command(name = "kanban-app", about = "Kanban board desktop app and CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Hermetic-launch mode: open exactly the given board in exactly one
    /// window and disable UIState persistence. Skips session restore and
    /// auto-open of recent boards so the developer's real configuration is
    /// untouched. Primarily intended for integration tests (tauri-driver,
    /// scripted launches) that need a deterministic starting state.
    #[arg(long, value_name = "BOARD_PATH", global = true)]
    pub only: Option<PathBuf>,
}

/// CLI subcommands exposed by `kanban-app`. Most produce JSON on stdout and
/// exit; `Gui` and the `None` command variant fall through to GUI startup.
#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new kanban board in the current directory
    Init {
        /// Board name
        name: String,
    },
    /// Show board status
    Board,
    /// List tasks
    List {
        /// Filter by column
        #[arg(long)]
        column: Option<String>,
    },
    /// Launch the GUI (default when no subcommand)
    Gui,
    /// Write a deterministic 3x3 board fixture (3 columns, 3 tasks per column)
    /// into the directory at `path`, then exit.
    ///
    /// This subcommand is debug-only — release builds omit it entirely. It
    /// exists so the tauri-driver E2E harness can materialise a board on
    /// disk from a Node script without replicating the kanban storage
    /// format outside of Rust. The written fixture is the exact shape the
    /// `--only <path>` hermetic launch mode expects.
    ///
    /// Prints a single JSON line on stdout with `{ "path": ..., "tasks": [...] }`
    /// so the caller can learn the task ids without re-parsing the fixture.
    #[cfg(debug_assertions)]
    #[command(name = "fixture-3x3", hide = true)]
    Fixture3x3 {
        /// Directory that will receive the `.kanban/` subtree. Must exist
        /// and be empty (or at least not already contain a `.kanban`).
        path: PathBuf,
    },
}

/// Run the CLI subcommand. Returns true if a CLI command was handled.
pub async fn run_cli(cli: &Cli) -> bool {
    let cmd = match &cli.command {
        Some(cmd) => cmd,
        None => return false, // No subcommand → launch GUI
    };

    match cmd {
        Command::Gui => return false, // Explicit gui → launch GUI
        Command::Init { name } => handle_init(name).await,
        Command::Board => handle_board().await,
        Command::List { column } => handle_list(column.as_deref()).await,
        #[cfg(debug_assertions)]
        Command::Fixture3x3 { path } => handle_fixture_3x3(path).await,
    }

    true
}

/// Build a deterministic 3x3 board fixture at the given directory path and
/// print a JSON manifest describing it to stdout.
///
/// The handler is debug-only — the subcommand itself is gated the same way,
/// so release builds never pull this code in. Exits with status 1 on failure
/// (bad path, already-populated directory, InitBoard error) so shell scripts
/// can short-circuit.
#[cfg(debug_assertions)]
async fn handle_fixture_3x3(path: &std::path::Path) {
    // `build_fixture` treats `path` as the board-root that will contain a
    // fresh `.kanban/` directory. Create it if it doesn't exist so Node
    // callers don't have to `mkdir -p` themselves; fail loudly if the
    // `.kanban/` dir would collide.
    if let Err(e) = std::fs::create_dir_all(path) {
        eprintln!("Error creating fixture root {}: {}", path.display(), e);
        std::process::exit(1);
    }
    if path.join(".kanban").exists() {
        eprintln!(
            "Error: {} already contains a .kanban directory; pass a fresh path",
            path.display()
        );
        std::process::exit(1);
    }

    let fixture = crate::test_support::build_fixture(path.to_path_buf(), 3, 3).await;
    let manifest = serde_json::json!({
        "path": fixture.path,
        "tasks": fixture.tasks,
    });
    println!("{}", serde_json::to_string(&manifest).unwrap());
}

async fn handle_init(name: &str) {
    let cwd = std::env::current_dir().expect("Cannot determine current directory");
    let kanban_path = cwd.join(".kanban");
    let ctx = KanbanContext::new(&kanban_path);
    let processor = KanbanOperationProcessor::new();

    match processor.process(&InitBoard::new(name), &ctx).await {
        Ok(result) => {
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn handle_board() {
    let cwd = std::env::current_dir().expect("Cannot determine current directory");
    let ctx = match KanbanContext::find(&cwd) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let processor = KanbanOperationProcessor::new();

    match processor.process(&GetBoard::default(), &ctx).await {
        Ok(result) => {
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn handle_list(column: Option<&str>) {
    let cwd = std::env::current_dir().expect("Cannot determine current directory");
    let ctx = match KanbanContext::find(&cwd) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let processor = KanbanOperationProcessor::new();

    let mut cmd = ListTasks::new();
    if let Some(col) = column {
        cmd.column = Some(swissarmyhammer_kanban::ColumnId::from_string(col));
    }

    match processor.process(&cmd, &ctx).await {
        Ok(result) => {
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// `--only <path>` parses into `Cli::only` and leaves `command` empty
    /// so the binary falls through to GUI startup. This is the contract
    /// integration tests and tauri-driver launches rely on.
    #[test]
    fn parse_only_flag_without_subcommand() {
        let cli = Cli::parse_from(["kanban-app", "--only", "/tmp/fixture.kanban"]);
        assert_eq!(cli.only, Some(PathBuf::from("/tmp/fixture.kanban")));
        assert!(cli.command.is_none());
    }

    /// Absence of `--only` leaves `Cli::only` as `None`, so `AppState::new()`
    /// runs and the normal auto-open + session-restore sequence fires.
    #[test]
    fn parse_without_only_flag() {
        let cli = Cli::parse_from(["kanban-app"]);
        assert_eq!(cli.only, None);
    }

    /// `--only` is declared `global = true` so it works alongside any
    /// subcommand — e.g. hypothetical future `kanban-app gui --only <path>`.
    /// Verify the parser accepts it after a subcommand too.
    #[test]
    fn parse_only_flag_with_gui_subcommand() {
        let cli = Cli::parse_from(["kanban-app", "gui", "--only", "/tmp/fixture.kanban"]);
        assert_eq!(cli.only, Some(PathBuf::from("/tmp/fixture.kanban")));
        assert!(matches!(cli.command, Some(Command::Gui)));
    }

    /// The hidden `fixture-3x3 <path>` subcommand parses its positional
    /// path argument into a `PathBuf`. The E2E harness relies on this
    /// exact contract — its `e2e/setup.ts` spawns the binary with these
    /// two positional args and parses the JSON line from stdout.
    ///
    /// Debug-only: the variant is gated by `#[cfg(debug_assertions)]`.
    #[cfg(debug_assertions)]
    #[test]
    fn parse_fixture_3x3_subcommand_captures_path() {
        let cli = Cli::parse_from(["kanban-app", "fixture-3x3", "/tmp/e2e-board"]);
        match cli.command {
            Some(Command::Fixture3x3 { path }) => {
                assert_eq!(path, PathBuf::from("/tmp/e2e-board"));
            }
            // `Command` does not derive Debug (it owns heavy subcommand state),
            // so we print whether *any* command was parsed and the CLI-level
            // `only` field to help track down parser drift.
            other => panic!(
                "expected Fixture3x3 variant (command_present={}, only={:?})",
                other.is_some(),
                cli.only,
            ),
        }
    }
}
