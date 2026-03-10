//! Diagnostic checking for code_context tool and LSP availability.
//!
//! The Doctor checks project type, detects appropriate LSP servers,
//! and verifies their availability before attempting to index.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LspAvailability {
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub project_type: Option<String>,
    pub lsp_servers: Vec<LspAvailability>,
}

/// Detect project type from filesystem markers.
pub fn detect_project_type(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").exists() {
        return Some("rust".to_string());
    }
    if root.join("package.json").exists() {
        return Some("javascript".to_string());
    }
    if root.join("pyproject.toml").exists() || root.join("setup.py").exists() {
        return Some("python".to_string());
    }
    if root.join("go.mod").exists() {
        return Some("go".to_string());
    }
    None
}

/// Check if a command/executable is available in PATH.
fn is_command_available(cmd: &str) -> (bool, Option<String>) {
    match Command::new("which").arg(cmd).output() {
        Ok(output) => {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                (true, Some(path))
            } else {
                (false, None)
            }
        }
        Err(_) => (false, None),
    }
}

/// Get LSP servers appropriate for a project type.
fn get_lsp_servers_for_type(project_type: &str) -> Vec<&'static str> {
    match project_type {
        "rust" => vec!["rust-analyzer"],
        "javascript" => vec!["node_modules/.bin/typescript-language-server", "tsserver"],
        "python" => vec!["pylsp", "pyright"],
        "go" => vec!["gopls"],
        _ => vec![],
    }
}

/// Run a doctor check on the workspace.
pub fn run_doctor(root: &Path) -> DoctorReport {
    let project_type = detect_project_type(root);

    let mut lsp_servers = Vec::new();

    if let Some(ref ptype) = project_type {
        for lsp_cmd in get_lsp_servers_for_type(ptype) {
            let (installed, path) = is_command_available(lsp_cmd);
            lsp_servers.push(LspAvailability {
                name: lsp_cmd.to_string(),
                installed,
                path,
            });
        }
    }

    DoctorReport {
        project_type,
        lsp_servers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        assert_eq!(detect_project_type(tmp.path()), Some("rust".to_string()));
    }

    #[test]
    fn test_detect_no_project() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_project_type(tmp.path()).is_none());
    }
}
