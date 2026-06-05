//! Composable `Initializable` components for sah init/deinit.
//!
//! Most of sah's install lifecycle — the MCP server, builtin skills, builtin
//! agents, the statusline, and the CLAUDE.md preamble — is installed
//! declaratively through sah's [`Profile`](mirdan::install::Profile) via
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`]
//! (see [`crate::commands::profile`]). The only install concern left here is
//! [`ProjectStructure`]: creating (and optionally removing) the `.sah/` +
//! `.prompts/` project workspace, which is not expressible as profile data
//! because it is a project-local filesystem scaffold rather than a per-agent
//! config edit.

use std::fs;

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_common::SwissarmyhammerDirectory;

// ── ProjectStructure (priority 40) ───────────────────────────────────

/// Creates/removes the `.sah/` and `.prompts/` project directories.
///
/// # User-scope behavior
///
/// `is_applicable` deliberately matches only `Project | Local` and skips
/// `User` scope. There is no corresponding global `~/.sah/` or `~/.prompts/`
/// counterpart created by this component, and that is intentional:
///
/// * `sah init --user` is a **per-agent config install** — it edits each
///   detected agent's global settings (Claude `~/.claude/settings.json`,
///   the global `CLAUDE.md` preamble, statusline config, deployed agent
///   definitions). All of those are handled by sah's
///   [`Profile`](mirdan::install::Profile); user scope has no shared runtime
///   artifacts of its own.
/// * Runtime state — `.sah/workflows/`, prompt overrides, kanban boards,
///   code-context indexes — is **project-local** by design. It belongs
///   inside the project tree, not in `$HOME`.
/// * The few readers that *do* look under `~/.sah/` (e.g. global
///   `tools.yaml` in `swissarmyhammer-tools::mcp::tool_config`, statusline
///   overrides in `swissarmyhammer-statusline`, `~/.prompts/` in the
///   health registry) all treat those paths as **optional, lazy
///   fallbacks**: missing-is-fine, and the dirs that need to exist are
///   created on demand by the code that writes into them. Pre-creating an
///   empty `~/.sah/` here would add no behavior and would mislead a future
///   reader into thinking user scope has a shared runtime state directory.
///
/// If a future feature genuinely needs a global runtime directory under
/// `$HOME`, add a separate `GlobalUserStructure` component applicable to
/// `User` rather than widening this one — the two scopes have different
/// lifecycles and ownership.
pub struct ProjectStructure {
    remove_directory: bool,
}

impl ProjectStructure {
    /// Create a new ProjectStructure component.
    pub fn new(remove_directory: bool) -> Self {
        Self { remove_directory }
    }
}

impl Initializable for ProjectStructure {
    /// The component name for project structure creation/removal.
    fn name(&self) -> &str {
        "project-structure"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Project workspace"
    }

    /// Component category: structural setup tasks.
    fn category(&self) -> &str {
        "structure"
    }

    /// Component priority: 40 (runs after per-agent settings, before the preamble).
    fn priority(&self) -> i32 {
        40
    }

    /// Only applicable to project and local scope installations.
    ///
    /// User scope is intentionally excluded — see the struct-level
    /// documentation on [`ProjectStructure`] for the rationale. In short:
    /// `sah init --user` installs per-agent config (settings, preamble,
    /// statusline, agents) but has no shared runtime artifacts of its own;
    /// sah's runtime state (`.sah/workflows/`, prompts, kanban, indexes)
    /// is project-local.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Create the project directory structure with .prompts, .sah, and workflows.
    ///
    /// Resolves the project root (git root, else the current directory) and
    /// delegates the actual creation to the root-explicit
    /// [`create_workspace_structure`]. Root resolution stays here because the
    /// CLI is rooted at the process working directory by design; the creation
    /// itself is root-explicit so it is unit-testable without touching the
    /// process CWD.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let root = match swissarmyhammer_common::utils::find_git_repository_root() {
            Some(root) => root,
            None => match std::env::current_dir() {
                Ok(cwd) => cwd,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to get current directory: {}", e),
                    )];
                }
            },
        };

        let sah_root = match create_workspace_structure(&root) {
            Ok(sah_root) => sah_root,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        reporter.emit(&InitEvent::Action {
            verb: "Created".to_string(),
            message: format!("workspace structure at {}", sah_root.display()),
        });

        vec![InitResult::ok(
            self.name(),
            "Workspace structure initialized",
        )]
    }

    /// Remove `.sah/` and `.prompts/` directories if `remove_directory` is true.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        if !self.remove_directory {
            return vec![InitResult::skipped(
                self.name(),
                "Directory removal not requested",
            )];
        }

        let cwd = match std::env::current_dir() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to get current directory: {}", e),
                )];
            }
        };

        let sah_dir = cwd.join(".sah");
        if sah_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&sah_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to remove {}: {}", sah_dir.display(), e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("{}", sah_dir.display()),
            });
        }

        let prompts_dir = cwd.join(".prompts");
        if prompts_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&prompts_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to remove {}: {}", prompts_dir.display(), e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("{}", prompts_dir.display()),
            });
        }

        vec![InitResult::ok(self.name(), "Project directories removed")]
    }
}

/// Create `<root>/.sah/` (with its `workflows/` subdir) and `<root>/.prompts/`.
///
/// Root-explicit so it never reads or mutates the process working directory:
/// [`ProjectStructure::init`] resolves the root (git-root-then-CWD) and passes
/// it here. Idempotent — [`SwissarmyhammerDirectory::from_custom_root`],
/// `ensure_subdir`, and `create_dir_all` are all no-ops when the layout already
/// exists.
///
/// Returns the created `.sah/` root on success, or an error message describing
/// the first filesystem failure encountered.
fn create_workspace_structure(root: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let sah_dir = SwissarmyhammerDirectory::from_custom_root(root.to_path_buf())
        .map_err(|e| format!("Failed to create .sah directory: {}", e))?;

    sah_dir
        .ensure_subdir("workflows")
        .map_err(|e| format!("Failed to create workflows directory: {}", e))?;

    let prompts_dir = root.join(".prompts");
    fs::create_dir_all(&prompts_dir)
        .map_err(|e| format!("Failed to create .prompts directory: {}", e))?;

    Ok(sah_dir.root().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_structure_name_and_priority() {
        let component = ProjectStructure::new(false);
        assert_eq!(Initializable::name(&component), "project-structure");
        assert_eq!(Initializable::display_name(&component), "Project workspace");
        assert_eq!(component.category(), "structure");
        assert_eq!(component.priority(), 40);
    }

    #[test]
    fn test_project_structure_skips_user_scope() {
        let component = ProjectStructure::new(false);
        assert!(component.is_applicable(&InitScope::Project));
        assert!(component.is_applicable(&InitScope::Local));
        assert!(!component.is_applicable(&InitScope::User));
    }

    #[test]
    fn test_project_structure_deinit_skips_without_remove_directory() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;

        let component = ProjectStructure::new(false);
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, InitStatus::Skipped);
    }

    #[test]
    fn test_create_workspace_structure_creates_layout_under_explicit_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let sah_root = create_workspace_structure(temp.path()).unwrap();

        assert!(temp.path().join(".sah").is_dir(), ".sah/ should exist");
        assert!(
            temp.path().join(".sah").join("workflows").is_dir(),
            ".sah/workflows/ should exist"
        );
        assert!(
            temp.path().join(".prompts").is_dir(),
            ".prompts/ should exist"
        );
        assert!(
            sah_root.ends_with(".sah"),
            "returned root should be the .sah/ directory, got {}",
            sah_root.display()
        );
    }

    #[test]
    fn test_create_workspace_structure_is_idempotent() {
        let temp = tempfile::TempDir::new().unwrap();
        // Re-running on an already-initialized workspace must not error.
        create_workspace_structure(temp.path()).unwrap();
        create_workspace_structure(temp.path()).unwrap();
        assert!(temp.path().join(".sah").join("workflows").is_dir());
        assert!(temp.path().join(".prompts").is_dir());
    }
}
