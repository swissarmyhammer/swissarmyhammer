//! Remove sah from all detected AI coding agents (skills + agents + MCP +
//! statusline).
//!
//! Mirrors [`super::init`]: the MCP server, builtin skills, builtin agents,
//! and statusline are removed through sah's declarative
//! [`Profile`] via [`mirdan::install::deinit_profile`], and the non-profile
//! `Initializable` components (project workspace, kanban merge drivers) run via
//! [`crate::commands::registry::register_all`].

use crate::cli::InstallTarget;

use super::{run_lifecycle, Direction};

/// Uninstall sah from all detected AI coding agents.
///
/// Runs sah's [`Profile`] through [`mirdan::install::deinit_profile`] (MCP,
/// skills, agents, statusline) and then the non-profile
/// `Initializable` components in reverse priority order. The `remove_directory`
/// flag controls whether `ProjectStructure` removes `.sah/` and `.prompts/`.
pub fn uninstall(target: InstallTarget, remove_directory: bool) -> Result<(), String> {
    run_lifecycle(Direction::Uninstall, target, remove_directory)
}

// Unit tests for the store-cleanup helpers (`remove_if_symlink`,
// `remove_store_entries`) live in `mirdan::store`'s test module — these
// helpers were moved out of swissarmyhammer-cli when the path-safety and
// store-cleanup code consolidated into mirdan.

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    /// Guard: `sah deinit` must NOT clean up the serve-applied Bash deny.
    ///
    /// The Bash deny is owned by the serve path and is sticky — `sah deinit`
    /// owns no Bash-permission teardown (neither sah's [`Profile`] nor the
    /// registry components touch permissions). Seed a pre-existing
    /// `permissions.deny: ["Bash"]` into the user-scope settings file (as the
    /// serve path would have written) and run the full deinit flow; the deny
    /// must survive untouched.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_does_not_reallow_bash() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        // claude-code's global settings file is ~/.claude/settings.json, which
        // resolves under the isolated HOME.
        let global_settings = env.home_path().join(".claude").join("settings.json");
        std::fs::create_dir_all(global_settings.parent().unwrap()).unwrap();
        std::fs::write(&global_settings, r#"{"permissions":{"deny":["Bash"]}}"#).unwrap();

        // Run the full user-scope deinit through the public entry point.
        let _ = uninstall(InstallTarget::User, false);

        // The deny must still be present: deinit owns no Bash-permission
        // teardown, so a serve-applied deny survives.
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .expect("permissions.deny must still be present after deinit");
        assert!(
            deny.iter().any(|v| v.as_str() == Some("Bash")),
            "Bash must remain in permissions.deny after deinit (serve-time deny is sticky), got {:?}",
            deny
        );
    }
}
