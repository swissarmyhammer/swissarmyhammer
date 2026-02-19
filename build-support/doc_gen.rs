//! Shared CLI documentation generation utilities.
//!
//! Used by both `avp-cli/build.rs` and `swissarmyhammer-cli/src/generate_docs.rs`
//! via `#[path = ...]` includes. Functions are parameterized by binary name to
//! support different CLI tools.

use std::io::Error;
use std::path::{Path, PathBuf};

/// Extra capacity for the brew install section injected into generated markdown.
/// Covers the `## Installation\n\n```bash\nbrew install ...\n```\n` boilerplate
/// plus a typical formula name.
const INSTALL_SECTION_OVERHEAD: usize = 128;

/// Wrap an io::Error with additional context.
fn io_context(msg: String, err: Error) -> Error {
    Error::other(format!("{msg}: {err}"))
}

/// Information about a generated file.
pub struct GeneratedFile {
    /// Path to the generated file.
    pub path: PathBuf,
    /// Size in bytes of the generated file.
    pub size: usize,
}

/// Generate a Markdown CLI reference and write it to `<dir>/<name>-cli.md`.
///
/// Uses `clap-markdown` to render `cmd` into Markdown. If `brew_formula` is
/// provided, a `## Installation` section with `brew install <formula>` is
/// injected before the command overview.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or the file
/// cannot be written.
pub fn generate_markdown_with_brew(
    cmd: &clap::Command,
    dir: &Path,
    name: &str,
    brew_formula: Option<&str>,
) -> std::io::Result<GeneratedFile> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let options = clap_markdown::MarkdownOptions::new().show_footer(false);
    let generated = clap_markdown::help_markdown_command_custom(cmd, &options);
    let md = if let Some(formula) = brew_formula {
        inject_install_section(&generated, formula)
    } else {
        generated
    };
    let filename = format!("{name}-cli.md");
    let path = dir.join(&filename);
    let size = md.len();
    std::fs::write(&path, &md)
        .map_err(|e| io_context(format!("failed to write {}", path.display()), e))?;
    Ok(GeneratedFile { path, size })
}

/// Insert an Installation section after the first description paragraph in the generated markdown.
fn inject_install_section(md: &str, formula: &str) -> String {
    // Insert after the "This document contains..." line
    if let Some(pos) = md.find("\n\n**Command Overview:**") {
        let mut out = String::with_capacity(md.len() + INSTALL_SECTION_OVERHEAD);
        out.push_str(&md[..pos]);
        out.push_str("\n\n## Installation\n\n```bash\nbrew install ");
        out.push_str(formula);
        out.push_str("\n```\n\n**Command Overview:**");
        out.push_str(&md[pos + "\n\n**Command Overview:**".len()..]);
        out
    } else {
        // Fallback: prepend after first line
        let mut out = String::with_capacity(md.len() + INSTALL_SECTION_OVERHEAD);
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

    #[test]
    fn generate_markdown_does_not_include_clap_footer() {
        let cmd = clap::Command::new("test-cmd").about("A test command");
        let dir = tempfile::TempDir::new().unwrap();
        let info = generate_markdown_with_brew(&cmd, dir.path(), "test", None).unwrap();
        let content = std::fs::read_to_string(&info.path).unwrap();
        assert_eq!(info.size, content.len());
        assert!(
            !content.contains("clap-markdown"),
            "Generated markdown should not contain clap-markdown footer, but found it:\n{content}"
        );
    }

    #[test]
    fn generate_markdown_with_brew_formula() {
        let cmd = clap::Command::new("test-cmd").about("A test command");
        let dir = tempfile::TempDir::new().unwrap();
        let info = generate_markdown_with_brew(&cmd, dir.path(), "test", Some("my-tap/my-formula"))
            .unwrap();
        let content = std::fs::read_to_string(&info.path).unwrap();
        assert_eq!(info.size, content.len());
        assert!(
            content.contains("brew install my-tap/my-formula"),
            "Expected brew install section in output:\n{content}"
        );
        assert!(
            content.contains("## Installation"),
            "Expected Installation heading in output:\n{content}"
        );
        assert!(
            !content.contains("clap-markdown"),
            "Should not contain clap-markdown footer:\n{content}"
        );
    }

    #[test]
    fn generate_markdown_returns_correct_path() {
        let cmd = clap::Command::new("mycli").about("My CLI");
        let dir = tempfile::TempDir::new().unwrap();
        let info = generate_markdown_with_brew(&cmd, dir.path(), "mycli", None).unwrap();
        assert_eq!(info.path, dir.path().join("mycli-cli.md"));
        assert!(info.path.exists());
    }

    #[test]
    fn generate_markdown_creates_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("sub").join("dir");
        let cmd = clap::Command::new("test").about("test");
        let info = generate_markdown_with_brew(&cmd, &nested, "test", None).unwrap();
        assert!(info.path.exists());
    }

    #[test]
    fn generate_manpage_writes_file() {
        let cmd = clap::Command::new("test-cmd").about("A test command");
        let dir = tempfile::TempDir::new().unwrap();
        let info = generate_manpage(&cmd, dir.path(), "test").unwrap();
        assert_eq!(info.path, dir.path().join("test.1"));
        assert!(info.path.exists());
        assert!(info.size > 0);
    }

    #[test]
    fn generate_manpage_creates_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("man").join("pages");
        let cmd = clap::Command::new("test").about("test");
        let info = generate_manpage(&cmd, &nested, "test").unwrap();
        assert!(info.path.exists());
    }

    #[test]
    fn generate_completions_writes_files() {
        let cmd = clap::Command::new("test-cmd").about("A test command");
        let dir = tempfile::TempDir::new().unwrap();
        let paths = generate_completions(cmd, dir.path(), "test-cmd").unwrap();
        assert_eq!(
            paths.len(),
            3,
            "Expected completions for Bash, Zsh, and Fish"
        );
        for path in &paths {
            assert!(
                path.exists(),
                "Completion file should exist: {}",
                path.display()
            );
        }
    }

    #[test]
    fn generate_completions_creates_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("comp").join("dir");
        let cmd = clap::Command::new("test").about("test");
        let paths = generate_completions(cmd, &nested, "test").unwrap();
        assert_eq!(paths.len(), 3);
        for path in &paths {
            assert!(path.exists());
        }
    }
}

/// Generate a ROFF man page and write it to `<dir>/<name>.1`.
///
/// Uses `clap_mangen` to render `cmd` into a roff-format man page.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created, the man page
/// cannot be rendered, or the file cannot be written.
pub fn generate_manpage(
    cmd: &clap::Command,
    dir: &Path,
    name: &str,
) -> std::io::Result<GeneratedFile> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)
        .map_err(|e| io_context(format!("failed to render man page for {name}"), e))?;
    let filename = format!("{name}.1");
    let path = dir.join(&filename);
    let size = buf.len();
    std::fs::write(&path, &buf)
        .map_err(|e| io_context(format!("failed to write {}", path.display()), e))?;
    Ok(GeneratedFile { path, size })
}

/// Generate shell completion scripts for Bash, Zsh, and Fish.
///
/// Writes one completion file per shell into `dir`, named according to each
/// shell's convention (e.g. `_name` for Zsh, `name.bash` for Bash).
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or any
/// completion script cannot be generated.
pub fn generate_completions(
    mut cmd: clap::Command,
    dir: &Path,
    name: &str,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)
        .map_err(|e| io_context(format!("failed to create directory {}", dir.display()), e))?;
    let mut paths = Vec::new();
    for shell in [
        clap_complete::Shell::Bash,
        clap_complete::Shell::Zsh,
        clap_complete::Shell::Fish,
    ] {
        let path = clap_complete::generate_to(shell, &mut cmd, name, dir)
            .map_err(|e| io_context(format!("failed to generate {shell:?} completions"), e))?;
        paths.push(path);
    }
    Ok(paths)
}
