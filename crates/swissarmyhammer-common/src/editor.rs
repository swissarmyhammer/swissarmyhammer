//! Editor utilities for opening files in the user's preferred editor.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Open a file in the user's preferred editor.
///
/// Resolves the editor from `$VISUAL`, then `$EDITOR`. If neither is set,
/// returns an error with a helpful message.
///
/// Handles editors specified with arguments (e.g. `"code -w"`) by splitting
/// on whitespace. Waits for the editor process to exit.
pub fn open_in_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .map_err(|_| {
            anyhow::anyhow!(
                "No editor configured. Set $EDITOR or $VISUAL environment variable.\n\
                 Examples:\n  export EDITOR=vim\n  export EDITOR=\"code -w\""
            )
        })?;

    let parts: Vec<&str> = editor.split_whitespace().collect();
    if parts.is_empty() {
        bail!("$EDITOR is set but empty");
    }

    let program = parts[0];
    let args = &parts[1..];

    let status = Command::new(program)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch editor '{}'", program))?;

    if !status.success() {
        bail!(
            "Editor '{}' exited with status {}",
            program,
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_open_in_editor_no_editor_set() {
        // Temporarily unset VISUAL and EDITOR
        let visual = std::env::var("VISUAL").ok();
        let editor = std::env::var("EDITOR").ok();
        std::env::remove_var("VISUAL");
        std::env::remove_var("EDITOR");

        let result = open_in_editor(&PathBuf::from("/tmp/test.md"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No editor configured"));

        // Restore
        if let Some(v) = visual {
            std::env::set_var("VISUAL", v);
        }
        if let Some(v) = editor {
            std::env::set_var("EDITOR", v);
        }
    }
}
