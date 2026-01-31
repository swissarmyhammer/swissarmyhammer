//! Quick CLI to benchmark workspace indexing
//!
//! Usage: cargo run --release -p swissarmyhammer-treesitter --example index_bench /path/to/directory

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use swissarmyhammer_treesitter::Workspace;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber for console output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("swissarmyhammer_treesitter=debug".parse()?)
                .add_directive("llama_embedding=info".parse()?)
                .add_directive("llama_loader=info".parse()?),
        )
        .with_target(true)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        std::process::exit(1);
    }

    let dir = PathBuf::from(&args[1]);
    if !dir.exists() {
        eprintln!("Directory does not exist: {}", dir.display());
        std::process::exit(1);
    }

    println!("Indexing directory: {}", dir.display());
    println!("---");

    let start = Instant::now();

    // Open workspace - this will scan and index
    let workspace = Workspace::open(&dir).await?;

    let elapsed = start.elapsed();

    // Get status
    let status = workspace.status().await?;

    println!("---");
    println!("Indexing complete!");
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!("  Files discovered: {}", status.files_total);
    println!("  Files indexed: {}", status.files_indexed);
    println!("  Files embedded: {}", status.files_embedded);
    println!("  Is leader: {}", workspace.is_leader());

    Ok(())
}
