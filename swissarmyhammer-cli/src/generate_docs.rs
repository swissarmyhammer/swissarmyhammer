//! Generate CLI documentation, man pages, and shell completions for `sah`.
//!
//! Run with: `cargo run --bin sah-generate-docs`

use std::path::Path;

use clap::CommandFactory;
use clap_complete::Shell;
use swissarmyhammer_cli::cli::Cli;

fn main() -> std::io::Result<()> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cmd = Cli::command();

    generate_markdown(&cmd, &repo_root.join("docs"))?;
    generate_manpage(&cmd, &repo_root.join("docs"))?;
    generate_completions(cmd, &repo_root.join("completions"))?;

    Ok(())
}

/// Generate markdown CLI reference.
fn generate_markdown(cmd: &clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let md = clap_markdown::help_markdown_command(cmd);
    let path = dir.join("sah-cli-reference.md");
    std::fs::write(&path, &md)?;
    println!("Generated {} ({} bytes)", path.display(), md.len());
    Ok(())
}

/// Generate ROFF man page.
fn generate_manpage(cmd: &clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let path = dir.join("sah.1");
    std::fs::write(&path, &buf)?;
    println!("Generated {} ({} bytes)", path.display(), buf.len());
    Ok(())
}

/// Generate shell completion scripts.
fn generate_completions(mut cmd: clap::Command, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        let path = clap_complete::generate_to(shell, &mut cmd, "sah", dir)?;
        println!("Generated {}", path.display());
    }
    Ok(())
}
