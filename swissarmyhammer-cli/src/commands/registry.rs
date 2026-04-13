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
/// Components are registered in priority order:
/// - priority 10: `McpRegistration` (MCP server config for detected agents)
/// - priority 11: `ClaudeLocalScope` (`~/.claude.json` local scope)
/// - priority 15: `DenyBash` (deny built-in Bash tool in Claude Code settings)
/// - priority 20: `ProjectStructure` (`.sah/`, `.prompts/` directory management)
/// - priority 22: `ClaudeMd` (`CLAUDE.md` preamble management)
/// - priority 30: `SkillDeployment` (builtin skill deployment via mirdan)
/// - priority 31: `AgentDeployment` (builtin agent deployment via mirdan)
/// - priority 32: `LockfileCleanup` (lockfile entry cleanup on deinit)
/// - default:     `KanbanTool` (kanban tool lifecycle, no-op for init/deinit)
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

    #[test]
    fn test_register_all_populates_registry() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        // 8 components from components::register_all (the 7 installable components
        // + KanbanTool) + 1 SkillDeployment (from commands::skill) = 9
        assert_eq!(registry.len(), 9);
    }

    #[test]
    fn test_register_all_with_remove_directory() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, true);
        // Same component count regardless of remove_directory flag
        assert_eq!(registry.len(), 9);
    }

    #[test]
    fn test_register_all_includes_skill_deployment() {
        // `commands/skill::SkillDeployment` must be registered by `register_all`
        // so that `sah init` deploys builtin skills. Verify by running init and
        // inspecting the result names — every registered component emits at
        // least one InitResult (even if it's a Skipped result for non-applicable
        // scopes).
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
    fn test_init_runs_without_panic() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        // Just verify no panic; results depend on environment
        let _results = registry.run_all_init(
            &swissarmyhammer_common::lifecycle::InitScope::Project,
            &reporter,
        );
    }

    #[test]
    fn test_deinit_runs_without_panic() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        let reporter = NullReporter;
        // Just verify no panic; results depend on environment
        let _results = registry.run_all_deinit(
            &swissarmyhammer_common::lifecycle::InitScope::Project,
            &reporter,
        );
    }
}
