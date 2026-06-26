//! sah init/deinit component registry.
//!
//! `sah init`/`sah deinit` install everything sah's [`Profile`] declares —
//! the shared SAH MCP server, all builtin skills, all builtin agents, the
//! statusline, and the CLAUDE.md preamble — through the single data-driven
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`]
//! path (see [`super::profile::sah_profile`]). There is no bespoke per-step
//! `Initializable` code for any of those concerns.
//!
//! This registry holds only the filesystem-scaffold install concerns that are
//! *not* expressible as profile data:
//!
//! | Priority | Component (display name)               | User | Notes                                                |
//! |---------:|----------------------------------------|:----:|------------------------------------------------------|
//! | 40       | ProjectStructure ("Project workspace") |  -   | Project-only — creates `.sah/` + `.prompts/`         |
//! | 45       | ExpectTool ("Expectations")            |  -   | Project-only — scaffolds the `.expect/` tree         |
//! | 55       | KanbanTool                             |  -   | Tool lifecycle: registers `.kanban/` merge drivers   |
//!
//! `ProjectStructure` creates the `.sah/` + `.prompts/` project tree (via
//! [`swissarmyhammer_common::SwissarmyhammerDirectory::from_custom_root`]) and
//! is skipped in User scope because sah's runtime state is project-local — see
//! [`super::install::components::ProjectStructure`] for the full rationale.
//! `KanbanTool` manages `.kanban/` merge drivers, a tool-init concern not
//! covered by the profile's `mcp_server`.
//!
//! There is no Bash-permission component: the Bash deny is owned by the serve
//! path (applied when a Claude client connects) and is sticky — neither
//! `sah init` nor `sah deinit` denies or re-allows Bash.

use swissarmyhammer_common::lifecycle::InitRegistry;

/// Register the non-profile sah init/deinit components into the given registry.
///
/// Only the filesystem-scaffold concerns are registered here —
/// `ProjectStructure` (`.sah/` + `.prompts/`), `ExpectTool` (the `.expect/`
/// tree), and `KanbanTool` (`.kanban/` merge drivers). Every other install
/// concern (MCP, skills, agents, statusline, preamble) is handled by
/// [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`] from
/// [`super::install::init`] / [`super::install::deinit`].
///
/// * `remove_directory` - Whether `ProjectStructure::deinit` should delete the
///   `.sah/` and `.prompts/` directories. Pass `false` for `init`.
pub fn register_all(registry: &mut InitRegistry, remove_directory: bool) {
    registry.register(super::install::components::ProjectStructure::new(
        remove_directory,
    ));

    // The `expect` tool scaffolds the project-local `.expect/` tree (config,
    // README, example, goldens/received dirs + .gitignore) and detects the
    // surface defaults. Project-local filesystem setup, like ProjectStructure.
    registry.register(swissarmyhammer_tools::mcp::tools::expect::ExpectTool::new());

    // sah exposes kanban through `sah serve`, NOT a separate `kanban` MCP
    // server — so it constructs the tool WITHOUT an injected MCP entry. The
    // tool's init/deinit then only manage `.kanban/` merge drivers.
    registry.register(swissarmyhammer_tools::mcp::tools::kanban::KanbanTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_all_populates_registry() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, false);
        // The three scaffold components: ProjectStructure + ExpectTool + KanbanTool.
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn test_register_all_with_remove_directory() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry, true);
        // Same component count regardless of remove_directory flag.
        assert_eq!(registry.len(), 3);
    }
}
