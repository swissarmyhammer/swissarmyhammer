//! Shared CLI documentation generation utilities.
//!
//! Used by both `avp-cli/build.rs` and `swissarmyhammer-cli/src/generate_docs.rs`
//! via `#[path = ...]` includes. Functions are parameterized by binary name to
//! support different CLI tools.

use std::io::{Error, ErrorKind};
use std::path::Path;

/// Wrap an io::Error with additional context.
fn io_context(msg: String, err: Error) -> Error {
    Error::new(ErrorKind::Other, format!("{msg}: {err}"))
}

/// Generate markdown CLI reference for mdbook.
pub fn generate_markdown(cmd: &clap::Command, dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let md = clap_markdown::help_markdown_command(cmd);
    let filename = format!("{name}-cli.md");
    let path = dir.join(&filename);
    std::fs::write(&path, &md)
        .map_err(|e| io_context(format!("failed to write {}", path.display()), e))?;
    eprintln!(
        "Generated {}/{filename} ({} bytes)",
        dir.display(),
        md.len()
    );
    Ok(())
}

/// Generate ROFF man page.
pub fn generate_manpage(cmd: &clap::Command, dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)
        .map_err(|e| io_context(format!("failed to render man page for {name}"), e))?;
    let filename = format!("{name}.1");
    let path = dir.join(&filename);
    std::fs::write(&path, &buf)
        .map_err(|e| io_context(format!("failed to write {}", path.display()), e))?;
    eprintln!(
        "Generated {}/{filename} ({} bytes)",
        dir.display(),
        buf.len()
    );
    Ok(())
}

/// Generate shell completion scripts.
pub fn generate_completions(mut cmd: clap::Command, dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    for shell in [
        clap_complete::Shell::Bash,
        clap_complete::Shell::Zsh,
        clap_complete::Shell::Fish,
    ] {
        let path = clap_complete::generate_to(shell, &mut cmd, name, dir)
            .map_err(|e| io_context(format!("failed to generate {shell:?} completions"), e))?;
        eprintln!("Generated {}", path.display());
    }
    Ok(())
}
