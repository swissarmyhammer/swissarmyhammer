//! Diagnostic checking for code_context tool and LSP availability.
//!
//! The Doctor checks project type, looks up LSP servers from the YAML-driven
//! registry in `swissarmyhammer-lsp`, and verifies their availability before
//! attempting to index.

use std::path::Path;
use std::process::Command;

use swissarmyhammer_project_detection::{detect_projects, ProjectType};

#[derive(Debug, Clone, serde::Serialize)]
pub struct LspAvailability {
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
    /// Why the binary failed to run (even if found on PATH)
    pub error: Option<String>,
    /// Human-readable install instructions
    pub install_hint: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub project_types: Vec<String>,
    pub lsp_servers: Vec<LspAvailability>,
}

/// Detect all project types from filesystem markers using the canonical detection logic.
///
/// Delegates to `swissarmyhammer_project_detection::detect_projects()` with depth 1
/// (root only) and deduplicates the resulting project types.
pub fn detect_project_types(root: &Path) -> Vec<String> {
    detect_project_type_enums(root)
        .into_iter()
        .map(|pt| format!("{:?}", pt).to_lowercase())
        .collect()
}

/// Detect project types as `ProjectType` enum values using the canonical detection logic.
fn detect_project_type_enums(root: &Path) -> Vec<ProjectType> {
    let projects = detect_projects(root, Some(1)).unwrap_or_default();
    let mut types: Vec<ProjectType> = projects.into_iter().map(|p| p.project_type).collect();
    types.dedup();
    types
}

/// Check if a command/executable is available and actually works.
///
/// Finding the binary via `which` isn't enough -- rustup shims exist on PATH
/// but fail if the actual component isn't installed. We verify by running
/// `cmd --version` and checking for a successful exit.
fn is_command_available(cmd: &str) -> (bool, Option<String>, Option<String>) {
    // First, find the binary path
    let path = match Command::new("which").arg(cmd).output() {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        _ => return (false, None, None),
    };

    // Binary exists on PATH -- now verify it actually runs
    match Command::new(cmd).arg("--version").output() {
        Ok(output) if output.status.success() => (true, path, None),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let reason = if stderr.is_empty() {
                format!("exited with status {}", output.status)
            } else {
                stderr
            };
            (false, path, Some(reason))
        }
        Err(e) => (false, path, Some(format!("failed to execute: {}", e))),
    }
}

/// Run a doctor check on the workspace.
///
/// Detects all project types present in `root` and checks LSP availability
/// for each one using the YAML-driven LSP registry. LSP entries are
/// deduplicated by command name so that overlapping server lists
/// (e.g. two project types needing the same LSP) don't produce duplicate entries.
pub fn run_doctor(root: &Path) -> DoctorReport {
    let project_type_enums = detect_project_type_enums(root);
    let project_types: Vec<String> = project_type_enums
        .iter()
        .map(|pt| format!("{:?}", pt).to_lowercase())
        .collect();

    let mut lsp_servers = Vec::new();
    let mut seen_cmds = std::collections::HashSet::new();

    for ptype in &project_type_enums {
        let specs = swissarmyhammer_lsp::servers_for_project(*ptype);
        for spec in &specs {
            if !seen_cmds.insert(spec.command.clone()) {
                continue; // already checked this command
            }
            let (installed, path, error) = is_command_available(&spec.command);
            lsp_servers.push(LspAvailability {
                name: spec.command.clone(),
                installed,
                path,
                error,
                install_hint: if installed {
                    None
                } else {
                    Some(spec.install_hint.clone())
                },
            });
        }
    }

    DoctorReport {
        project_types,
        lsp_servers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        assert_eq!(detect_project_types(tmp.path()), vec!["rust".to_string()]);
    }

    #[test]
    fn test_detect_no_project() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_project_types(tmp.path()).is_empty());
    }

    #[test]
    fn test_detect_mixed_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("package.json"), "{\"name\": \"test\"}\n").unwrap();

        let types = detect_project_types(tmp.path());
        assert_eq!(types, vec!["rust".to_string(), "nodejs".to_string()]);

        // run_doctor should report both types and LSPs for each
        let report = run_doctor(tmp.path());
        assert_eq!(
            report.project_types,
            vec!["rust".to_string(), "nodejs".to_string()]
        );
        let lsp_names: Vec<&str> = report.lsp_servers.iter().map(|l| l.name.as_str()).collect();
        assert!(
            lsp_names.contains(&"rust-analyzer"),
            "missing rust-analyzer"
        );
        assert!(
            lsp_names.contains(&"typescript-language-server"),
            "missing typescript-language-server"
        );
    }
}
