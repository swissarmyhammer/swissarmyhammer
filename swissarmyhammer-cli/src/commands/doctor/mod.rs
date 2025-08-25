//! Doctor command implementation
//!
//! Diagnoses configuration and setup issues for swissarmyhammer
//!
//! This module provides comprehensive system diagnostics for SwissArmyHammer installations,
//! checking various aspects of the system configuration to ensure optimal operation.
//!
//! # Features
//!
//! - Installation verification (binary permissions, PATH configuration)
//! - Claude Code MCP integration checking
//! - Prompt directory validation
//! - YAML front matter parsing verification
//! - Workflow system diagnostics
//! - Disk space monitoring
//! - File permission checks

use crate::exit_codes::EXIT_ERROR;
use anyhow::Result;
use colored::*;

// Re-export types from submodules
pub use types::*;

pub mod checks;
pub mod types;
pub mod utils;

/// Help text for the doctor command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Count of checks by status for summary display
#[derive(Debug)]
pub struct CheckCounts {
    /// Number of checks that passed
    pub ok_count: usize,
    /// Number of checks with warnings
    pub warning_count: usize,
    /// Number of checks with errors
    pub error_count: usize,
}

/// Groups of checks organized by category
struct CheckGroups<'a> {
    pub system_checks: Vec<&'a Check>,
    pub config_checks: Vec<&'a Check>,
    pub prompt_checks: Vec<&'a Check>,
    pub workflow_checks: Vec<&'a Check>,
}

/// Main diagnostic tool for SwissArmyHammer system health checks
///
/// The Doctor struct accumulates diagnostic results and provides a summary
/// of the system's configuration and any potential issues.
pub struct Doctor {
    checks: Vec<Check>,
}

impl Doctor {
    /// Create a new Doctor instance for running diagnostics
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run diagnostic checks with specific options
    ///
    /// # Arguments
    ///
    ///
    /// # Returns
    ///
    /// Returns an exit code:
    /// - 0: All checks passed
    /// - 1: Warnings detected
    /// - 2: Errors detected
    pub fn run_diagnostics_with_options(&mut self) -> Result<i32> {
        println!("{}", "🔨 SwissArmyHammer Doctor".bold().blue());
        println!("{}", "Running diagnostics...".dimmed());
        println!();

        // First, ensure we're in a Git repository
        use swissarmyhammer::directory_utils::find_git_repository_root;

        let git_root = match find_git_repository_root() {
            Some(path) => {
                println!("✅ Git repository detected at: {}", path.display());
                path
            }
            None => {
                println!("❌ SwissArmyHammer requires a Git repository");
                println!();
                println!("Please run this command from within a Git repository.");
                println!("You can create a Git repository with: git init");
                return Ok(ExitCode::Error.into());
            }
        };

        // Check .swissarmyhammer directory
        self.check_swissarmyhammer_directory(&git_root)?;

        // Run all checks
        self.run_system_checks()?;
        self.run_configuration_checks()?;
        self.run_prompt_checks()?;
        self.run_workflow_checks()?;

        // Print results
        self.print_results();

        // Return exit code
        Ok(self.get_exit_code())
    }

    /// Run system checks
    fn run_system_checks(&mut self) -> Result<()> {
        checks::check_installation(&mut self.checks)?;
        checks::check_in_path(&mut self.checks)?;
        checks::check_file_permissions(&mut self.checks)?;
        Ok(())
    }

    /// Run configuration checks
    fn run_configuration_checks(&mut self) -> Result<()> {
        checks::check_claude_config(&mut self.checks)?;
        Ok(())
    }

    /// Run prompt checks
    fn run_prompt_checks(&mut self) -> Result<()> {
        checks::check_prompt_directories(&mut self.checks)?;
        checks::check_yaml_parsing(&mut self.checks)?;
        Ok(())
    }

    /// Run workflow checks
    fn run_workflow_checks(&mut self) -> Result<()> {
        checks::check_workflow_directories(&mut self.checks)?;
        checks::check_workflow_permissions(&mut self.checks)?;
        checks::check_workflow_parsing(&mut self.checks)?;
        checks::check_workflow_run_storage(&mut self.checks)?;
        checks::check_workflow_dependencies(&mut self.checks)?;
        Ok(())
    }

    /// Check SwissArmyHammer directory in Git repository
    fn check_swissarmyhammer_directory(&mut self, git_root: &std::path::Path) -> Result<()> {
        let swissarmyhammer_dir = git_root.join(".swissarmyhammer");

        if !swissarmyhammer_dir.exists() {
            println!("⚠️  .swissarmyhammer directory does not exist (will be created when needed)");
            return Ok(());
        }

        println!(
            "✅ .swissarmyhammer directory found: {}",
            swissarmyhammer_dir.display()
        );

        // Check directory permissions
        match std::fs::metadata(&swissarmyhammer_dir) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    println!("  ✅ Directory is accessible");

                    // Check if directory is writable by trying to create a test file
                    let test_file = swissarmyhammer_dir.join(".doctor_test");
                    match std::fs::write(&test_file, "test") {
                        Ok(_) => {
                            let _ = std::fs::remove_file(&test_file); // Clean up
                            println!("  ✅ Directory is writable");
                        }
                        Err(_) => {
                            println!("  ⚠️  Directory may not be writable");
                        }
                    }
                } else {
                    println!("  ❌ .swissarmyhammer exists but is not a directory");
                }
            }
            Err(e) => {
                println!("  ❌ Cannot access .swissarmyhammer directory: {}", e);
            }
        }

        // Check subdirectories
        let subdirs = ["memos", "todo", "runs", "workflows", "prompts"];
        for subdir in &subdirs {
            let subdir_path = swissarmyhammer_dir.join(subdir);
            if subdir_path.exists() {
                if subdir_path.is_dir() {
                    let file_count = match std::fs::read_dir(&subdir_path) {
                        Ok(entries) => entries.count(),
                        Err(_) => 0,
                    };
                    println!("  ✅ {}/ ({} items)", subdir, file_count);
                } else {
                    println!("  ⚠️  {} exists but is not a directory", subdir);
                }
            } else {
                println!("  ⚠️  {}/ (will be created when needed)", subdir);
            }
        }

        // Check important files
        let semantic_db = swissarmyhammer_dir.join("semantic.db");
        if semantic_db.exists() {
            match std::fs::metadata(&semantic_db) {
                Ok(metadata) => {
                    let size = metadata.len();
                    if size > 0 {
                        println!("  ✅ semantic.db ({} bytes)", size);
                    } else {
                        println!("  ⚠️  semantic.db (empty file)");
                    }
                }
                Err(_) => {
                    println!("  ⚠️  semantic.db (cannot read metadata)");
                }
            }
        } else {
            println!("  ⚠️  semantic.db (will be created when needed)");
        }

        // Check for potential issues
        let abort_file = swissarmyhammer_dir.join(".abort");
        if abort_file.exists() {
            println!("  ⚠️  .abort file exists (previous workflow may have been aborted)");
        }

        println!();
        Ok(())
    }

    /// Print the results
    ///
    /// Displays all diagnostic results grouped by category:
    /// - System checks
    /// - Configuration
    /// - Prompts
    /// - Workflows
    ///
    /// Results are color-coded based on status (OK, Warning, Error).
    pub fn print_results(&self) {
        let use_color = crate::cli::Cli::should_use_color();

        // Group and print checks by category
        let check_groups = self.group_checks_by_category();

        self.print_check_category(&check_groups.system_checks, "System Checks:", use_color);
        self.print_check_category(&check_groups.config_checks, "Configuration:", use_color);
        self.print_check_category(&check_groups.prompt_checks, "Prompts:", use_color);
        self.print_check_category(&check_groups.workflow_checks, "Workflows:", use_color);

        // Print summary
        self.print_summary(use_color);
    }

    /// Group checks by category
    fn group_checks_by_category(&self) -> CheckGroups<'_> {
        let mut system_checks = Vec::new();
        let mut config_checks = Vec::new();
        let mut prompt_checks = Vec::new();
        let mut workflow_checks = Vec::new();

        for check in &self.checks {
            if check.name.contains("Installation")
                || check.name.contains("PATH")
                || check.name.contains("Permission")
                || check.name.contains("Binary")
            {
                system_checks.push(check);
            } else if check.name.contains("Claude")
                || check.name.contains("Config")
                || check.name.contains("MCP")
            {
                config_checks.push(check);
            } else if check.name.contains("Prompt")
                || check.name.contains("YAML")
                || check.name.contains("Template")
            {
                prompt_checks.push(check);
            } else if check.name.contains("Workflow") || check.name.contains("workflow") {
                workflow_checks.push(check);
            } else {
                // Default to system checks
                system_checks.push(check);
            }
        }

        CheckGroups {
            system_checks,
            config_checks,
            prompt_checks,
            workflow_checks,
        }
    }

    /// Print a category of checks
    fn print_check_category(&self, checks: &[&Check], category_name: &str, use_color: bool) {
        if !checks.is_empty() {
            if use_color {
                println!("{}", category_name.bold().yellow());
            } else {
                println!("{category_name}");
            }
            for check in checks {
                print_check(check, use_color);
            }
            println!();
        }
    }

    /// Print the summary of check results
    fn print_summary(&self, use_color: bool) {
        let counts = self.count_check_statuses();

        if use_color {
            println!("{}", "Summary:".bold().green());
        } else {
            println!("Summary:");
        }

        match (counts.error_count, counts.warning_count) {
            (0, 0) => {
                if use_color {
                    println!("  ✨ All checks passed!");
                } else {
                    println!("  All checks passed!");
                }
            }
            (0, _) => {
                if use_color {
                    println!(
                        "  {} checks passed, {} warnings",
                        counts.ok_count.to_string().green(),
                        counts.warning_count.to_string().yellow()
                    );
                } else {
                    println!(
                        "  {} checks passed, {} warnings",
                        counts.ok_count, counts.warning_count
                    );
                }
            }
            _ => {
                if use_color {
                    println!(
                        "  {} checks passed, {} warnings, {} errors",
                        counts.ok_count.to_string().green(),
                        counts.warning_count.to_string().yellow(),
                        counts.error_count.to_string().red()
                    );
                } else {
                    println!(
                        "  {} checks passed, {} warnings, {} errors",
                        counts.ok_count, counts.warning_count, counts.error_count
                    );
                }
            }
        }
    }

    /// Count checks by status
    fn count_check_statuses(&self) -> CheckCounts {
        CheckCounts {
            ok_count: self
                .checks
                .iter()
                .filter(|c| c.status == CheckStatus::Ok)
                .count(),
            warning_count: self
                .checks
                .iter()
                .filter(|c| c.status == CheckStatus::Warning)
                .count(),
            error_count: self
                .checks
                .iter()
                .filter(|c| c.status == CheckStatus::Error)
                .count(),
        }
    }

    /// Get exit code based on check results
    ///
    /// # Returns
    ///
    /// - 0: All checks passed (no errors or warnings)
    /// - 1: At least one warning detected
    /// - 2: At least one error detected
    pub fn get_exit_code(&self) -> i32 {
        let has_error = self.checks.iter().any(|c| c.status == CheckStatus::Error);
        let has_warning = self.checks.iter().any(|c| c.status == CheckStatus::Warning);

        let exit_code = if has_error {
            ExitCode::Error
        } else if has_warning {
            ExitCode::Warning
        } else {
            ExitCode::Success
        };

        exit_code.into()
    }
}

impl Default for Doctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Print a single check result
fn print_check(check: &Check, use_color: bool) {
    let (symbol, color_fn): (&str, fn(&str) -> ColoredString) = match check.status {
        CheckStatus::Ok => ("✓", |s: &str| s.green()),
        CheckStatus::Warning => ("⚠", |s: &str| s.yellow()),
        CheckStatus::Error => ("✗", |s: &str| s.red()),
    };

    if use_color {
        print!(
            "  {} {} - {}",
            color_fn(symbol),
            check.name.bold(),
            check.message
        );
    } else {
        print!("  {} {} - {}", symbol, check.name, check.message);
    }

    if let Some(fix) = &check.fix {
        println!();
        if use_color {
            println!("    {} {}", "→".dimmed(), fix.dimmed());
        } else {
            println!("    → {fix}");
        }
    } else {
        println!();
    }
}

/// Handle the doctor command
pub async fn handle_command() -> i32 {
    let mut doctor = Doctor::new();

    match doctor.run_diagnostics_with_options() {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Doctor command failed: {}", e);
            EXIT_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_creation() {
        let doctor = Doctor::new();
        assert_eq!(doctor.checks.len(), 0);
    }

    #[test]
    fn test_check_status_exit_codes() {
        let mut doctor = Doctor::new();

        // All OK should return 0
        doctor.checks.push(Check {
            name: "Test OK".to_string(),
            status: CheckStatus::Ok,
            message: "Everything is fine".to_string(),
            fix: None,
        });
        assert_eq!(doctor.get_exit_code(), 0);

        // Warning should return 1
        doctor.checks.push(Check {
            name: "Test Warning".to_string(),
            status: CheckStatus::Warning,
            message: "Something might be wrong".to_string(),
            fix: Some("Consider fixing this".to_string()),
        });
        assert_eq!(doctor.get_exit_code(), 1);

        // Error should return 2
        doctor.checks.push(Check {
            name: "Test Error".to_string(),
            status: CheckStatus::Error,
            message: "Something is definitely wrong".to_string(),
            fix: Some("You must fix this".to_string()),
        });
        assert_eq!(doctor.get_exit_code(), 2);
    }

    #[test]
    fn test_run_diagnostics() {
        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_with_options();
        assert!(result.is_ok());

        // Should have at least some checks
        assert!(!doctor.checks.is_empty());

        // Exit code should be 0, 1, or 2
        let exit_code = doctor.get_exit_code();
        assert!(exit_code <= 2);
    }

    #[test]
    fn test_workflow_diagnostics_in_run_diagnostics_with_options() {
        let mut doctor = Doctor::new();
        let result = doctor.run_diagnostics_with_options();
        assert!(result.is_ok());

        // Should have workflow-related checks in the full diagnostics
        let workflow_checks: Vec<_> = doctor
            .checks
            .iter()
            .filter(|c| c.name.contains("Workflow") || c.name.contains("workflow"))
            .collect();
        assert!(
            !workflow_checks.is_empty(),
            "run_diagnostics_with_options should include workflow checks"
        );
    }

    #[test]
    fn test_exit_code_conversion() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::Warning), 1);
        assert_eq!(i32::from(ExitCode::Error), 2);
    }

    #[test]
    fn test_warning_checks_are_categorized() {
        let mut doctor = Doctor::new();

        // Add a warning check that should be categorized
        doctor.checks.push(Check {
            name: "Binary Name".to_string(),
            status: CheckStatus::Warning,
            message: "Test warning message".to_string(),
            fix: Some("Test fix".to_string()),
        });

        let check_groups = doctor.group_checks_by_category();

        // Verify that warning checks are included in appropriate categories
        let all_categorized_checks: Vec<_> = check_groups
            .system_checks
            .iter()
            .chain(check_groups.config_checks.iter())
            .chain(check_groups.prompt_checks.iter())
            .chain(check_groups.workflow_checks.iter())
            .collect();

        // All warning checks should be categorized
        let warning_checks: Vec<_> = doctor
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warning)
            .collect();

        for warning_check in &warning_checks {
            assert!(
                all_categorized_checks
                    .iter()
                    .any(|&&c| std::ptr::eq(c, *warning_check)),
                "Warning check '{}' should be categorized but was not found in any category",
                warning_check.name
            );
        }
    }
}
