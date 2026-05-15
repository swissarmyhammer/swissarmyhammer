//! Shelltool Doctor — Diagnostic checks for shelltool setup and configuration.
//!
//! Checks:
//! - ShellExecuteTool health checks (config, patterns, Bash denied, skill deployed)
//! - shelltool binary in PATH
//! - Git repository (warning if not found)

use std::env;
use std::path::PathBuf;

use swissarmyhammer_common::health::{Doctorable, HealthStatus};
use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// Shelltool diagnostic runner.
pub struct ShelltoolDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for ShelltoolDoctor {
    /// Returns immutable reference to accumulated checks.
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// Returns mutable reference to accumulated checks.
    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl ShelltoolDoctor {
    /// Create a new ShelltoolDoctor instance.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all shelltool diagnostic checks.
    ///
    /// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
    pub fn run_diagnostics(&mut self) -> i32 {
        self.check_git_repository();
        self.check_shelltool_in_path();
        self.check_shell_tool_health();

        self.get_exit_code()
    }

    /// Check if we're in a Git repository.
    ///
    /// This is a warning (not error) since shelltool can work outside git repos.
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

    /// Check if the shelltool binary is in PATH.
    fn check_shelltool_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) {
            "shelltool.exe"
        } else {
            "shelltool"
        };
        let mut found = false;
        let mut found_path = None;

        for path in paths {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                found = true;
                found_path = Some(exe_path);
                break;
            }
        }

        if found {
            self.add_check(Check {
                name: "shelltool in PATH".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", found_path.unwrap().display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "shelltool in PATH".to_string(),
                status: CheckStatus::Warning,
                message: "shelltool not found in PATH".to_string(),
                fix: Some(
                    "Add shelltool to your PATH or install with `cargo install --path shelltool-cli`"
                        .to_string(),
                ),
            });
        }
    }

    /// Run ShellExecuteTool health checks via the Doctorable trait.
    ///
    /// Converts each HealthCheck from the tool into a Check for the doctor runner.
    fn check_shell_tool_health(&mut self) {
        let tool = ShellExecuteTool::new();
        let health_checks = tool.run_health_checks();

        for health_check in health_checks {
            let status = match health_check.status {
                HealthStatus::Ok => CheckStatus::Ok,
                HealthStatus::Warning => CheckStatus::Warning,
                HealthStatus::Error => CheckStatus::Error,
            };

            self.add_check(Check {
                name: health_check.name,
                status,
                message: health_check.message,
                fix: health_check.fix,
            });
        }
    }
}

impl Default for ShelltoolDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the doctor command and display results.
///
/// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
pub fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = ShelltoolDoctor::new();
    let exit_code = doctor.run_diagnostics();
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    /// RAII guard that restores `env::current_dir` on drop.
    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        /// Capture the current working directory so it can be restored later.
        fn capture() -> Self {
            Self {
                original: env::current_dir().expect("current_dir must be readable"),
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            // Best-effort restore; ignore errors during unwind.
            let _ = env::set_current_dir(&self.original);
        }
    }

    /// RAII guard that restores the `PATH` env var on drop.
    struct PathEnvGuard {
        original: Option<String>,
    }

    impl PathEnvGuard {
        /// Capture the current `PATH` value so it can be restored later.
        fn capture() -> Self {
            Self {
                original: env::var("PATH").ok(),
            }
        }
    }

    impl Drop for PathEnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => env::set_var("PATH", value),
                None => env::remove_var("PATH"),
            }
        }
    }

    #[test]
    fn test_new() {
        let doctor = ShelltoolDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    #[tokio::test]
    async fn test_run_diagnostics() {
        let mut doctor = ShelltoolDoctor::new();
        let exit_code = doctor.run_diagnostics();

        // Should have at least 3 checks: git, path, shell tool health checks
        assert!(!doctor.checks().is_empty());
        assert!(doctor.checks().len() >= 3);

        // Exit code should be 0, 1, or 2
        assert!(exit_code <= 2);
    }

    #[tokio::test]
    async fn test_run_doctor() {
        // This will print to stdout, but we just verify it doesn't panic
        // and returns a valid exit code
        let exit_code = run_doctor(false);
        assert!(exit_code <= 2);
    }

    #[tokio::test]
    async fn test_run_doctor_verbose() {
        // Test verbose mode
        let exit_code = run_doctor(true);
        assert!(exit_code <= 2);
    }

    #[test]
    fn test_default() {
        let doctor = ShelltoolDoctor::default();
        assert!(doctor.checks().is_empty());
    }

    #[test]
    fn test_check_git_repository() {
        let mut doctor = ShelltoolDoctor::new();
        doctor.check_git_repository();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Git Repository");
        // Status depends on whether we're actually in a git repo
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_shelltool_in_path() {
        let mut doctor = ShelltoolDoctor::new();
        doctor.check_shelltool_in_path();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "shelltool in PATH");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[tokio::test]
    async fn test_check_shell_tool_health() {
        let mut doctor = ShelltoolDoctor::new();
        doctor.check_shell_tool_health();

        // Should produce at least some checks from ShellExecuteTool
        assert!(!doctor.checks().is_empty());
    }

    /// Exercises the `None` arm of `check_git_repository` by running the
    /// check from a tempdir with no `.git` in any ancestor.
    #[test]
    #[serial(env)]
    fn test_check_git_repository_not_in_git() {
        let _cwd = CwdGuard::capture();

        // Canonicalize to resolve any symlinks (e.g. /tmp -> /private/tmp on
        // macOS) so `find_git_repository_root` walks the real ancestor chain.
        let tmp = TempDir::new().expect("create tempdir");
        let tmp_path = tmp
            .path()
            .canonicalize()
            .expect("canonicalize tempdir path");
        env::set_current_dir(&tmp_path).expect("set_current_dir to tempdir");

        let mut doctor = ShelltoolDoctor::new();
        doctor.check_git_repository();

        assert_eq!(doctor.checks().len(), 1);
        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Git Repository");
        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.message, "Not in a Git repository");
        assert_eq!(
            check.fix.as_deref(),
            Some("Run from within a Git repository or run `git init`")
        );
    }

    /// Exercises the not-found arm of `check_shelltool_in_path` by pointing
    /// `PATH` at an empty tempdir that cannot contain the shelltool binary.
    #[test]
    #[serial(env)]
    fn test_check_shelltool_in_path_not_found() {
        let _path_env = PathEnvGuard::capture();

        let tmp = TempDir::new().expect("create tempdir");
        env::set_var("PATH", tmp.path());

        let mut doctor = ShelltoolDoctor::new();
        doctor.check_shelltool_in_path();

        assert_eq!(doctor.checks().len(), 1);
        let check = &doctor.checks()[0];
        assert_eq!(check.name, "shelltool in PATH");
        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.message, "shelltool not found in PATH");
        assert_eq!(
            check.fix.as_deref(),
            Some("Add shelltool to your PATH or install with `cargo install --path shelltool-cli`")
        );
    }
}
