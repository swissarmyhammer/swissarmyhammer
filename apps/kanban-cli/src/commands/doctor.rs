//! Kanban Doctor — Diagnostic checks for kanban setup and configuration.
//!
//! Checks:
//! - Git repository (warning if not found)
//! - `kanban` binary in PATH
//! - Kanban board initialized under `.kanban/` in the current working directory
//!
//! Modeled on the same `DoctorRunner` pattern as `shelltool-cli` and `avp-cli`
//! so the three CLI doctors stay structurally consistent.

use std::env;
use std::path::PathBuf;

use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};
use swissarmyhammer_kanban::KanbanContext;

/// Kanban diagnostic runner.
///
/// Accumulates [`Check`] results for the three kanban-specific diagnostics
/// (git repo, binary on PATH, board initialized) and exposes the shared
/// [`DoctorRunner`] helpers for exit-code computation and table printing.
pub struct KanbanDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for KanbanDoctor {
    /// Returns immutable reference to accumulated checks.
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// Returns mutable reference to accumulated checks.
    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl KanbanDoctor {
    /// Create a new `KanbanDoctor` with no checks.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all kanban diagnostic checks.
    ///
    /// Runs the three checks in order (git repo, PATH, board initialized)
    /// and returns an exit code derived from the accumulated results:
    /// 0 for success, 1 for warnings, 2 for errors.
    pub fn run_diagnostics(&mut self) -> i32 {
        self.check_git_repository();
        self.check_kanban_in_path();
        self.check_board_initialized();

        self.get_exit_code()
    }

    /// Check if we're in a Git repository.
    ///
    /// This is a warning (not error) since kanban boards can live outside
    /// git repositories — but most workflows assume a repo-scoped `.kanban/`.
    fn check_git_repository(&mut self) {
        use swissarmyhammer_common::utils::find_git_repository_root;

        match find_git_repository_root() {
            Some(path) => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Detected at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Warning,
                    message: "Not in a Git repository".to_string(),
                    fix: Some("Run from within a Git repository or run `git init`".to_string()),
                });
            }
        }
    }

    /// Check if the `kanban` binary is in `PATH`.
    ///
    /// Warning (not error): the user may be running a freshly-built binary
    /// directly from `target/`, but anyone calling the CLI by name needs it
    /// on `PATH`.
    fn check_kanban_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) {
            "kanban.exe"
        } else {
            "kanban"
        };

        let mut found_path = None;
        for path in paths {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                found_path = Some(exe_path);
                break;
            }
        }

        match found_path {
            Some(path) => {
                self.add_check(Check {
                    name: "kanban in PATH".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Found at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "kanban in PATH".to_string(),
                    status: CheckStatus::Warning,
                    message: "kanban not found in PATH".to_string(),
                    fix: Some(
                        "Add kanban to your PATH or install with `cargo install --path kanban-cli`"
                            .to_string(),
                    ),
                });
            }
        }
    }

    /// Check if a kanban board is initialized in the current working directory.
    ///
    /// Delegates to [`KanbanContext::is_initialized`], which is the canonical
    /// "is this board initialized?" predicate used by the rest of the kanban
    /// crate. It accepts any of the supported on-disk layouts:
    ///
    /// - `<cwd>/.kanban/boards/board.yaml` (current entity layout — what
    ///   `init board` writes today)
    /// - `<cwd>/.kanban/board.yaml` (legacy single-file layout)
    /// - `<cwd>/.kanban/board.json` (very old legacy layout)
    ///
    /// Missing board files are reported as a warning, not an error, because
    /// many kanban commands (like `open` or `serve`) are useful even before a
    /// board has been initialized.
    fn check_board_initialized(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let kanban_root = cwd.join(".kanban");
        let ctx = KanbanContext::new(&kanban_root);

        if ctx.is_initialized() {
            self.add_check(Check {
                name: "Board Initialized".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", kanban_root.display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "Board Initialized".to_string(),
                status: CheckStatus::Warning,
                message: "No kanban board found in .kanban/".to_string(),
                fix: Some(
                    "Run `kanban board init --name \"<board name>\"` to create a board".to_string(),
                ),
            });
        }
    }
}

impl Default for KanbanDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the doctor command and display results.
///
/// Builds a fresh [`KanbanDoctor`], runs all diagnostics, prints the results
/// as a formatted table (via [`DoctorRunner::print_table`]), and returns the
/// exit code: 0 for success, 1 for warnings, 2 for errors.
pub fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = KanbanDoctor::new();
    let exit_code = doctor.run_diagnostics();
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A freshly-constructed `KanbanDoctor` must have no accumulated checks.
    /// This is the contract every downstream test relies on — if `new()`
    /// accidentally seeded any checks, counts like "exactly one check" in
    /// the per-check tests below would silently be wrong.
    #[test]
    fn new_starts_with_empty_checks() {
        let doctor = KanbanDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    /// `Default` must behave identically to `new()` — both produce an empty
    /// doctor. Kept as a guard against drift if one constructor is changed
    /// without updating the other.
    #[test]
    fn default_matches_new() {
        let doctor = KanbanDoctor::default();
        assert!(doctor.checks().is_empty());
    }

    /// `check_git_repository` must produce exactly one check named
    /// "Git Repository". The status is host-dependent (Ok inside a git repo,
    /// Warning outside) so we accept either — the hard contract is the count
    /// and name.
    #[test]
    fn check_git_repository_produces_one_check() {
        let mut doctor = KanbanDoctor::new();
        doctor.check_git_repository();

        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Git Repository");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    /// `check_kanban_in_path` must produce exactly one check named
    /// "kanban in PATH". Like the git check, the status depends on whether
    /// `kanban` happens to be installed on the host running the tests.
    #[test]
    fn check_kanban_in_path_produces_one_check() {
        let mut doctor = KanbanDoctor::new();
        doctor.check_kanban_in_path();

        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "kanban in PATH");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    /// `check_board_initialized` must produce exactly one check named
    /// "Board Initialized". The status depends on whether the `.kanban/`
    /// directory in the test's CWD happens to contain a board file; we
    /// only assert shape here.
    #[test]
    fn check_board_initialized_produces_one_check() {
        let mut doctor = KanbanDoctor::new();
        doctor.check_board_initialized();

        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Board Initialized");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    /// When the CWD contains `.kanban/boards/board.yaml` (the canonical
    /// entity layout written by `init board`), `check_board_initialized`
    /// must report `Ok`. This is the regression guard against the original
    /// bug, where the doctor only knew about the legacy single-file layout
    /// `<root>/board.yaml` and emitted a false-negative warning inside any
    /// repo that uses the entity layout.
    ///
    /// Uses `CurrentDirGuard` + `#[serial]` per `feedback_test_isolation.md`
    /// so this test cannot race with other tests that read or mutate CWD.
    #[test]
    #[serial_test::serial]
    fn check_board_initialized_recognizes_entity_layout() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;
        use tempfile::TempDir;

        let temp = TempDir::new().expect("create tempdir");
        let boards_dir = temp.path().join(".kanban").join("boards");
        std::fs::create_dir_all(&boards_dir).expect("create .kanban/boards");
        std::fs::write(boards_dir.join("board.yaml"), "name: Test Board\n")
            .expect("write board.yaml");

        let _guard = CurrentDirGuard::new(temp.path()).expect("enter tempdir");

        let mut doctor = KanbanDoctor::new();
        doctor.check_board_initialized();

        assert_eq!(doctor.checks().len(), 1);
        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Board Initialized");
        assert_eq!(
            check.status,
            CheckStatus::Ok,
            "expected Ok when .kanban/boards/board.yaml exists, got {:?} (message: {})",
            check.status,
            check.message,
        );
    }

    /// When the CWD contains no `.kanban/` directory at all,
    /// `check_board_initialized` must report a `Warning` with a fix
    /// suggesting the actual `kanban board init` verb. The fix string is
    /// load-bearing: it must match the real CLI subcommand path so users
    /// who copy-paste it actually create a board.
    #[test]
    #[serial_test::serial]
    fn check_board_initialized_warns_when_no_kanban_dir() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;
        use tempfile::TempDir;

        let temp = TempDir::new().expect("create tempdir");
        let _guard = CurrentDirGuard::new(temp.path()).expect("enter tempdir");

        let mut doctor = KanbanDoctor::new();
        doctor.check_board_initialized();

        assert_eq!(doctor.checks().len(), 1);
        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Board Initialized");
        assert_eq!(
            check.status,
            CheckStatus::Warning,
            "expected Warning when no .kanban/ dir exists, got {:?} (message: {})",
            check.status,
            check.message,
        );
        let fix = check.fix.as_deref().unwrap_or("");
        assert!(
            fix.contains("kanban board init"),
            "fix string must reference the actual CLI verb `kanban board init`, got: {fix}"
        );
    }

    /// `run_diagnostics` must run all three checks and yield a valid exit
    /// code in `0..=2`. The three checks are the documented suite; bumping
    /// past three should be a deliberate change flagged by this test.
    #[test]
    fn run_diagnostics_runs_all_three_checks() {
        let mut doctor = KanbanDoctor::new();
        let exit_code = doctor.run_diagnostics();

        assert_eq!(doctor.checks().len(), 3);
        assert!(exit_code <= 2);
    }

    /// `run_doctor(false)` must return a valid exit code (0, 1, or 2). This
    /// is the user-facing entry point wired into `main.rs`; it must stay
    /// safe to call in any environment, printing only — never panicking.
    #[test]
    fn run_doctor_non_verbose_returns_valid_exit_code() {
        let exit_code = run_doctor(false);
        assert!(exit_code <= 2);
    }

    /// `run_doctor(true)` exercises the verbose printing path alongside the
    /// non-verbose variant above. Any panic in the verbose table renderer
    /// would surface here.
    #[test]
    fn run_doctor_verbose_returns_valid_exit_code() {
        let exit_code = run_doctor(true);
        assert!(exit_code <= 2);
    }
}
