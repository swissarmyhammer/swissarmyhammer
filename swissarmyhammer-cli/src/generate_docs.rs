//! Generate CLI documentation, man pages, and shell completions for `sah`.
//!
//! Run with: `cargo run --bin sah-generate-docs`
//!
//! Output locations:
//! - Markdown CLI reference → doc/src/reference/ (mdbook source)
//! - Man pages → docs/ (gitignored, included in release archives)
//! - Shell completions → completions/

use std::path::Path;

use clap::CommandFactory;
use swissarmyhammer_cli::cli::Cli;

#[path = "../../build-support/doc_gen.rs"]
mod doc_gen;

fn main() -> std::io::Result<()> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cmd = Cli::command();

    doc_gen::generate_markdown(&cmd, &repo_root.join("doc/src/reference"), "sah")?;
    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "sah")?;
    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "sah")?;

    Ok(())
}
