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

pub mod checks;
pub mod display;
pub mod types;
pub mod utils;

/// Help text for the doctor command
pub const DESCRIPTION: &str = include_str!("description.md");

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
    pub async fn run_diagnostics_without_output(&mut self) -> Result<i32> {
        // First, ensure we're in a Git repository
        use swissarmyhammer_common::utils::find_git_repository_root;

        let git_root = match find_git_repository_root() {
            Some(path) => {
                self.checks.push(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Detected at {}", path.display()),
                    fix: None,
                });
                path
            }
            None => {
                self.checks.push(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Error,
                    message: "SwissArmyHammer requires a Git repository".to_string(),
                    fix: Some("Run this command from within a Git repository or create one with: git init".to_string()),
                });
                return Ok(ExitCode::Error.into());
            }
        };

        // Check .swissarmyhammer directory
        self.check_swissarmyhammer_directory(&git_root)?;

        // Run all checks
        self.run_system_checks()?;
        self.run_tool_health_checks().await?;
        self.run_configuration_checks()?;

        // Return exit code without printing results
        Ok(self.get_exit_code())
    }

    /// Run system checks
    fn run_system_checks(&mut self) -> Result<()> {
        checks::check_installation(&mut self.checks)?;
        checks::check_in_path(&mut self.checks)?;
        checks::check_file_permissions(&mut self.checks)?;
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

    /// Run configuration checks
    fn run_configuration_checks(&mut self) -> Result<()> {
        checks::check_claude_config(&mut self.checks)?;
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

        // Check directory permissions
        match std::fs::metadata(&swissarmyhammer_dir) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    self.checks.push(Check {
                        name: "Directory Access".to_string(),
                        status: CheckStatus::Ok,
                        message: "Directory is accessible".to_string(),
                        fix: None,
                    });

                    // Check if directory is writable by trying to create a test file
                    let test_file = swissarmyhammer_dir.join(".doctor_test");
                    match std::fs::write(&test_file, "test") {
                        Ok(_) => {
                            let _ = std::fs::remove_file(&test_file); // Clean up
                            self.checks.push(Check {
                                name: "Directory Write Access".to_string(),
                                status: CheckStatus::Ok,
                                message: "Directory is writable".to_string(),
                                fix: None,
                            });
                        }
                        Err(_) => {
                            self.checks.push(Check {
                                name: "Directory Write Access".to_string(),
                                status: CheckStatus::Warning,
                                message: "Directory may not be writable".to_string(),
                                fix: Some("Check directory permissions".to_string()),
                            });
                        }
                    }
                } else {
                    self.checks.push(Check {
                        name: "Directory Type".to_string(),
                        status: CheckStatus::Error,
                        message: ".swissarmyhammer exists but is not a directory".to_string(),
                        fix: Some(
                            "Remove the file and let SwissArmyHammer recreate it as a directory"
                                .to_string(),
                        ),
                    });
                }
            }
            Err(e) => {
                self.checks.push(Check {
                    name: "Directory Access".to_string(),
                    status: CheckStatus::Error,
                    message: format!("Cannot access .swissarmyhammer directory: {}", e),
                    fix: Some("Check file permissions and ownership".to_string()),
                });
            }
        }

        Ok(())
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

    #[test]
    fn test_doctor_creation() {
        let doctor = Doctor::new();
        assert_eq!(doctor.checks.len(), 0);
    }

    #[tokio::test]
    async fn test_run_diagnostics() {
        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_without_output().await;
        assert!(result.is_ok());

        // Should have at least some checks
        assert!(!doctor.checks.is_empty());

        // Exit code should be 0, 1, or 2
        let exit_code = doctor.get_exit_code();
        assert!(exit_code <= 2);
    }

    #[tokio::test]
    async fn test_workflow_diagnostics_in_run_diagnostics_without_output() {
        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_without_output().await;
        assert!(result.is_ok());

        // Should have workflow-related checks in the full diagnostics
        let workflow_checks: Vec<_> = doctor
            .checks
            .iter()
            .filter(|c| c.name.contains("Workflow") || c.name.contains("workflow"))
            .collect();
        assert!(
            !workflow_checks.is_empty(),
            "run_diagnostics_without_output should include workflow checks"
        );
    }
}
