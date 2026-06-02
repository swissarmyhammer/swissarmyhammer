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
/// This is the kanban-app fast path — it deploys just the `kanban`-profile
/// cluster instead of all ~22 builtin skills. The operation is idempotent:
/// skills already current on disk are not rewritten.
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
}
