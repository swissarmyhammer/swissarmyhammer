//! Build script for kanban-cli.
//!
//! Generates CLI documentation, man pages, and shell completions from the
//! clap derive definitions in `src/cli.rs`. Output locations:
//!
//! - Markdown CLI reference -> doc/src/reference/ (mdbook source)
//! - Man pages -> docs/ (gitignored, included in release archives)
//! - Shell completions -> completions/
//!
//! Only the lifecycle subcommands (serve/init/deinit/doctor) defined in
//! `src/cli.rs` appear in generated docs. The schema-driven noun/verb
//! commands are built dynamically in `main.rs` and are not visible to
//! clap's static introspection -- same trade-off as shelltool-cli.

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
        "kanban",
        Some("swissarmyhammer/tap/kanban-cli"),
    )?;

    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "kanban")?;

    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "kanban")?;

    Ok(())
}
