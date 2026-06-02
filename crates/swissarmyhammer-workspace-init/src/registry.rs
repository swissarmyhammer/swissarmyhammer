//! Registry wiring for the root-explicit workspace-init components.
//!
//! Mirrors the `commands::registry` pattern used by `swissarmyhammer-cli`: a
//! single `register_*` function populates an [`InitRegistry`] with the
//! canonical components in priority order, and a `run_*` convenience runs them
//! against an explicit workspace root.

use std::path::Path;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope};
use swissarmyhammer_common::reporter::InitReporter;

use crate::components::{ProjectStructure, SkillDeployment};
// `for_profile` is the kanban-app fast path; see `run_workspace_init_for_profile`.

/// Register the root-explicit workspace-init components into `registry`.
///
/// Components are registered in priority order:
/// - priority 20: [`ProjectStructure`] — `.sah/` + `.prompts/` directory layout
/// - priority 30: [`SkillDeployment`] — builtin skills into `.sah/skills/`
///
/// Both components are rooted at `root`; nothing reads the process working
/// directory. `root` is the workspace directory (the folder that should
/// *contain* `.sah/`), not the `.sah/` directory itself.
pub fn register_workspace_init(registry: &mut InitRegistry, root: &Path) {
    registry.register(ProjectStructure::new(root.to_path_buf()));
    registry.register(SkillDeployment::new(root.to_path_buf()));
}

/// Run workspace init against an explicit `root`, returning the per-component
/// results.
///
/// This builds a fresh [`InitRegistry`], registers the workspace-init
/// components via [`register_workspace_init`], and runs them in priority
/// order. The operation is idempotent: running it again on an already
/// initialized workspace produces no errors and no duplicate state.
///
/// `root` is the workspace directory; `.sah/` and `.prompts/` are created as
/// children of it. The process working directory is never read or mutated.
pub fn run_workspace_init(
    root: &Path,
    scope: &InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut registry = InitRegistry::new();
    register_workspace_init(&mut registry, root);
    registry.run_all_init(scope, reporter)
}

/// Run workspace init against an explicit `root`, deploying only the builtin
/// skills tagged with the given init `profile`.
///
/// Identical to [`run_workspace_init`] except the [`SkillDeployment`] component
/// is profile-filtered: only skills whose `profiles` frontmatter list contains
/// `profile` are written under `<root>/.sah/skills/`. The [`ProjectStructure`]
/// component (the `.sah/` + `.prompts/` layout) is unaffected.
///
/// This is the `sah init` profile slice — it deploys just one profile's cluster
/// alongside the full workspace structure. The operation is idempotent: skills
/// already current on disk are not rewritten.
///
/// Note: the kanban app does **not** use this. A board open ensures the
/// workspace's *tool set* via [`run_workspace_tools_init`], which runs each
/// tool's own `Initializable` and deliberately omits the generic
/// [`ProjectStructure`] step. See that function for the tool-set model.
///
/// [`ProjectStructure`]: crate::ProjectStructure
/// [`SkillDeployment`]: crate::SkillDeployment
pub fn run_workspace_init_for_profile(
    root: &Path,
    profile: &str,
    scope: &InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut registry = InitRegistry::new();
    registry.register(ProjectStructure::new(root.to_path_buf()));
    registry.register(SkillDeployment::for_profile(root.to_path_buf(), profile));
    registry.run_all_init(scope, reporter)
}

/// The set of tools a kanban board's workspace is made of.
///
/// A workspace is defined as a *set of tools*; ensuring the workspace means
/// running each tool's [`Initializable`]. Today that set is exactly the kanban
/// tool, expressed as the init profile whose tagged builtin skills the tool
/// deploys (`kanban` → the 6-skill workflow cluster). Adding a tool to a board's
/// workspace is a one-line addition to this table, not new control flow.
///
/// [`Initializable`]: swissarmyhammer_common::lifecycle::Initializable
const WORKSPACE_TOOLS: &[&str] = &["kanban"];

/// Ensure a board folder's workspace by running each of its tools'
/// [`Initializable`]s, rooted at `root`.
///
/// This is the kanban-app board-open path. The workspace is modeled as a *set
/// of tools* ([`WORKSPACE_TOOLS`], currently just the kanban tool); ensuring the
/// workspace runs each tool's init. For the kanban tool that means deploying its
/// profile's builtin skills under `<root>/.sah/skills/` via a profile-filtered
/// [`SkillDeployment`] — the directory the in-process board MCP server's `skill`
/// tool reads.
///
/// Unlike [`run_workspace_init`] / [`run_workspace_init_for_profile`], this path
/// does **not** run the generic [`ProjectStructure`] step: there is no separate
/// "SAH workspace" to set up here, only the workspace's tools. The `.sah/skills/`
/// directory each tool needs is created by the tool's own deploy step.
///
/// The operation is idempotent: skills already current on disk are not
/// rewritten, so it is safe to call on every board open.
///
/// [`ProjectStructure`]: crate::ProjectStructure
/// [`SkillDeployment`]: crate::SkillDeployment
/// [`Initializable`]: swissarmyhammer_common::lifecycle::Initializable
pub fn run_workspace_tools_init(
    root: &Path,
    scope: &InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut registry = InitRegistry::new();
    for tool_profile in WORKSPACE_TOOLS {
        registry.register(SkillDeployment::for_profile(
            root.to_path_buf(),
            *tool_profile,
        ));
    }
    registry.run_all_init(scope, reporter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;

    #[test]
    fn test_register_workspace_init_populates_two_components() {
        let mut registry = InitRegistry::new();
        register_workspace_init(&mut registry, Path::new("/tmp/example"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_run_workspace_init_returns_results_for_project_scope() {
        let temp = tempfile::TempDir::new().unwrap();
        let results = run_workspace_init(temp.path(), &InitScope::Project, &NullReporter);
        // Both components are applicable to project scope, so both emit results.
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    }

    #[test]
    fn test_run_workspace_tools_init_deploys_kanban_tool_skills_only() {
        use swissarmyhammer_common::lifecycle::InitStatus;

        let temp = tempfile::TempDir::new().unwrap();
        let results = run_workspace_tools_init(temp.path(), &InitScope::Project, &NullReporter);

        // One result per tool in the workspace set — currently just the kanban
        // tool's skill deployment.
        assert_eq!(results.len(), WORKSPACE_TOOLS.len());
        assert!(results.iter().all(|r| r.status != InitStatus::Error));

        let skills_dir = temp.path().join(".sah").join("skills");
        // The kanban tool's init deploys its profile skills under `.sah/skills/`.
        assert!(
            skills_dir.join("kanban").join("SKILL.md").is_file(),
            "kanban tool init must deploy the kanban-profile skills"
        );
        assert!(
            skills_dir.join("plan").join("SKILL.md").is_file(),
            "kanban tool init must deploy `plan`"
        );
        // No generic ProjectStructure step runs on this path, so `.prompts/` is
        // never created — the workspace is just its tools.
        assert!(
            !temp.path().join(".prompts").exists(),
            "run_workspace_tools_init must not run the generic ProjectStructure step"
        );
        // An untagged builtin (`commit`) is not part of the kanban tool.
        assert!(
            !skills_dir.join("commit").exists(),
            "untagged `commit` skill must not be deployed by the kanban tool"
        );
    }
}
