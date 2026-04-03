//! Check implementations for the doctor module
//!
//! Contains CLI-specific checks (installation, PATH, Claude config).
//! Tool-specific health checks live in each tool's Doctorable impl
//! and are collected via `swissarmyhammer_tools::collect_all_health_checks()`.

use super::types::*;
use super::utils::*;
use anyhow::Result;
use std::env;
use std::path::PathBuf;

/// Check names constants to avoid typos and improve maintainability
#[allow(dead_code)]
pub mod check_names {
    /// Human-readable check name for installation method detection
    pub const INSTALLATION_METHOD: &str = "Installation Method";
    /// Human-readable check name for binary executable permissions
    pub const BINARY_PERMISSIONS: &str = "Binary Permissions";
    /// Human-readable check name for binary filename validation
    pub const BINARY_NAME: &str = "Binary Name";
    /// Human-readable check name for sah binary in PATH
    pub const IN_PATH: &str = "swissarmyhammer in PATH";
    /// Human-readable check name for Claude Code MCP configuration
    pub const CLAUDE_CONFIG: &str = "Claude Code MCP configuration";
    /// Human-readable check name for current directory permissions
    pub const FILE_PERMISSIONS: &str = "File permissions";
    /// Human-readable check name for AVP binary in PATH
    pub const AVP_IN_PATH: &str = "AVP in PATH";
    /// Human-readable check name for AVP hooks installation
    pub const AVP_HOOKS_INSTALLED: &str = "AVP Hooks Installed";

    /// Human-readable check name for CLAUDE.md preamble verification
    pub const CLAUDE_MD: &str = "CLAUDE.md Preamble";

    /// Build a dynamic check name for an LSP server
    pub fn lsp_server(command: &str) -> String {
        format!("{command} (LSP)")
    }
}

/// Check installation method and binary integrity
///
/// Verifies:
/// - Installation method (cargo, system, development build)
/// - Binary version and build type
/// - Execute permissions on Unix systems
/// - Binary naming conventions
pub fn check_installation(checks: &mut Vec<Check>) -> Result<()> {
    let current_exe = env::current_exe().unwrap_or_default();
    checks.push(check_installation_method(&current_exe));
    check_binary_permissions(&current_exe, checks);
    checks.push(check_binary_name(&current_exe));
    Ok(())
}

/// Determine the installation method and build info for the current binary.
fn check_installation_method(current_exe: &std::path::Path) -> Check {
    let exe_path = current_exe.to_string_lossy();

    let installation_method = if exe_path.contains(".cargo/bin") {
        "Cargo install"
    } else if exe_path.contains("/usr/local/bin") || exe_path.contains("/usr/bin") {
        "System installation"
    } else if exe_path.contains("target/") && exe_path.contains("debug") {
        "Development build"
    } else if exe_path.contains("target/") && exe_path.contains("release") {
        "Local release build"
    } else {
        "Unknown"
    };

    let version = env!("CARGO_PKG_VERSION");
    let build_info = if cfg!(debug_assertions) {
        "debug build"
    } else {
        "release build"
    };

    Check {
        name: check_names::INSTALLATION_METHOD.to_string(),
        status: CheckStatus::Ok,
        message: format!("{installation_method} (v{version}, {build_info}) at {exe_path}"),
        fix: None,
    }
}

/// Check if the binary has execute permissions (Unix only).
#[cfg(unix)]
fn check_binary_permissions(current_exe: &std::path::Path, checks: &mut Vec<Check>) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(current_exe) {
        let mode = metadata.permissions().mode();
        let exe_path = current_exe.to_string_lossy();

        if mode & 0o111 != 0 {
            checks.push(Check {
                name: check_names::BINARY_PERMISSIONS.to_string(),
                status: CheckStatus::Ok,
                message: format!("Executable permissions: {:o}", mode & 0o777),
                fix: None,
            });
        } else {
            checks.push(Check {
                name: check_names::BINARY_PERMISSIONS.to_string(),
                status: CheckStatus::Error,
                message: "Binary is not executable".to_string(),
                fix: Some(format!("Run: chmod +x {exe_path}")),
            });
        }
    }
}

#[cfg(not(unix))]
fn check_binary_permissions(_current_exe: &std::path::Path, _checks: &mut Vec<Check>) {}

/// Check if this is the expected binary name.
fn check_binary_name(current_exe: &std::path::Path) -> Check {
    let exe_name = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    if exe_name == "sah" || exe_name == "sah.exe" {
        Check {
            name: check_names::BINARY_NAME.to_string(),
            status: CheckStatus::Ok,
            message: format!("Running as {exe_name}"),
            fix: None,
        }
    } else {
        Check {
            name: check_names::BINARY_NAME.to_string(),
            status: CheckStatus::Warning,
            message: format!("Unexpected binary name: {exe_name}"),
            fix: Some("Consider renaming binary to 'swissarmyhammer'".to_string()),
        }
    }
}

/// Check if swissarmyhammer is in PATH
///
/// Searches the system PATH for the swissarmyhammer executable
/// and reports its location if found.
pub fn check_in_path(checks: &mut Vec<Check>) -> Result<()> {
    let path_var = env::var("PATH").unwrap_or_default();
    let paths: Vec<std::path::PathBuf> = env::split_paths(&path_var).collect();

    let exe_name = "sah";
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
        checks.push(Check {
            name: check_names::IN_PATH.to_string(),
            status: CheckStatus::Ok,
            message: format!(
                "Found at: {:?}",
                found_path.expect("found_path should be Some when found is true")
            ),
            fix: None,
        });
    } else {
        checks.push(Check {
            name: check_names::IN_PATH.to_string(),
            status: CheckStatus::Warning,
            message: "sah not found in PATH".to_string(),
            fix: Some(
                "Add sah to your PATH or use the full path in Claude Code config".to_string(),
            ),
        });
    }

    Ok(())
}

/// Check Claude Code MCP configuration
///
/// Verifies that swissarmyhammer is properly configured as an MCP server
/// in Claude Code by running `claude mcp list` and checking the output.
pub fn check_claude_config(checks: &mut Vec<Check>) -> Result<()> {
    let claude_path = find_claude_binary();

    if claude_path.is_none() {
        let path_var = env::var("PATH").unwrap_or_default();
        checks.push(Check {
            name: check_names::CLAUDE_CONFIG.to_string(),
            status: CheckStatus::Error,
            message: "Claude Code command not found in PATH".to_string(),
            fix: Some(format!(
                "Install Claude Code from https://claude.ai/code or ensure the 'claude' command is in your PATH\nCurrent PATH: {}",
                env::split_paths(&path_var).take(3).map(|p| p.display().to_string()).collect::<Vec<_>>().join(if cfg!(windows) { ";" } else { ":" }) + "..."
            )),
        });
        return Ok(());
    }

    checks.push(check_claude_mcp_list(&claude_path));
    Ok(())
}

/// Find the Claude Code binary on PATH.
fn find_claude_binary() -> Option<PathBuf> {
    let executables = if cfg!(windows) {
        &["claude.exe", "claude.cmd", "claude.bat"][..]
    } else {
        &["claude"][..]
    };
    executables.iter().find_map(|name| which::which(name).ok())
}

/// Run `claude mcp list` and return a Check for whether sah is configured.
fn check_claude_mcp_list(claude_path: &Option<PathBuf>) -> Check {
    use std::process::Command;

    let fallback_path = std::path::Path::new("claude");
    let display_path = claude_path.as_deref().unwrap_or(fallback_path);
    let command_path = claude_path.as_deref().unwrap_or(fallback_path);

    let output = match Command::new(command_path).arg("mcp").arg("list").output() {
        Ok(o) => o,
        Err(e) => {
            return Check {
                name: check_names::CLAUDE_CONFIG.to_string(),
                status: CheckStatus::Error,
                message: format!(
                    "Found claude at {:?} but failed to execute it: {}",
                    display_path, e
                ),
                fix: Some(
                    "Check that Claude Code is properly installed and executable".to_string(),
                ),
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Check {
            name: check_names::CLAUDE_CONFIG.to_string(),
            status: CheckStatus::Error,
            message: format!("Failed to run 'claude mcp list': {}", stderr.trim()),
            fix: Some(
                "Ensure Claude Code is installed and the 'claude' command is available".to_string(),
            ),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("sah") {
        Check {
            name: check_names::CLAUDE_CONFIG.to_string(),
            status: CheckStatus::Ok,
            message: format!(
                "swissarmyhammer is configured in Claude Code (found claude at: {:?})",
                display_path
            ),
            fix: None,
        }
    } else {
        Check {
            name: check_names::CLAUDE_CONFIG.to_string(),
            status: CheckStatus::Warning,
            message: "swissarmyhammer not found in Claude Code MCP servers".to_string(),
            fix: Some(get_claude_add_command()),
        }
    }
}

/// Check file permissions
///
/// Verifies that the current directory is readable, which is
/// essential for SwissArmyHammer operations.
pub fn check_file_permissions(checks: &mut Vec<Check>) -> Result<()> {
    match std::env::current_dir() {
        Ok(cwd) => {
            checks.push(Check {
                name: check_names::FILE_PERMISSIONS.to_string(),
                status: CheckStatus::Ok,
                message: format!("Can read current directory: {cwd:?}"),
                fix: None,
            });
        }
        Err(e) => {
            checks.push(Check {
                name: check_names::FILE_PERMISSIONS.to_string(),
                status: CheckStatus::Error,
                message: format!("Failed to read current directory: {e}"),
                fix: Some("Check file permissions for the current directory".to_string()),
            });
        }
    }

    Ok(())
}

/// Check LSP server availability for all detected project types
///
/// Uses project detection to find all project types in the current workspace,
/// then queries the LSP registry for relevant servers. Each server is checked
/// for availability via `which` and `--version`.
pub fn check_lsp_servers(checks: &mut Vec<Check>) -> Result<()> {
    use std::collections::HashSet;
    use swissarmyhammer_lsp::registry::servers_for_project;
    use swissarmyhammer_project_detection::detect_projects;

    let cwd = std::env::current_dir().unwrap_or_default();
    let projects = detect_projects(&cwd, Some(3)).unwrap_or_default();

    let mut seen_commands = HashSet::new();
    let mut specs = Vec::new();
    for project in &projects {
        for spec in servers_for_project(project.project_type) {
            if seen_commands.insert(spec.command.clone()) {
                specs.push(spec);
            }
        }
    }

    if specs.is_empty() {
        checks.push(Check {
            name: "LSP Servers".to_string(),
            status: CheckStatus::Ok,
            message: "No project types detected; no LSP servers to check".to_string(),
            fix: None,
        });
        return Ok(());
    }

    for spec in &specs {
        checks.push(check_single_lsp_server(spec));
    }

    Ok(())
}

/// Check a single LSP server for availability and functionality.
fn check_single_lsp_server(spec: &swissarmyhammer_lsp::types::OwnedLspServerSpec) -> Check {
    let check_name = check_names::lsp_server(&spec.command);

    let path = match which::which(&spec.command) {
        Ok(p) => p,
        Err(_) => {
            return Check {
                name: check_name,
                status: CheckStatus::Warning,
                message: format!("{} not found in PATH", spec.command),
                fix: Some(spec.install_hint.clone()),
            };
        }
    };

    match std::process::Command::new(&path).arg("--version").output() {
        Ok(output) if output.status.success() => Check {
            name: check_name,
            status: CheckStatus::Ok,
            message: format!("Available at {}", path.display()),
            fix: None,
        },
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let reason = if stderr.is_empty() {
                format!("exited with status {}", output.status)
            } else {
                stderr
            };
            Check {
                name: check_name,
                status: CheckStatus::Error,
                message: format!("Found at {} but broken: {}", path.display(), reason),
                fix: Some(spec.install_hint.clone()),
            }
        }
        Err(e) => Check {
            name: check_name,
            status: CheckStatus::Error,
            message: format!("Found at {} but failed to execute: {}", path.display(), e),
            fix: Some(spec.install_hint.clone()),
        },
    }
}

/// Check if AVP (Agent Validator Protocol) binary is in PATH.
///
/// Searches the system PATH for the `avp` executable and reports its location.
/// This is a warning (not error) since AVP is optional.
#[allow(dead_code)]
pub fn check_avp_in_path(checks: &mut Vec<Check>) -> Result<()> {
    let exe_name = if cfg!(windows) { "avp.exe" } else { "avp" };

    match which::which(exe_name) {
        Ok(path) => {
            checks.push(Check {
                name: check_names::AVP_IN_PATH.to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", path.display()),
                fix: None,
            });
        }
        Err(_) => {
            checks.push(Check {
                name: check_names::AVP_IN_PATH.to_string(),
                status: CheckStatus::Warning,
                message: "avp not found in PATH".to_string(),
                fix: Some("Install AVP with: cargo install --path avp-cli".to_string()),
            });
        }
    }

    Ok(())
}

/// Check if AVP hooks are installed in Claude Code settings.
///
/// Checks `.claude/settings.json` (project) and `.claude/settings.local.json` (local)
/// in the current directory for AVP hook entries. Uses `avp_common::install::is_avp_hook()`
/// for canonical hook detection.
#[allow(dead_code)]
pub fn check_avp_hooks(checks: &mut Vec<Check>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let scopes: &[(&str, &str)] = &[
        ("project", ".claude/settings.json"),
        ("local", ".claude/settings.local.json"),
    ];

    let installed_at: Vec<&str> = scopes
        .iter()
        .filter(|(_, path)| has_avp_hooks_in_file(&cwd.join(path)))
        .map(|(name, _)| *name)
        .collect();

    if installed_at.is_empty() {
        checks.push(Check {
            name: check_names::AVP_HOOKS_INSTALLED.to_string(),
            status: CheckStatus::Warning,
            message: "AVP hooks not installed".to_string(),
            fix: Some("Run 'avp init' to install AVP hooks".to_string()),
        });
    } else {
        checks.push(Check {
            name: check_names::AVP_HOOKS_INSTALLED.to_string(),
            status: CheckStatus::Ok,
            message: format!("AVP hooks installed at: {}", installed_at.join(", ")),
            fix: None,
        });
    }

    Ok(())
}

/// Check if a settings file contains any AVP hooks.
#[allow(dead_code)]
fn has_avp_hooks_in_file(path: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let hooks = match settings.get("hooks").and_then(|h| h.as_object()) {
        Some(h) => h,
        None => return false,
    };
    hooks.values().any(|event_hooks| {
        event_hooks
            .as_array()
            .map(|arr| arr.iter().any(avp_common::install::is_avp_hook))
            .unwrap_or(false)
    })
}

/// Check that CLAUDE.md exists and has the required preamble.
///
/// Uses `find_git_repository_root()` to locate the git root, then delegates
/// to `check_claude_md_at()` for the actual file inspection.
pub fn check_claude_md(checks: &mut Vec<Check>) -> Result<()> {
    let root = match swissarmyhammer_common::utils::find_git_repository_root() {
        Some(r) => r,
        None => return Ok(()), // git check earlier already reported this
    };
    check_claude_md_at(&root, checks);
    Ok(())
}

/// Check CLAUDE.md preamble at a specific root path.
///
/// This is the testable core of the check — it takes a root directory
/// instead of relying on `find_git_repository_root()`.
fn check_claude_md_at(root: &std::path::Path, checks: &mut Vec<Check>) {
    use crate::commands::install::components::CLAUDE_MD_PREAMBLE;

    let path = root.join("CLAUDE.md");
    if !path.exists() {
        checks.push(Check {
            name: check_names::CLAUDE_MD.to_string(),
            status: CheckStatus::Warning,
            message: "CLAUDE.md not found at git root".to_string(),
            fix: Some("Run `sah init` to create CLAUDE.md".to_string()),
        });
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            checks.push(Check {
                name: check_names::CLAUDE_MD.to_string(),
                status: CheckStatus::Error,
                message: format!("Failed to read CLAUDE.md: {}", e),
                fix: None,
            });
            return;
        }
    };

    let first_non_empty = content.lines().find(|l| !l.trim().is_empty());
    if first_non_empty.is_some_and(|line| line.contains(CLAUDE_MD_PREAMBLE)) {
        checks.push(Check {
            name: check_names::CLAUDE_MD.to_string(),
            status: CheckStatus::Ok,
            message: "CLAUDE.md has the required preamble".to_string(),
            fix: None,
        });
    } else {
        checks.push(Check {
            name: check_names::CLAUDE_MD.to_string(),
            status: CheckStatus::Warning,
            message: "CLAUDE.md is missing the required preamble".to_string(),
            fix: Some("Run `sah init` to add the required preamble".to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_claude_path_detection() {
        let temp_dir = TempDir::new().unwrap();
        let fake_bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&fake_bin_dir).unwrap();

        let claude_path = fake_bin_dir.join("claude");
        fs::write(&claude_path, "#!/bin/sh\necho fake claude").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&claude_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&claude_path, perms).unwrap();
        }

        let original_path = env::var("PATH").unwrap_or_default();
        let path_separator = if cfg!(windows) { ";" } else { ":" };
        let new_path = format!(
            "{}{}{}",
            fake_bin_dir.display(),
            path_separator,
            original_path
        );
        env::set_var("PATH", &new_path);

        let mut checks = Vec::new();
        let result = check_claude_config(&mut checks);

        env::set_var("PATH", original_path);

        assert!(result.is_ok());
        assert!(!checks.is_empty());

        let claude_check = checks
            .iter()
            .find(|c| c.name == check_names::CLAUDE_CONFIG)
            .unwrap();

        assert!(!claude_check
            .message
            .contains("Claude Code command not found in PATH"));
    }

    #[test]
    fn test_lsp_servers_check() {
        let mut checks = Vec::new();
        let result = check_lsp_servers(&mut checks);

        assert!(result.is_ok());
        assert!(!checks.is_empty());

        for check in &checks {
            assert!(
                check.name.contains("(LSP)") || check.name == "LSP Servers",
                "Unexpected check name: {}",
                check.name
            );
        }

        let ra_check = checks
            .iter()
            .find(|c| c.name == check_names::lsp_server("rust-analyzer"));
        assert!(
            ra_check.is_some(),
            "Should find a rust-analyzer check when running in a Rust project"
        );

        let ra = ra_check.unwrap();
        match ra.status {
            CheckStatus::Ok => {
                assert!(ra.message.contains("Available at"));
            }
            CheckStatus::Error => {
                assert!(ra.message.contains("broken") || ra.message.contains("failed to execute"));
                assert!(ra.fix.is_some());
            }
            CheckStatus::Warning => {
                assert!(ra.message.contains("not found in PATH"));
                assert!(ra.fix.is_some());
            }
        }
    }

    #[test]
    fn test_lsp_servers_check_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        {
            std::env::set_current_dir(temp_dir.path()).unwrap();
            let mut checks = Vec::new();
            let result = check_lsp_servers(&mut checks);

            assert!(result.is_ok());
            assert_eq!(checks.len(), 1);
            assert_eq!(checks[0].name, "LSP Servers");
            assert!(checks[0].message.contains("No project types detected"));
        }

        let _ = std::env::set_current_dir(&original_dir);
    }

    #[test]
    fn test_check_avp_in_path() {
        let mut checks = Vec::new();
        let result = check_avp_in_path(&mut checks);

        assert!(result.is_ok());
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, check_names::AVP_IN_PATH);
        // Status depends on whether avp is actually installed
        assert!(checks[0].status == CheckStatus::Ok || checks[0].status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_avp_hooks_empty() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut checks = Vec::new();
        let result = check_avp_hooks(&mut checks);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, check_names::AVP_HOOKS_INSTALLED);
        assert_eq!(checks[0].status, CheckStatus::Warning);
        assert!(checks[0].message.contains("not installed"));
        assert!(checks[0].fix.as_ref().unwrap().contains("avp init"));
    }

    #[test]
    fn test_check_claude_md_healthy() {
        use crate::commands::install::components::CLAUDE_MD_PREAMBLE;

        let temp_dir = TempDir::new().unwrap();
        let claude_md = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_md, format!("{}\nother stuff\n", CLAUDE_MD_PREAMBLE)).unwrap();

        let mut checks = Vec::new();
        check_claude_md_at(temp_dir.path(), &mut checks);

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, check_names::CLAUDE_MD);
        assert_eq!(checks[0].status, CheckStatus::Ok);
    }

    #[test]
    fn test_check_claude_md_missing() {
        let temp_dir = TempDir::new().unwrap();

        let mut checks = Vec::new();
        check_claude_md_at(temp_dir.path(), &mut checks);

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, check_names::CLAUDE_MD);
        assert_eq!(checks[0].status, CheckStatus::Warning);
        assert!(checks[0].fix.as_ref().unwrap().contains("sah init"));
    }

    #[test]
    fn test_check_claude_md_no_preamble() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_md, "some other content\n").unwrap();

        let mut checks = Vec::new();
        check_claude_md_at(temp_dir.path(), &mut checks);

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, check_names::CLAUDE_MD);
        assert_eq!(checks[0].status, CheckStatus::Warning);
        assert!(checks[0].message.contains("missing"));
        assert!(checks[0].fix.as_ref().unwrap().contains("sah init"));
    }
}
