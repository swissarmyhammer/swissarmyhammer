//! Build script for avp-cli.
//!
//! Generates CLI documentation, man pages, and shell completions from the
//! clap derive definitions in `src/cli.rs`. Output is written to the repo
//! root so generated files can be committed and referenced as stable URLs.

use std::path::Path;

use clap::CommandFactory;
use clap_complete::Shell;

// Compile cli.rs independently — it only depends on clap and std.
#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    let cmd = cli::Cli::command();

    // Write to repo root (build.rs runs from the crate directory)
    let docs_dir = Path::new("..").join("docs");
    let completions_dir = Path::new("..").join("completions");

    generate_markdown(&cmd, &docs_dir)?;
    generate_manpage(&cmd, &docs_dir)?;
    generate_completions(cmd, &completions_dir)?;

    Ok(())
}

/// Generate markdown CLI reference.
fn generate_markdown(cmd: &clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let md = clap_markdown::help_markdown_command(cmd);
    std::fs::write(dir.join("avp-cli-reference.md"), md)?;
    Ok(())
}

/// Generate ROFF man page.
fn generate_manpage(cmd: &clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    std::fs::write(dir.join("avp.1"), buf)?;
    Ok(())
}

/// Generate shell completion scripts.
fn generate_completions(mut cmd: clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        clap_complete::generate_to(shell, &mut cmd, "avp", dir)?;
    }
    Ok(())
}
