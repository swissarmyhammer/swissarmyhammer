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
    /// Why the binary failed to run (even if found on PATH)
    pub error: Option<String>,
    /// Human-readable install instructions
    pub install_hint: Option<String>,
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

/// Check if a command/executable is available and actually works.
///
/// Finding the binary via `which` isn't enough — rustup shims exist on PATH
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

    // Binary exists on PATH — now verify it actually runs
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

/// LSP server info for a project type: (command, install_hint).
fn get_lsp_servers_for_type(project_type: &str) -> Vec<(&'static str, &'static str)> {
    match project_type {
        "rust" => vec![("rust-analyzer", "rustup component add rust-analyzer")],
        "javascript" => vec![
            ("typescript-language-server", "npm install -g typescript-language-server typescript"),
            ("tsserver", "npm install -g typescript"),
        ],
        "python" => vec![
            ("pylsp", "pip install python-lsp-server"),
            ("pyright", "npm install -g pyright"),
        ],
        "go" => vec![("gopls", "go install golang.org/x/tools/gopls@latest")],
        _ => vec![],
    }
}

/// Run a doctor check on the workspace.
pub fn run_doctor(root: &Path) -> DoctorReport {
    let project_type = detect_project_type(root);

    let mut lsp_servers = Vec::new();

    if let Some(ref ptype) = project_type {
        for (lsp_cmd, hint) in get_lsp_servers_for_type(ptype) {
            let (installed, path, error) = is_command_available(lsp_cmd);
            lsp_servers.push(LspAvailability {
                name: lsp_cmd.to_string(),
                installed,
                path,
                error,
                install_hint: if installed { None } else { Some(hint.to_string()) },
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
        let root = Path::new("/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools");
        assert_eq!(detect_project_type(root), Some("rust".to_string()));
    }

    #[test]
    fn test_detect_no_project() {
        let root = Path::new("/tmp");
        assert!(detect_project_type(root).is_none());
    }
}
