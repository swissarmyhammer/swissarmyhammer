//! AVP Edit - Open a RuleSet's VALIDATOR.md in the user's editor.

use std::path::PathBuf;

use crate::AvpCliError;

/// Run the edit command.
///
/// Opens the VALIDATOR.md file for the named RuleSet in `$VISUAL` or `$EDITOR`.
/// When `global` is true, looks in `~/.avp/validators/`; otherwise `.avp/validators/`.
pub fn run_edit(name: &str, global: bool) -> Result<(), AvpCliError> {
    let base_dir = if global {
        dirs::home_dir()
            .ok_or_else(|| AvpCliError::Validation("Could not find home directory".to_string()))?
            .join(".avp")
            .join("validators")
            .join(name)
    } else {
        PathBuf::from(".avp").join("validators").join(name)
    };

    let validator_path = base_dir.join("VALIDATOR.md");

    if !validator_path.exists() {
        let scope = if global { "global" } else { "project" };
        return Err(AvpCliError::Validation(format!(
            "RuleSet '{}' not found in {} scope ({}).\nCreate it with: avp new {}{}",
            name,
            scope,
            validator_path.display(),
            name,
            if global { " --global" } else { "" }
        )));
    }

    swissarmyhammer_common::open_in_editor(&validator_path)
        .map_err(|e| AvpCliError::Validation(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_returns_error_for_missing_project_ruleset() {
        let result = run_edit("nonexistent-ruleset", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_project_error_names_the_ruleset() {
        let msg = run_edit("nonexistent-ruleset", false)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("nonexistent-ruleset"));
    }

    #[test]
    fn test_project_error_shows_project_scope() {
        let msg = run_edit("nonexistent-ruleset", false)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("project"));
    }

    #[test]
    fn test_project_error_suggests_creation_command() {
        let msg = run_edit("nonexistent-ruleset", false)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("avp new nonexistent-ruleset"));
    }

    #[test]
    fn test_returns_error_for_missing_global_ruleset() {
        let result = run_edit("nonexistent-ruleset", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_global_error_shows_global_scope() {
        let msg = run_edit("nonexistent-ruleset", true)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("global"));
    }

    #[test]
    fn test_global_error_suggests_global_flag() {
        let msg = run_edit("nonexistent-ruleset", true)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("--global"));
    }
}
