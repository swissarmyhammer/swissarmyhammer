//! `code-context skill` — deploy the builtin code-context skills to agents.
//!
//! Deploys the `code-context` skill (teaching agents how to use code
//! intelligence ops), `explore` (the structural investigation workflow), `lsp`
//! (diagnosing and installing missing LSP servers), and `detected-projects`
//! (project-type/build-command discovery) to every detected agent's `.skills/`
//! directory.
//!
//! This is a thin wrapper over mirdan's profile installer: it builds a
//! skills-only [`mirdan::install::Profile`] (the same [`registry::skills_selector`]
//! the full `init` profile uses, with no MCP server) and applies it through
//! [`mirdan::install::init_profile`]. All rendering (Liquid templating of the
//! builtin SKILL.md) and store+symlink deployment live in mirdan, so there is no
//! per-CLI render/deploy pipeline here.

use swissarmyhammer_common::lifecycle::{InitScope, InitStatus};
use swissarmyhammer_common::reporter::CliReporter;

use crate::commands::registry;

/// Deploy the code-context skills to detected agents and return an exit code.
///
/// Builds a skills-only profile and applies it at [`InitScope::Project`] (the
/// scope the standalone `skill` command has always targeted — project-local
/// `.skills/` directories). Returns 0 when no result reported an error, 1
/// otherwise.
pub fn run_skill() -> i32 {
    let profile = mirdan::install::Profile {
        mcp_server: None,
        skills: Some(registry::skills_selector()),
        agents: None,
        statusline: false,
        preamble: false,
    };
    let reporter = CliReporter;
    let results = mirdan::install::init_profile(&profile, InitScope::Project, None, &reporter);
    let had_error = results.iter().any(|r| r.status == InitStatus::Error);
    i32::from(had_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    /// Deploying a skill writes the central `.skills/` store and per-agent
    /// `.claude/skills`, `.zed/skills`, etc. directories relative to the
    /// process working directory. During `cargo test` the working directory
    /// is the crate manifest dir, so any test that runs real deployment must
    /// first chdir into an isolated temp dir or it pollutes the source tree
    /// (in particular it would overwrite the tracked `.skills/*/SKILL.md`
    /// files committed under this crate).
    /// Returns the guard (restores cwd on drop) and the owning `TempDir`.
    fn isolated_deploy_dir() -> (CurrentDirGuard, tempfile::TempDir) {
        let temp = tempfile::tempdir().expect("create temp dir for skill deployment");
        let guard = CurrentDirGuard::new(temp.path()).expect("chdir into isolated temp dir");
        (guard, temp)
    }

    #[test]
    fn test_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("code-context"),
            "builtin 'code-context' skill should exist"
        );
    }

    #[test]
    fn test_lsp_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("lsp"),
            "builtin 'lsp' skill should exist"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_run_skill_returns_valid_exit_code() {
        // run_skill() may report errors if there are no agent directories
        // detected, but it should never panic -- it returns 0 or 1. Run it
        // inside an isolated temp dir so the deployed
        // `.skills/code-context/SKILL.md`, `.skills/explore/SKILL.md`,
        // `.skills/lsp/SKILL.md`, `.skills/detected-projects/SKILL.md`,
        // `.claude/skills`, etc. land there instead of overwriting the tracked
        // source files in this crate.
        //
        // `#[serial_test::serial(cwd)]` joins the crate-wide `cwd` serialization
        // group — the single mutex shared by EVERY CWD-touching test in this
        // crate, including the `serial(cwd)` tests in `ops.rs` and the
        // `CurrentDirGuard` test in `logging.rs`.
        let (_guard, _temp) = isolated_deploy_dir();
        let exit_code = run_skill();
        assert!(
            exit_code == 0 || exit_code == 1,
            "exit code should be 0 or 1, got {exit_code}"
        );
    }
}
