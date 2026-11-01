//! sah's install [`Profile`] — the declarative manifest of what `sah init` /
//! `sah deinit` install across detected AI coding agents.
//!
//! sah is "just a bigger profile": it declares the shared SAH MCP server
//! (`sah serve`), every builtin skill, every builtin agent, and the sah-only
//! statusline flag.
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`]
//! interpret this data — there is no bespoke per-step `Initializable` code for
//! skill/agent/mcp/statusline.
//!
//! The two concerns that are *not* expressible as profile data —
//! creating the `.sah/` + `.prompts/` project workspace and registering the
//! `.kanban/` merge drivers — remain as the `Initializable` components
//! registered by [`super::registry::register_all`].

use mirdan::install::{Profile, ProfileMcpServer, Selector, LEGACY_SHELLTOOL_MCP_SERVER};

/// Skill names that older sah versions deployed and no longer ship.
///
/// `sah init` and `sah deinit` remove each name from the skill store and from
/// every detected agent. Retiring a skill means adding its name here and
/// nowhere else.
const LEGACY_SKILLS: &[&str] = &["shell"];

/// Build sah's install profile.
///
/// Declares:
/// - `mcp_server`: the shared SAH MCP server, launched via `sah serve`.
/// - `skills`: every builtin skill ([`Selector::All`]).
/// - `agents`: every builtin agent ([`Selector::All`]).
/// - `statusline`: install the `sah statusline` block.
/// - `legacy_mcp_servers`: remove the `shelltool` MCP entry that an older
///   install wrote.
/// - `legacy_skills`: remove every retired skill listed in [`LEGACY_SKILLS`].
///
/// The profile declares no native tool deny. Init and deinit remove the legacy
/// deny entries of an older sah for every profile (see
/// [`mirdan::install::LEGACY_NATIVE_DENY_TOOLS`]).
///
/// Kept in sync with mirdan's cross-CLI consistency tests
/// (`mirdan::install::profile_consistency_tests::sah_profile`), which
/// reconstruct this shape to prove every CLI installs by the one
/// store+symlink mechanism. Changing this profile means updating that
/// reconstruction too.
pub fn sah_profile() -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("sah")),
        skills: Some(Selector::All),
        agents: Some(Selector::All),
        validators: Some(Selector::All),
        statusline: true,
        legacy_mcp_servers: vec![LEGACY_SHELLTOOL_MCP_SERVER.to_string()],
        legacy_skills: LEGACY_SKILLS.iter().map(|name| name.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mirdan::install::init_profile;
    use mirdan::test_support::{
        assert_no_init_error, write_single_agent_config, MirdanConfigGuard,
    };
    use serial_test::serial;
    use swissarmyhammer_common::lifecycle::InitScope;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    /// A builtin skill the real sah profile still deploys.
    const CURRENT_SKILL: &str = "commit";

    /// sah's profile declares the full set: the `sah serve` MCP server, all
    /// builtin skills, all builtin agents, and statusline.
    #[test]
    fn test_sah_profile_declares_full_set() {
        let profile = sah_profile();

        let server = profile
            .mcp_server
            .expect("sah profile registers an MCP server");
        assert_eq!(server.name, "sah");
        assert_eq!(server.command, "sah");
        assert_eq!(server.args, vec!["serve".to_string()]);

        assert_eq!(profile.skills, Some(Selector::All));
        assert_eq!(profile.agents, Some(Selector::All));
        assert_eq!(profile.validators, Some(Selector::All));
        assert!(profile.statusline);
        assert_eq!(
            profile.legacy_mcp_servers,
            vec![LEGACY_SHELLTOOL_MCP_SERVER.to_string()]
        );
        assert_eq!(profile.legacy_skills, vec!["shell".to_string()]);
    }

    /// `init_profile` on the real sah profile removes every retired skill from
    /// the store and from the agent skill folder, and still deploys the
    /// current skills. Temporary directories only.
    #[test]
    #[serial(cwd)]
    fn test_sah_init_removes_retired_skills_and_keeps_current() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let home = env.home_path();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let _config = MirdanConfigGuard::set(write_single_agent_config(&root, &home));

        let agent_skills = home.join(".fake/skills");
        std::fs::create_dir_all(&agent_skills).unwrap();
        let profile = sah_profile();
        for name in &profile.legacy_skills {
            let store = home.join(".skills").join(name);
            std::fs::create_dir_all(&store).unwrap();
            std::fs::write(
                store.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: seeded\n---\nbody\n"),
            )
            .unwrap();
            std::os::unix::fs::symlink(&store, agent_skills.join(name)).unwrap();
        }

        let results = init_profile(&profile, InitScope::User, None, &NullReporter);
        assert_no_init_error("sah init", &results);

        let present = |path: std::path::PathBuf| std::fs::symlink_metadata(path).is_ok();
        let retired_left: Vec<&String> = profile
            .legacy_skills
            .iter()
            .filter(|name| {
                present(home.join(".skills").join(name)) || present(agent_skills.join(name))
            })
            .collect();
        assert!(
            retired_left.is_empty(),
            "retired skills left: {retired_left:?}"
        );
        assert!(present(home.join(".skills").join(CURRENT_SKILL)));
        assert!(present(agent_skills.join(CURRENT_SKILL)));
    }
}
