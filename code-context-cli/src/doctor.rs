//! Code-context Doctor -- Diagnostic checks for code-context setup and configuration.
//!
//! Checks:
//! - Git repository (warning if not found)
//! - code-context binary in PATH
//! - `.code-context/` index directory existence
//! - LSP server availability per detected project type

use std::env;
use std::path::PathBuf;

use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};
use swissarmyhammer_tools::mcp::tools::code_context::doctor as cc_doctor;

/// Code-context diagnostic runner.
///
/// Accumulates [`Check`]s for code-context setup: git repo detection,
/// binary availability, index directory presence, and LSP server status.
pub struct CodeContextDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for CodeContextDoctor {
    /// Returns immutable reference to accumulated checks.
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// Returns mutable reference to accumulated checks.
    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl CodeContextDoctor {
    /// Create a new CodeContextDoctor with no checks.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all code-context diagnostic checks.
    ///
    /// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
    pub fn run_diagnostics(&mut self) -> i32 {
        self.check_git_repository();
        self.check_code_context_in_path();
        self.check_index_directory();
        self.check_lsp_status();

        self.get_exit_code()
    }

    /// Check if we're in a Git repository.
    ///
    /// This is a warning (not error) since code-context can work outside git repos,
    /// but many features depend on repository context.
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

    /// Check if the code-context binary is in PATH.
    fn check_code_context_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) {
            "code-context.exe"
        } else {
            "code-context"
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
                    name: "code-context in PATH".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Found at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "code-context in PATH".to_string(),
                    status: CheckStatus::Warning,
                    message: "code-context not found in PATH".to_string(),
                    fix: Some(
                        "Add code-context to your PATH or install with \
                         `cargo install --path code-context-cli`"
                            .to_string(),
                    ),
                });
            }
        }
    }

    /// Check if the `.code-context/` index directory exists in the current working directory.
    fn check_index_directory(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let index_dir = cwd.join(".code-context");

        if index_dir.is_dir() {
            self.add_check(Check {
                name: "Index Directory".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", index_dir.display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "Index Directory".to_string(),
                status: CheckStatus::Warning,
                message: "No .code-context/ directory found".to_string(),
                fix: Some(
                    "Run `code-context serve` to initialize the index, or start an MCP session"
                        .to_string(),
                ),
            });
        }
    }

    /// Check LSP availability for detected project types.
    ///
    /// Delegates to [`cc_doctor::run_doctor`] to detect project types and probe
    /// each LSP server. Creates one check per LSP server — Ok if installed,
    /// Warning with an install hint if not.
    fn check_lsp_status(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let report = cc_doctor::run_doctor(&cwd);

        for lsp in &report.lsp_servers {
            if lsp.installed {
                self.add_check(Check {
                    name: format!("LSP: {}", lsp.name),
                    status: CheckStatus::Ok,
                    message: match &lsp.path {
                        Some(p) => format!("Installed at {}", p),
                        None => "Installed".to_string(),
                    },
                    fix: None,
                });
            } else {
                let message = match &lsp.error {
                    Some(err) => format!("{} found but failed: {}", lsp.name, err),
                    None => format!("{} not found", lsp.name),
                };
                self.add_check(Check {
                    name: format!("LSP: {}", lsp.name),
                    status: CheckStatus::Warning,
                    message,
                    fix: lsp.install_hint.clone(),
                });
            }
        }
    }
}

impl Default for CodeContextDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the doctor command and display results.
///
/// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
pub fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = CodeContextDoctor::new();
    let exit_code = doctor.run_diagnostics();
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let doctor = CodeContextDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    #[test]
    fn test_run_diagnostics() {
        let mut doctor = CodeContextDoctor::new();
        let exit_code = doctor.run_diagnostics();

        // Should have at least 3 checks: git, path, index directory
        // (LSP checks depend on detected project types, so the minimum is 3)
        assert!(
            doctor.checks().len() >= 3,
            "expected >= 3 checks, got {}",
            doctor.checks().len()
        );

        // Exit code should be 0, 1, or 2
        assert!(exit_code <= 2, "exit code was {}", exit_code);
    }

    #[test]
    fn test_check_git_repository() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_git_repository();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Git Repository");
        // Status depends on whether we're actually in a git repo
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_code_context_in_path() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_code_context_in_path();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "code-context in PATH");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_index_directory() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_index_directory();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Index Directory");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_default() {
        let doctor = CodeContextDoctor::default();
        assert!(doctor.checks().is_empty());
    }
}
