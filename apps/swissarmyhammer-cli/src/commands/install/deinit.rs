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
    use mirdan::install::SUPERSEDED_NATIVE_DENY_TOOLS;
    use std::path::{Path, PathBuf};
    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    /// Write `<home>/.claude/<file_name>` with a `permissions.deny` array that
    /// holds the whole [`SUPERSEDED_NATIVE_DENY_TOOLS`] roster, and return the
    /// path of that file.
    ///
    /// The fixture reads the roster from the constant instead of spelling the
    /// tool names out, so a tool added to the constant is seeded here too.
    fn seed_superseded_deny(home: &Path, file_name: &str) -> PathBuf {
        let settings = home.join(".claude").join(file_name);
        std::fs::create_dir_all(settings.parent().expect("settings path has a parent"))
            .expect("create the agent settings directory");
        let seeded = serde_json::json!({
            "permissions": { "deny": SUPERSEDED_NATIVE_DENY_TOOLS }
        });
        std::fs::write(&settings, seeded.to_string()).expect("seed the settings file");
        settings
    }

    /// Read the `permissions.deny` entries of the settings file at `path` back
    /// as plain strings. A missing file, a missing key, and a non-array value
    /// all read as no entries.
    fn deny_entries(path: &Path) -> Vec<String> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("the settings file holds valid JSON");
        parsed
            .pointer("/permissions/deny")
            .and_then(|deny| deny.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Guard: `sah deinit` must NOT clean up the serve-applied denies.
    ///
    /// Since `SUPERSEDED_NATIVE_DENY_TOOLS` (`mirdan::install::profile`) folded
    /// `Bash` into the roster sah's `edit_redirect` profile fragment installs
    /// and removes, `sah init`/`sah deinit` now *do* manage those entries in
    /// `permissions.deny` — but only in `settings.json`
    /// (`agent_project_settings_file` / `agent_global_settings_file`). The
    /// serve path's deny is a separate mechanism: it targets
    /// `InitScope::Local`, which resolves to the `settings.local.json` sibling
    /// (see `mirdan::strategy::local_settings_sibling`), a file the profile
    /// fragment never touches at any scope. Seed a pre-existing
    /// `permissions.deny` roster into that sibling file (as the serve path
    /// would have written) and run the full user-scope deinit flow; every
    /// entry must survive untouched because deinit never opens that file.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_does_not_reallow_superseded_natives() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        // The serve-time deny lives beside the global settings file, in its
        // settings.local.json sibling — never in settings.json itself.
        let local_settings = seed_superseded_deny(&env.home_path(), "settings.local.json");

        // Run the full user-scope deinit through the public entry point.
        let _ = uninstall(InstallTarget::User, false);

        // Every deny must still be present: the profile's edit-redirect
        // fragment only ever reads/writes settings.json, so a serve-applied
        // deny in settings.local.json survives untouched.
        let deny = deny_entries(&local_settings);
        for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
            assert!(
                deny.iter().any(|entry| entry == tool),
                "{tool} must remain in settings.local.json's permissions.deny after deinit \
                 (the serve-time deny is sticky), got {deny:?}"
            );
        }
    }

    /// Guard: `sah deinit` DOES remove every `SUPERSEDED_NATIVE_DENY_TOOLS`
    /// entry from the profile-managed `settings.json`, `Bash` and `Read`
    /// included — the accepted consequence of folding the roster into one
    /// constant (see the doctor-agreement fix that added `Bash`/`Read` to
    /// `SUPERSEDED_NATIVE_DENY_TOOLS`). This is the mirror of
    /// [`test_deinit_does_not_reallow_superseded_natives`]: that test guards
    /// the file the profile never touches, this one guards the file it does.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_removes_superseded_natives_from_profile_managed_settings() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        let global_settings = seed_superseded_deny(&env.home_path(), "settings.json");

        let _ = uninstall(InstallTarget::User, false);

        let deny = deny_entries(&global_settings);
        for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
            assert!(
                !deny.iter().any(|entry| entry == tool),
                "{tool} must be removed from settings.json's permissions.deny by deinit \
                 (it is part of SUPERSEDED_NATIVE_DENY_TOOLS), got {deny:?}"
            );
        }
    }
}
