//! Migrate .kanban storage from JSON to YAML/Markdown format.
//!
//! Usage:
//!   cargo run -p swissarmyhammer-kanban --example migrate_storage -- /path/to/.kanban
//!
//! If no path is given, searches upward from the current directory.

use swissarmyhammer_kanban::KanbanContext;

#[tokio::main]
async fn main() {
    let ctx = match std::env::args().nth(1) {
        Some(path) => KanbanContext::new(path),
        None => {
            let cwd = std::env::current_dir().expect("cannot read current directory");
            KanbanContext::find(&cwd).expect("no .kanban directory found")
        }
    };

    eprintln!("Migrating storage at: {}", ctx.root().display());

    match ctx.migrate_storage().await {
        Ok(stats) => {
            eprintln!("{}", stats);
            if stats.total() == 0 {
                eprintln!("Nothing to migrate — all files are already in the new format.");
            } else {
                eprintln!("Migration complete.");
            }
        }
        Err(e) => {
            eprintln!("Migration failed: {}", e);
            std::process::exit(1);
        }
    }
}
