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
    /// Since `SUPERSEDED_NATIVE_DENY_TOOLS` (`mirdan::install::profile`) folded
    /// `Bash` into the roster sah's `edit_redirect` profile fragment installs
    /// and removes, `sah init`/`sah deinit` now *do* manage a `Bash` entry in
    /// `permissions.deny` — but only in `settings.json`
    /// (`agent_project_settings_file` / `agent_global_settings_file`). The
    /// serve path's Bash deny is a separate mechanism: it targets
    /// `InitScope::Local`, which resolves to the `settings.local.json` sibling
    /// (see `mirdan::strategy::local_settings_sibling`), a file the profile
    /// fragment never touches at any scope. Seed a pre-existing
    /// `permissions.deny: ["Bash"]` into that sibling file (as the serve path
    /// would have written) and run the full user-scope deinit flow; the deny
    /// must survive untouched because deinit never opens that file.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_does_not_reallow_bash() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        // The serve-time Bash deny lives beside the global settings file, in
        // its settings.local.json sibling — never in settings.json itself.
        let local_settings = env.home_path().join(".claude").join("settings.local.json");
        std::fs::create_dir_all(local_settings.parent().unwrap()).unwrap();
        std::fs::write(&local_settings, r#"{"permissions":{"deny":["Bash"]}}"#).unwrap();

        // Run the full user-scope deinit through the public entry point.
        let _ = uninstall(InstallTarget::User, false);

        // The deny must still be present: the profile's edit-redirect fragment
        // only ever reads/writes settings.json, so a serve-applied deny in
        // settings.local.json survives untouched.
        let content = std::fs::read_to_string(&local_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .expect("permissions.deny must still be present after deinit");
        assert!(
            deny.iter().any(|v| v.as_str() == Some("Bash")),
            "Bash must remain in settings.local.json's permissions.deny after deinit (serve-time deny is sticky), got {:?}",
            deny
        );
    }

    /// Guard: `sah deinit` DOES remove `Bash` from the profile-managed
    /// `settings.json`, since `SUPERSEDED_NATIVE_DENY_TOOLS` now includes it
    /// alongside `Edit`/`Read`/`Write` — the accepted consequence of folding
    /// the roster into one constant (see the doctor-agreement fix that added
    /// `Bash`/`Read` to `SUPERSEDED_NATIVE_DENY_TOOLS`). This is the mirror of
    /// [`test_deinit_does_not_reallow_bash`]: that test guards the file the
    /// profile never touches, this one guards the file it does.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_removes_bash_from_profile_managed_settings() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        let global_settings = env.home_path().join(".claude").join("settings.json");
        std::fs::create_dir_all(global_settings.parent().unwrap()).unwrap();
        std::fs::write(
            &global_settings,
            r#"{"permissions":{"deny":["Bash","Edit","Read","Write"]}}"#,
        )
        .unwrap();

        let _ = uninstall(InstallTarget::User, false);

        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !deny.iter().any(|v| v.as_str() == Some("Bash")),
            "Bash must be removed from settings.json's permissions.deny by deinit \
             (it is part of SUPERSEDED_NATIVE_DENY_TOOLS), got {:?}",
            deny
        );
    }
}
