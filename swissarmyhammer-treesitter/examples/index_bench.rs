//! Quick CLI to benchmark workspace indexing with progress display
//!
//! Usage: cargo run --release -p swissarmyhammer-treesitter --example index_bench /path/to/directory
//!
//! This example demonstrates:
//! - Using the builder pattern to configure a workspace
//! - Setting up a progress callback to monitor indexing
//! - Incremental indexing (run, kill, run again to see incremental behavior)

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use swissarmyhammer_treesitter::{IndexPhase, Workspace};
use tracing::info;

/// Log progress every N items to avoid flooding output
const PROGRESS_LOG_INTERVAL: usize = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with timestamps
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("index_bench=info".parse()?)
                .add_directive("swissarmyhammer_treesitter=info".parse()?)
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

    info!(directory = %dir.display(), "Starting index");

    // Track last printed progress to avoid flooding output
    let last_printed = Arc::new(AtomicUsize::new(0));
    let last_printed_clone = last_printed.clone();

    // Open workspace with progress callback using builder pattern
    let workspace = Workspace::new(&dir)
        .with_progress(move |status| {
            let processed = status.files_processed();
            let last = last_printed_clone.load(Ordering::Relaxed);

            // Log based on phase
            match status.phase {
                IndexPhase::Discovering => {
                    if status.files_skipped > 0 && status.files_skipped > last {
                        last_printed_clone.store(status.files_skipped, Ordering::Relaxed);
                        info!(
                            skipped = status.files_skipped,
                            current = ?status.current_file,
                            "Skipping unchanged"
                        );
                    }
                }
                IndexPhase::Parsing => {
                    if processed >= last + PROGRESS_LOG_INTERVAL || status.is_complete() {
                        last_printed_clone.store(processed, Ordering::Relaxed);
                        info!(
                            parsed = status.files_parsed,
                            total = status.files_total,
                            skipped = status.files_skipped,
                            errors = status.files_errored,
                            current = ?status.current_file,
                            "Parsing files"
                        );
                    }
                }
                IndexPhase::Embedding => {
                    if status.chunks_embedded >= last + PROGRESS_LOG_INTERVAL
                        || status.is_complete()
                    {
                        last_printed_clone.store(status.chunks_embedded, Ordering::Relaxed);
                        info!(
                            embedded = status.chunks_embedded,
                            total = status.chunks_total,
                            progress = format!("{:.1}%", status.progress().unwrap_or(0.0) * 100.0),
                            "Embedding chunks"
                        );
                    }
                }
                IndexPhase::Complete => {
                    info!(
                        parsed = status.files_parsed,
                        skipped = status.files_skipped,
                        "Build complete"
                    );
                }
                IndexPhase::Idle => {}
            }
        })
        .open()
        .await?;

    info!(is_leader = workspace.is_leader(), "Opened workspace");

    // Build the index - this is where the progress callback fires
    if workspace.is_leader() {
        info!("Building index");
        workspace.build().await?;
    } else {
        info!("Not leader - using existing index");
    }

    // Get final status
    let status = workspace.status().await?;

    info!(
        files_total = status.files_total,
        files_indexed = status.files_indexed,
        files_embedded = status.files_embedded,
        is_leader = workspace.is_leader(),
        database = %workspace.database_path().display(),
        "Indexing complete"
    );

    Ok(())
}
