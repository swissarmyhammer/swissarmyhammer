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
    use mirdan::test_support::{deny_entries, legacy_deny_settings, read_json, UNRELATED_DENY};
    use std::path::{Path, PathBuf};
    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    /// Write `<home>/.claude/<file_name>` with the shared legacy deny fixture
    /// (the whole legacy roster plus one unrelated entry), and return the path
    /// of that file.
    fn seed_legacy_deny(home: &Path, file_name: &str) -> PathBuf {
        let settings = home.join(".claude").join(file_name);
        std::fs::create_dir_all(settings.parent().expect("settings path has a parent"))
            .expect("create the agent settings directory");
        std::fs::write(&settings, legacy_deny_settings().to_string())
            .expect("seed the settings file");
        settings
    }

    /// Guard: `sah deinit` removes every legacy native tool deny (`Bash`,
    /// `Edit`, `Read`, `Write`) from the user settings file through the public
    /// entry point, and keeps an unrelated deny entry.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_removes_legacy_native_denies_from_user_settings() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        let global_settings = seed_legacy_deny(&env.home_path(), "settings.json");

        uninstall(InstallTarget::User, false).expect("user-scope deinit must succeed");

        assert_eq!(
            deny_entries(&read_json(&global_settings)),
            [UNRELATED_DENY],
            "deinit must strip every legacy native tool deny and keep the unrelated one"
        );
    }
}
