//! sah init/deinit component registry.
//!
//! Defines the canonical set of `Initializable` components for `sah init` and
//! `sah deinit`, and exposes `register_all` to populate an `InitRegistry` with
//! them in priority order.
//!
//! This module follows the same pattern used by `shelltool-cli` and
//! `code-context-cli` — a top-level `commands::registry` module that owns the
//! registration function, keeping `init` and `deinit` command handlers thin.
//! Skill deployment lives in [`super::skill`] and is registered here alongside
//! the other install components, matching how sibling CLIs register their own
//! `*SkillDeployment` components.

use swissarmyhammer_common::lifecycle::InitRegistry;

/// Register all sah init/deinit components into the given registry.
///
/// Components are registered in priority order. Lower priority runs first.
/// Priorities are re-spaced into clean 10s so the install pipeline has a
/// stable, predictable ordering. The `User` column shows whether the
/// component participates in `sah init --user` (`y`) or is skipped (`-`).
///
/// | Priority | Component (display name)                       | User | Notes                                                |
/// |---------:|-----------------------------------------------|:----:|------------------------------------------------------|
/// | 10       | McpRegistration ("Register MCP server")       |  y   | Delegates to mirdan appliers (per-agent strategies)  |
/// | 30       | Statusline ("Statusline")                     |  y   | Edits each agent's per-scope settings file           |
/// | 40       | ProjectStructure ("Project workspace")        |  -   | Project-only — skipped in User scope (see below)     |
/// | 50       | ClaudeMd ("Preamble")                         |  y   | Targets each agent's per-scope preamble file         |
/// | 55       | KanbanTool                                    |  -   | Tool lifecycle: registers `.kanban/` merge drivers   |
/// | 60       | SkillDeployment ("Skills")                    |  y   | Builtin skill deployment via mirdan                  |
/// | 70       | AgentDeployment ("Subagents")                 |  y   | Deploys agent defs to per-scope agent dir            |
/// | 80       | LockfileCleanup ("Lockfile")                  |  y   | Cleans up `.sah/` lockfiles                          |
///
/// The display name in parentheses is the human-readable label returned by
/// [`swissarmyhammer_common::lifecycle::Initializable::display_name`]. The
/// bare component name remains a stable slug used by lockfile entries and
/// test selectors.
///
/// Components at priorities 10–80 (except `SkillDeployment`) plus `KanbanTool`
/// are registered by [`super::install::components::register_all`]. There is no
/// Bash-permission component: the Bash deny is owned by the serve path (applied
/// when a Claude client connects) and is sticky — neither `sah init` nor
/// `sah deinit` denies or re-allows Bash.
/// `SkillDeployment` (priority 60) lives in [`super::skill`] and is registered
/// directly here, matching how sibling CLIs (`shelltool-cli`, `code-context-cli`)
/// register their own `*SkillDeployment` components.
///
/// # Why `ProjectStructure` skips User scope
///
/// `ProjectStructure` creates `.sah/` and `.prompts/` at the **project**
/// root and has no `~/.sah/` / `~/.prompts/` counterpart. User scope is
/// purely a per-agent config install (settings, preamble, statusline,
/// agent defs, skills); sah's runtime state — workflows, prompt overrides,
/// kanban boards, code-context indexes — is project-local by design and
/// lives inside the project tree. Readers that consult `~/.sah/` (global
/// `tools.yaml`, statusline overrides, `~/.prompts/`) treat those paths
/// as optional, lazy fallbacks, so there is nothing for this component
/// to pre-create in User scope. See [`super::install::components::ProjectStructure`]
/// for the full rationale.
///
/// The `.sah/` + `.prompts/` workspace-structure logic is **not** forked: the
/// `ProjectStructure` component's `init` delegates to the root-explicit
/// [`swissarmyhammer_workspace_init`] crate — the same crate the kanban-app
/// runs in-process on board open — so `sah init` and the in-process agent
/// produce an identical workspace layout. The CLI's `SkillDeployment` deploys
/// builtin skills to detected coding-agent directories (a distinct target from
/// the workspace-local `.sah/skills/` the workspace-init crate produces).
///
/// The `remove_directory` parameter controls whether `ProjectStructure::deinit`
/// deletes `.sah/` and `.prompts/` directories. Pass `false` for `init`.
pub fn register_all(registry: &mut InitRegistry, remove_directory: bool) {
    super::install::components::register_all(registry, remove_directory);
    registry.register(super::skill::SkillDeployment);
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    #[test]
    fn test_register_all_populates_registry() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        // 7 components from components::register_all (the 6 installable components
        // + KanbanTool) + 1 SkillDeployment (from commands::skill) = 8.
        // (ClaudeLocalScope was folded into McpRegistration's mirdan applier;
        // AllowBashCleanup was removed — the serve-time Bash deny is sticky and
        // deinit must not re-allow it.)
        assert_eq!(registry.len(), 8);
    }

    #[test]
    fn test_register_all_with_remove_directory() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, true);
        // Same component count regardless of remove_directory flag
        assert_eq!(registry.len(), 8);
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_register_all_includes_skill_deployment() {
        // `commands/skill::SkillDeployment` must be registered by `register_all`
        // so that `sah init` deploys builtin skills. Verify by running init and
        // inspecting the result names — every registered component emits at
        // least one InitResult (even if it's a Skipped result for non-applicable
        // scopes).
        //
        // Run inside an isolated HOME + CWD so init components do not touch the
        // host repo (previously this deleted CLAUDE.md and wrote `.sah/` etc.
        // into the live workspace).
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        let results = registry.run_all_init(
            &swissarmyhammer_common::lifecycle::InitScope::Project,
            &reporter,
        );
        assert!(
            results.iter().any(|r| r.name == "skill-deployment"),
            "skill-deployment component should appear in init results, got: {:?}",
            results.iter().map(|r| r.name.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_init_runs_without_panic() {
        // Isolate HOME + CWD — init components otherwise mutate the host repo
        // (create `.sah/`, `.prompts/`, rewrite CLAUDE.md, etc.).
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        let _results = registry.run_all_init(
            &swissarmyhammer_common::lifecycle::InitScope::Project,
            &reporter,
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_deinit_runs_without_panic() {
        // Isolate HOME + CWD — deinit would otherwise call
        // `ClaudeMd::deinit` against the host repo, deleting the real
        // CLAUDE.md when the preamble is all that remains.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        let _results = registry.run_all_deinit(
            &swissarmyhammer_common::lifecycle::InitScope::Project,
            &reporter,
        );
    }

    /// Guard: `sah deinit` must NOT clean up the serve-applied Bash deny.
    ///
    /// The Bash deny is owned by the serve path and is sticky — the
    /// `AllowBashCleanup` component that previously re-allowed Bash on deinit
    /// was removed. Seed a pre-existing `permissions.deny: ["Bash"]` into the
    /// user-scope settings file (as the serve path would have written) and run
    /// the full deinit flow; the deny must survive untouched.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_deinit_does_not_reallow_bash() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");

        // claude-code's global settings file is ~/.claude/settings.json, which
        // resolves under the isolated HOME.
        let global_settings = env.home_path().join(".claude").join("settings.json");
        std::fs::create_dir_all(global_settings.parent().unwrap()).unwrap();
        std::fs::write(&global_settings, r#"{"permissions":{"deny":["Bash"]}}"#).unwrap();

        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        registry.run_all_deinit(
            &swissarmyhammer_common::lifecycle::InitScope::User,
            &reporter,
        );

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
