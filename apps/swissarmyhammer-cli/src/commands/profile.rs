//! sah's install [`Profile`] — the declarative manifest of what `sah init` /
//! `sah deinit` install across detected AI coding agents.
//!
//! sah is "just a bigger profile": it declares the shared SAH MCP server
//! (`sah serve`), every builtin skill, every builtin agent, and the sah-only
//! statusline + CLAUDE.md preamble flags. [`mirdan::install::init_profile`] /
//! [`mirdan::install::deinit_profile`] interpret this data — there is no
//! bespoke per-step `Initializable` code for skill/agent/mcp/statusline/preamble.
//!
//! The two concerns that are *not* expressible as profile data —
//! creating the `.sah/` + `.prompts/` project workspace and registering the
//! `.kanban/` merge drivers — remain as the `Initializable` components
//! registered by [`super::registry::register_all`].

use mirdan::install::{Profile, ProfileMcpServer, Selector};

/// Build sah's install profile.
///
/// Declares:
/// - `mcp_server`: the shared SAH MCP server, launched via `sah serve`.
/// - `skills`: every builtin skill ([`Selector::All`]).
/// - `agents`: every builtin agent ([`Selector::All`]).
/// - `statusline`: install the `sah statusline` block.
/// - `preamble`: ensure the CLAUDE.md preamble is present.
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
        statusline: true,
        preamble: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sah's profile declares the full set: the `sah serve` MCP server, all
    /// builtin skills, all builtin agents, statusline, and preamble.
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
        assert!(profile.statusline);
        assert!(profile.preamble);
    }
}
