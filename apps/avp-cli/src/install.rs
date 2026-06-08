//! Install and uninstall AVP hooks in Claude Code settings.
//!
//! Thin wrapper around `avp_common::install` that converts CLI types.

use swissarmyhammer_common::lifecycle::InitScope;

/// Re-export InstallTarget from the self-contained cli module.
pub use crate::cli::InstallTarget;

/// Get the settings file path for the given target.
///
/// Returns an error if the home directory cannot be determined for `User` scope.
pub fn settings_path(target: InstallTarget) -> Result<std::path::PathBuf, String> {
    avp_common::install::settings_path(target.into())
}

// The `From<InstallTarget> for InitScope` conversion is now provided by the
// canonical shared `InstallTarget` in
// `swissarmyhammer_cli_completions::lifecycle`, so it is not redeclared here.

/// Install AVP hooks to the specified target.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let scope: InitScope = target.into();
    let cwd =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    avp_common::install::install(scope, &cwd)
}

/// Uninstall AVP hooks from the specified target.
pub fn uninstall(target: InstallTarget) -> Result<(), String> {
    let scope: InitScope = target.into();
    let cwd =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    avp_common::install::uninstall(scope, &cwd)
}

#[cfg(test)]
mod tests {
    // Install/uninstall logic is tested in avp-common/src/install.rs.
    // This module only tests the CLI-specific type conversion.
    use super::*;

    #[test]
    fn test_install_target_to_init_scope() {
        assert_eq!(InitScope::from(InstallTarget::Project), InitScope::Project);
        assert_eq!(InitScope::from(InstallTarget::Local), InitScope::Local);
        assert_eq!(InitScope::from(InstallTarget::User), InitScope::User);
    }
}
