//! Build script for mirdan-cli.
//!
//! Generates CLI documentation, man pages, and shell completions from the
//! clap derive definitions in `src/cli.rs`. Output locations:
//!
//! - Markdown CLI reference -> doc/src/reference/ (mdbook source)
//! - Man pages -> docs/ (gitignored, included in release archives)
//! - Shell completions -> completions/

use std::path::Path;

use clap::CommandFactory;

#[path = "src/cli.rs"]
mod cli;

#[path = "../build-support/doc_gen.rs"]
mod doc_gen;

fn main() -> std::io::Result<()> {
    let cmd = cli::Cli::command();
    let repo_root = Path::new("..");

    doc_gen::generate_markdown_with_brew(
        &cmd,
        &repo_root.join("doc/src/reference"),
        "mirdan",
        Some("swissarmyhammer/tap/mirdan-cli"),
    )?;
    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "mirdan")?;
    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "mirdan")?;

    Ok(())
}
