//! Shared CLI documentation generation utilities.
//!
//! Used by both `avp-cli/build.rs` and `swissarmyhammer-cli/src/generate_docs.rs`
//! via `#[path = ...]` includes. Functions are parameterized by binary name to
//! support different CLI tools.

use std::io::Error;
use std::path::Path;

/// Wrap an io::Error with additional context.
fn io_context(msg: String, err: Error) -> Error {
    Error::other(format!("{msg}: {err}"))
}

/// Generate markdown CLI reference with optional brew installation instructions.
pub fn generate_markdown_with_brew(
    cmd: &clap::Command,
    dir: &Path,
    name: &str,
    brew_formula: Option<&str>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let generated = clap_markdown::help_markdown_command(cmd);
    let md = if let Some(formula) = brew_formula {
        inject_install_section(&generated, formula)
    } else {
        generated
    };
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

/// Insert an Installation section after the first description paragraph in the generated markdown.
fn inject_install_section(md: &str, formula: &str) -> String {
    // Insert after the "This document contains..." line
    if let Some(pos) = md.find("\n\n**Command Overview:**") {
        let mut out = String::with_capacity(md.len() + 128);
        out.push_str(&md[..pos]);
        out.push_str("\n\n## Installation\n\n```bash\nbrew install ");
        out.push_str(formula);
        out.push_str("\n```\n\n**Command Overview:**");
        out.push_str(&md[pos + "\n\n**Command Overview:**".len()..]);
        out
    } else {
        // Fallback: prepend after first line
        let mut out = String::with_capacity(md.len() + 128);
        if let Some(pos) = md.find('\n') {
            out.push_str(&md[..pos]);
            out.push_str("\n\n## Installation\n\n```bash\nbrew install ");
            out.push_str(formula);
            out.push_str("\n```\n");
            out.push_str(&md[pos..]);
        } else {
            out.push_str(md);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_install_section_with_command_overview() {
        let md = "# Help for `foo`\n\nSome description.\n\n**Command Overview:**\n\n* stuff";
        let result = inject_install_section(md, "tap/foo");
        assert!(result.contains("## Installation"));
        assert!(result.contains("brew install tap/foo"));
        // Installation appears before Command Overview
        let install_pos = result.find("## Installation").expect("missing install");
        let overview_pos = result
            .find("**Command Overview:**")
            .expect("missing overview");
        assert!(install_pos < overview_pos);
        // Command Overview still present exactly once
        assert_eq!(result.matches("**Command Overview:**").count(), 1);
    }

    #[test]
    fn inject_install_section_fallback_no_command_overview() {
        let md = "# Help for `foo`\n\nSome other content.";
        let result = inject_install_section(md, "tap/foo");
        assert!(result.contains("## Installation"));
        assert!(result.contains("brew install tap/foo"));
        // Original content preserved
        assert!(result.contains("Some other content."));
    }

    #[test]
    fn inject_install_section_single_line() {
        let md = "# Help";
        let result = inject_install_section(md, "tap/bar");
        // No newline to split on, returns unchanged
        assert_eq!(result, md);
    }
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
