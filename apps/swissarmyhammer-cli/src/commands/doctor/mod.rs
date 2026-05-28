//! Doctor command implementation
//!
//! Diagnoses configuration and setup issues for swissarmyhammer.
//!
//! CLI-specific checks (installation, PATH, Claude config) live here.
//! Tool-specific health checks are provided by each tool's `Doctorable` impl
//! and collected via `swissarmyhammer_tools::collect_all_health_checks()`.

use crate::exit_codes::EXIT_ERROR;
use anyhow::Result;
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_doctor::DoctorRunner;

// Re-export types from submodules
pub use types::*;

/// Check implementations for installation, PATH, Claude config, AVP, and LSP.
pub mod checks;
/// Display formatting for doctor check results.
pub mod display;
/// Type definitions and re-exports for the doctor module.
pub mod types;
/// Utility helpers shared across doctor checks.
pub mod utils;

/// Main diagnostic tool for SwissArmyHammer system health checks
///
/// The Doctor struct accumulates diagnostic results and provides a summary
/// of the system's configuration and any potential issues.
pub struct Doctor {
    checks: Vec<Check>,
}

impl DoctorRunner for Doctor {
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl Doctor {
    /// Create a new Doctor instance for running diagnostics
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run diagnostic checks without printing results (for CliContext integration)
    ///
    /// A surrounding Git repository is informational, not required: the only
    /// project-scoped check (the `.sah` directory) runs when a repo is detected.
    /// The install-stack checks (preamble, permissions, skills, agents, MCP)
    /// cover both project and user scope agent-agnostically and always run. In a
    /// user-mode install (e.g. running `sah doctor` from `~`) the missing repo is
    /// reported as a Warning and all scope-independent checks still run. The exit
    /// code is driven solely by the resulting check statuses.
    pub async fn run_diagnostics_without_output(&mut self) -> Result<i32> {
        use swissarmyhammer_common::utils::find_git_repository_root;

        let git_root: Option<std::path::PathBuf> = find_git_repository_root();
        match &git_root {
            Some(path) => self.checks.push(Check {
                name: "Git Repository".to_string(),
                status: CheckStatus::Ok,
                message: format!("Detected at {}", path.display()),
                fix: None,
            }),
            None => self.checks.push(Check {
                name: "Git Repository".to_string(),
                status: CheckStatus::Warning,
                message: "Not inside a Git repository; project-scoped checks skipped".to_string(),
                fix: Some(
                    "Run from within a Git repository (or `git init`) to enable project checks"
                        .to_string(),
                ),
            }),
        }

        // Project-scoped checks only make sense with a repository root.
        if let Some(root) = &git_root {
            self.check_swissarmyhammer_directory(root)?;
        }

        // Scope-independent checks always run.
        self.run_system_checks()?;
        self.run_tool_health_checks().await?;
        self.run_configuration_checks()?;
        self.run_install_stack_checks()?;

        // Return exit code without printing results
        Ok(self.get_exit_code())
    }

    /// Run system checks
    fn run_system_checks(&mut self) -> Result<()> {
        checks::check_installation(&mut self.checks)?;
        checks::check_in_path(&mut self.checks)?;
        checks::check_file_permissions(&mut self.checks)?;
        checks::check_lsp_servers(&mut self.checks)?;
        checks::check_avp_in_path(&mut self.checks)?;
        checks::check_avp_hooks(&mut self.checks)?;
        Ok(())
    }

    /// Run tool health checks using the Doctorable trait
    async fn run_tool_health_checks(&mut self) -> Result<()> {
        use swissarmyhammer_common::health::HealthStatus;

        // Collect all health checks from registered MCP tools
        let health_checks = swissarmyhammer_tools::collect_all_health_checks().await;

        // Convert HealthCheck to Check format
        for health_check in health_checks {
            let status = match health_check.status {
                HealthStatus::Ok => CheckStatus::Ok,
                HealthStatus::Warning => CheckStatus::Warning,
                HealthStatus::Error => CheckStatus::Error,
            };

            self.checks.push(Check {
                name: health_check.name,
                status,
                message: health_check.message,
                fix: health_check.fix,
            });
        }

        Ok(())
    }

    /// Run scope-independent configuration checks (Claude Code CLI/config).
    ///
    /// This is the runtime probe (`claude mcp list`) that confirms the agent
    /// actually loads sah; it is complementary to the install-stack checks, which
    /// inspect on-disk artifacts.
    fn run_configuration_checks(&mut self) -> Result<()> {
        checks::check_claude_config(&mut self.checks)?;
        Ok(())
    }

    /// Run the agent-agnostic install-stack checks for project and user scope.
    ///
    /// Reports one row per applicable (agent, scope, component) — preamble,
    /// permissions, skills, subagents, and MCP — for every detected agent. This
    /// is scope-independent and runs regardless of whether a Git repository is
    /// present, so user-scope rows surface even from `~`.
    fn run_install_stack_checks(&mut self) -> Result<()> {
        checks::check_install_stack(&mut self.checks)?;
        Ok(())
    }

    /// Check SwissArmyHammer directory in Git repository
    fn check_swissarmyhammer_directory(&mut self, git_root: &std::path::Path) -> Result<()> {
        let swissarmyhammer_dir = git_root.join(SwissarmyhammerDirectory::dir_name());

        if !swissarmyhammer_dir.exists() {
            self.checks.push(Check {
                name: "SwissArmyHammer Directory".to_string(),
                status: CheckStatus::Warning,
                message: "Directory does not exist (will be created when needed)".to_string(),
                fix: Some("Directory will be created automatically when first needed".to_string()),
            });
            return Ok(());
        }

        self.checks.push(Check {
            name: "SwissArmyHammer Directory".to_string(),
            status: CheckStatus::Ok,
            message: format!("Found at {}", swissarmyhammer_dir.display()),
            fix: None,
        });

        self.check_directory_access(&swissarmyhammer_dir);
        Ok(())
    }

    /// Check directory access permissions and writability.
    fn check_directory_access(&mut self, dir: &std::path::Path) {
        let metadata = match std::fs::metadata(dir) {
            Ok(m) => m,
            Err(e) => {
                self.checks.push(Check {
                    name: "Directory Access".to_string(),
                    status: CheckStatus::Error,
                    message: format!("Cannot access .sah directory: {}", e),
                    fix: Some("Check file permissions and ownership".to_string()),
                });
                return;
            }
        };

        if !metadata.is_dir() {
            self.checks.push(Check {
                name: "Directory Type".to_string(),
                status: CheckStatus::Error,
                message: ".sah exists but is not a directory".to_string(),
                fix: Some(
                    "Remove the file and let SwissArmyHammer recreate it as a directory"
                        .to_string(),
                ),
            });
            return;
        }

        self.checks.push(Check {
            name: "Directory Access".to_string(),
            status: CheckStatus::Ok,
            message: "Directory is accessible".to_string(),
            fix: None,
        });

        self.checks.push(check_directory_writability(dir));
    }
}

/// Check if a directory is writable by creating and removing a test file.
fn check_directory_writability(dir: &std::path::Path) -> Check {
    let test_file = dir.join(".doctor_test");
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            Check {
                name: "Directory Write Access".to_string(),
                status: CheckStatus::Ok,
                message: "Directory is writable".to_string(),
                fix: None,
            }
        }
        Err(_) => Check {
            name: "Directory Write Access".to_string(),
            status: CheckStatus::Warning,
            message: "Directory may not be writable".to_string(),
            fix: Some("Check directory permissions".to_string()),
        },
    }
}

impl Default for Doctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle the doctor command
pub async fn handle_command(cli_context: &crate::context::CliContext) -> i32 {
    let mut doctor = Doctor::new();

    match run_doctor_diagnostics(&mut doctor, cli_context).await {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Doctor command failed: {}", e);
            EXIT_ERROR
        }
    }
}

/// Run diagnostic checks and display results using CliContext
async fn run_doctor_diagnostics(
    doctor: &mut Doctor,
    cli_context: &crate::context::CliContext,
) -> Result<i32> {
    // Run all diagnostics without output
    let exit_code = doctor.run_diagnostics_without_output().await?;

    // Print the checks table using the shared crate
    doctor.print_table(cli_context.verbose);

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    #[test]
    fn test_doctor_creation() {
        let doctor = Doctor::new();
        assert_eq!(doctor.checks.len(), 0);
    }

    /// `run_diagnostics_without_output` reads process-global CWD — via
    /// `find_git_repository_root()` and the CWD-relative checks in `checks.rs`
    /// (`check_lsp_servers`, `check_file_permissions`). `#[serial_test::serial(cwd)]`
    /// joins the crate-wide `cwd` group so it cannot run concurrently with any
    /// CWD-mutating test.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_run_diagnostics() {
        // Isolate HOME + CWD — `run_diagnostics_without_output` exercises the
        // full check pipeline, which inspects/creates `.sah/` and other on-disk
        // artifacts relative to cwd and would otherwise leak them into the host
        // crate directory. Mirrors the pattern in
        // `commands::registry::tests::test_init_runs_without_panic`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_without_output().await;
        assert!(result.is_ok());

        // Should have at least some checks
        assert!(!doctor.checks.is_empty());

        // Exit code should be 0, 1, or 2
        let exit_code = doctor.get_exit_code();
        assert!(exit_code <= 2);
    }

    /// Regression: three Claude-only, project-scope-blind legacy health checks
    /// were deleted in favor of mirdan's scope-aware install stack. The full
    /// doctor pipeline must not emit any of them.
    ///
    /// See kanban 01KSMXKZM1NZV1QH0SSKAP0V4P.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_deleted_legacy_checks_absent() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_without_output().await;
        assert!(result.is_ok(), "diagnostics should succeed");

        // Positive control: without this, the absence assertions below would
        // pass vacuously if the pipeline ever returned an empty `checks` Vec
        // (e.g. due to a panic-swallowing error path or accidental
        // short-circuit). Mirrors the assertion in `test_run_diagnostics`.
        assert!(
            !doctor.checks.is_empty(),
            "doctor pipeline produced no checks — regression assertion would pass vacuously"
        );

        let deleted = ["Skills installation", "Bash denied", "Shell skill deployed"];
        for name in deleted {
            assert!(
                !doctor.checks.iter().any(|c| c.name == name),
                "deleted legacy check '{}' must not appear in doctor.checks()",
                name
            );
        }
    }

    /// In a user-mode install (no surrounding Git repository), doctor must not
    /// hard-fail: it should record the missing repo as a Warning, still run all
    /// scope-independent checks, and not short-circuit with an error exit code.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_run_diagnostics_outside_git_repo() {
        let _env = IsolatedTestEnvironment::new().expect("isolated env");

        // A temp dir with no .git anywhere up the tree.
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let _cwd_guard = CurrentDirGuard::new(temp_dir.path()).expect("cwd guard");

        // Guard against the temp dir somehow resolving inside a repo (e.g. /tmp
        // being part of one): if a git root is still found, the premise of this
        // test does not hold and we skip rather than assert a false negative.
        if swissarmyhammer_common::utils::find_git_repository_root().is_some() {
            return;
        }

        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_without_output().await;

        // Must not short-circuit / error out just because there's no repo.
        assert!(result.is_ok(), "diagnostics should succeed outside a repo");

        // Exactly one "Git Repository" check, and it must be a Warning.
        let git_checks: Vec<&Check> = doctor
            .checks
            .iter()
            .filter(|c| c.name == "Git Repository")
            .collect();
        assert_eq!(git_checks.len(), 1, "expected one Git Repository check");
        assert_eq!(
            git_checks[0].status,
            CheckStatus::Warning,
            "missing repo should be a Warning, not an Error"
        );

        // The missing repo alone must not produce any Error-status check.
        assert!(
            !doctor.checks.iter().any(|c| c.status == CheckStatus::Error),
            "missing git repo must not add Error-status checks"
        );

        // Scope-independent checks still ran (e.g. installation method).
        assert!(
            doctor
                .checks
                .iter()
                .any(|c| c.name == checks::check_names::INSTALLATION_METHOD),
            "scope-independent installation check should still be present"
        );

        // Project-scoped checks must be skipped (no .sah directory check).
        assert!(
            !doctor
                .checks
                .iter()
                .any(|c| c.name == "SwissArmyHammer Directory"),
            "project-scoped .sah check should be skipped outside a repo"
        );
    }
}
