//! AVP Edit - Open a RuleSet's VALIDATOR.md in the user's editor.

use std::path::PathBuf;

use crate::registry::RegistryError;

/// Run the edit command.
///
/// Opens the VALIDATOR.md file for the named RuleSet in `$VISUAL` or `$EDITOR`.
/// When `global` is true, looks in `~/.avp/validators/`; otherwise `.avp/validators/`.
pub fn run_edit(name: &str, global: bool) -> Result<(), RegistryError> {
    let base_dir = if global {
        dirs::home_dir()
            .ok_or_else(|| RegistryError::Validation("Could not find home directory".to_string()))?
            .join(".avp")
            .join("validators")
            .join(name)
    } else {
        PathBuf::from(".avp").join("validators").join(name)
    };

    let validator_path = base_dir.join("VALIDATOR.md");

    if !validator_path.exists() {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::Validation(format!(
            "RuleSet '{}' not found in {} scope ({}).\nCreate it with: avp new {}{}",
            name,
            scope,
            validator_path.display(),
            name,
            if global { " --global" } else { "" }
        )));
    }

    swissarmyhammer_common::open_in_editor(&validator_path)
        .map_err(|e| RegistryError::Validation(e.to_string()))?;

    Ok(())
}
