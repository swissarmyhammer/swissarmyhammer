//! Composable `Initializable` components for sah init/deinit.
//!
//! Most of sah's install lifecycle — the MCP server, builtin skills, builtin
//! agents, and the statusline — is installed declaratively through sah's
//! [`Profile`](mirdan::install::Profile) via
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`]
//! (see [`crate::commands::profile`]). The only install concern left here is
//! [`ProjectStructure`]: creating (and optionally removing) the `.sah/` +
//! `.prompts/` project workspace, which is not expressible as profile data
//! because it is a project-local filesystem scaffold rather than a per-agent
//! config edit.

use std::fs;
use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_common::SwissarmyhammerDirectory;

// ── ProjectStructure (priority 40) ───────────────────────────────────

/// Where [`ProjectStructure`] sits in the ascending `InitRegistry` ordering.
///
/// It runs after the profile's per-agent settings, and `sah init` registers
/// only one other component (the kanban tool, at 55), which must follow it.
const PROJECT_STRUCTURE_PRIORITY: i32 = 40;

/// The project runtime-state directory [`ProjectStructure`] creates and removes.
const SAH_DIR_NAME: &str = ".sah";

/// The project prompt-override directory [`ProjectStructure`] creates and
/// removes.
const PROMPTS_DIR_NAME: &str = ".prompts";

/// The workflow-definition subdirectory created inside [`SAH_DIR_NAME`].
const WORKFLOWS_SUBDIR_NAME: &str = "workflows";

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
///   statusline config, deployed agent definitions). All of those are
///   handled by sah's
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
#[derive(Debug)]
pub struct ProjectStructure {
    remove_directory: bool,
}

impl ProjectStructure {
    /// Create a new ProjectStructure component.
    pub fn new(remove_directory: bool) -> Self {
        Self { remove_directory }
    }

    /// The project root both lifecycle halves act on, or the single errored
    /// result they both report when it cannot be resolved.
    fn root_or_error(&self) -> Result<PathBuf, Vec<InitResult>> {
        workspace_root().map_err(|e| vec![InitResult::error(self.name(), e)])
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

    /// Component priority: [`PROJECT_STRUCTURE_PRIORITY`] — it runs after the
    /// profile's per-agent settings.
    fn priority(&self) -> i32 {
        PROJECT_STRUCTURE_PRIORITY
    }

    /// Only applicable to project and local scope installations.
    ///
    /// User scope is intentionally excluded — see the struct-level
    /// documentation on [`ProjectStructure`] for the rationale. In short:
    /// `sah init --user` installs per-agent config (settings, statusline,
    /// agents) but has no shared runtime artifacts of its own;
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
        let root = match self.root_or_error() {
            Ok(root) => root,
            Err(failure) => return failure,
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

        let root = match self.root_or_error() {
            Ok(root) => root,
            Err(failure) => return failure,
        };

        for dir_name in [SAH_DIR_NAME, PROMPTS_DIR_NAME] {
            if let Some(failure) =
                remove_directory_if_exists(&root, dir_name, self.name(), reporter)
            {
                return vec![failure];
            }
        }

        vec![InitResult::ok(self.name(), "Project directories removed")]
    }
}

/// Resolve the project root this component acts on: the git repository root,
/// else the process working directory.
///
/// [`ProjectStructure::init`] and [`ProjectStructure::deinit`] must agree on
/// this, or a `deinit` run from a subdirectory would look for a workspace that
/// `init` created at the git root.
fn workspace_root() -> Result<PathBuf, String> {
    if let Some(root) = swissarmyhammer_common::utils::find_git_repository_root() {
        return Ok(root);
    }
    std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))
}

/// Remove `<root>/<dir_name>` and report it, if the directory is there.
///
/// Returns `None` when the directory was removed or was already absent, and
/// `Some(error)` naming `component` when the removal failed — so the caller can
/// stop at the first failure.
fn remove_directory_if_exists(
    root: &Path,
    dir_name: &str,
    component: &str,
    reporter: &dyn InitReporter,
) -> Option<InitResult> {
    let dir = root.join(dir_name);
    if !dir.exists() {
        return None;
    }

    if let Err(e) = fs::remove_dir_all(&dir) {
        return Some(InitResult::error(
            component,
            format!("failed to remove {}: {e}", dir.display()),
        ));
    }

    reporter.emit(&InitEvent::Action {
        verb: "Removed".to_string(),
        message: dir.display().to_string(),
    });
    None
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
fn create_workspace_structure(root: &Path) -> Result<PathBuf, String> {
    let sah_dir = SwissarmyhammerDirectory::from_custom_root(root.to_path_buf())
        .map_err(|e| format!("failed to create {SAH_DIR_NAME} directory: {e}"))?;

    sah_dir
        .ensure_subdir(WORKFLOWS_SUBDIR_NAME)
        .map_err(|e| format!("failed to create {WORKFLOWS_SUBDIR_NAME} directory: {e}"))?;

    let prompts_dir = root.join(PROMPTS_DIR_NAME);
    fs::create_dir_all(&prompts_dir)
        .map_err(|e| format!("failed to create {PROMPTS_DIR_NAME} directory: {e}"))?;

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

    /// A reporter that keeps the emitted events so a test can assert what the
    /// removal announced, rather than only that it returned.
    #[derive(Default)]
    struct RecordingReporter {
        messages: std::sync::Mutex<Vec<String>>,
    }

    impl InitReporter for RecordingReporter {
        fn emit(&self, event: &InitEvent) {
            if let InitEvent::Action { verb, message } = event {
                self.messages
                    .lock()
                    .expect("recording reporter mutex")
                    .push(format!("{verb} {message}"));
            }
        }
    }

    /// A directory that is there is deleted, announced, and reported as no
    /// failure.
    #[test]
    fn test_remove_directory_if_exists_removes_and_reports() {
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join(SAH_DIR_NAME);
        fs::create_dir_all(target.join(WORKFLOWS_SUBDIR_NAME)).unwrap();

        let reporter = RecordingReporter::default();
        let failure =
            remove_directory_if_exists(temp.path(), SAH_DIR_NAME, "project-structure", &reporter);

        assert!(failure.is_none(), "removal should report no failure");
        assert!(!target.exists(), "{SAH_DIR_NAME} should be gone");
        assert_eq!(
            reporter.messages.lock().unwrap().as_slice(),
            [format!("Removed {}", target.display())]
        );
    }

    /// A directory that was never created is not a failure, and announces
    /// nothing.
    #[test]
    fn test_remove_directory_if_exists_ignores_missing_directory() {
        let temp = tempfile::TempDir::new().unwrap();

        let reporter = RecordingReporter::default();
        let failure = remove_directory_if_exists(
            temp.path(),
            PROMPTS_DIR_NAME,
            "project-structure",
            &reporter,
        );

        assert!(failure.is_none(), "a missing directory is not a failure");
        assert!(reporter.messages.lock().unwrap().is_empty());
    }

    /// The component's own lifecycle round-trips, and both halves target the
    /// same directory: run from a subdirectory of a repository, `init` creates
    /// the workspace at the repository root and `deinit` removes it from there
    /// again. A `deinit` that resolved the root differently from `init` would
    /// look in the subdirectory and leave the workspace behind.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_project_structure_round_trips_from_a_subdirectory() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let temp = tempfile::TempDir::new().unwrap();
        let repo = temp.path().canonicalize().unwrap();
        // A `.git` entry is all the root finder looks for.
        fs::create_dir_all(repo.join(".git")).unwrap();
        let subdir = repo.join("crates").join("inner");
        fs::create_dir_all(&subdir).unwrap();
        let _cwd = CurrentDirGuard::new(&subdir).unwrap();
        let reporter = RecordingReporter::default();

        let component = ProjectStructure::new(true);
        component.init(&InitScope::Project, &reporter);
        assert!(
            repo.join(SAH_DIR_NAME).is_dir(),
            "init creates {SAH_DIR_NAME} at the repository root"
        );
        assert!(
            repo.join(PROMPTS_DIR_NAME).is_dir(),
            "init creates {PROMPTS_DIR_NAME} at the repository root"
        );

        component.deinit(&InitScope::Project, &reporter);
        assert!(
            !repo.join(SAH_DIR_NAME).exists(),
            "deinit removes {SAH_DIR_NAME} from the repository root"
        );
        assert!(
            !repo.join(PROMPTS_DIR_NAME).exists(),
            "deinit removes {PROMPTS_DIR_NAME} from the repository root"
        );
    }
}
