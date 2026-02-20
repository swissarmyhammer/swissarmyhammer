//! Prompt edit command - open a prompt's source file in the user's editor.

use anyhow::{bail, Result};
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer_common::open_in_editor;

use super::cli::EditCommand;

/// Execute the edit command: open a prompt file in $EDITOR.
pub async fn execute_edit_command(
    cmd: EditCommand,
    _context: &crate::context::CliContext,
) -> Result<()> {
    if cmd.prompt_name.is_empty() {
        bail!("Prompt name is required. Usage: sah prompt edit <name>");
    }

    let mut resolver = PromptResolver::new();
    let mut library = PromptLibrary::new();
    resolver.load_all_prompts(&mut library)?;

    let prompt = library.get(&cmd.prompt_name).map_err(|_| {
        anyhow::anyhow!(
            "Prompt '{}' not found. Run 'sah prompt list' to see available prompts.",
            cmd.prompt_name
        )
    })?;

    let source_path = prompt.source.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Prompt '{}' is a built-in prompt and cannot be edited directly.\n\
             Create an override with: sah prompt new {}",
            cmd.prompt_name,
            cmd.prompt_name
        )
    })?;

    if !source_path.exists() {
        bail!(
            "Prompt source file not found: {}\nIt may have been deleted.",
            source_path.display()
        );
    }

    open_in_editor(source_path)?;

    Ok(())
}
