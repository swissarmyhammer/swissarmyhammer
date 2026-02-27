//! CLI subcommands for the kanban app binary.

use clap::{Parser, Subcommand};
use swissarmyhammer_kanban::{
    board::{GetBoard, InitBoard},
    task::ListTasks,
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};

#[derive(Parser)]
#[command(name = "swissarmyhammer-kanban-app", about = "Kanban board desktop app and CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

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
    }

    true
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
