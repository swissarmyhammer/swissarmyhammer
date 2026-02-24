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
pub mod check_names {
    pub const INSTALLATION_METHOD: &str = "Installation Method";
    pub const BINARY_PERMISSIONS: &str = "Binary Permissions";
    pub const BINARY_NAME: &str = "Binary Name";
    pub const IN_PATH: &str = "swissarmyhammer in PATH";
    pub const CLAUDE_CONFIG: &str = "Claude Code MCP configuration";
    pub const FILE_PERMISSIONS: &str = "File permissions";
}

/// Check installation method and binary integrity
///
/// Verifies:
/// - Installation method (cargo, system, development build)
/// - Binary version and build type
/// - Execute permissions on Unix systems
/// - Binary naming conventions
pub fn check_installation(checks: &mut Vec<Check>) -> Result<()> {
    // Check if running from cargo install vs standalone binary
    let current_exe = env::current_exe().unwrap_or_default();
    let exe_path = current_exe.to_string_lossy();

    // Determine installation method
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

    // Check binary version and build info
    let version = env!("CARGO_PKG_VERSION");
    let build_info = if cfg!(debug_assertions) {
        "debug build"
    } else {
        "release build"
    };

    checks.push(Check {
        name: check_names::INSTALLATION_METHOD.to_string(),
        status: CheckStatus::Ok,
        message: format!("{installation_method} (v{version}, {build_info}) at {exe_path}"),
        fix: None,
    });

    // Check if binary has execute permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&current_exe) {
            let permissions = metadata.permissions();
            let mode = permissions.mode();

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

    // Check if this is the expected binary name
    let exe_name = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    if exe_name == "sah" || exe_name == "sah.exe" {
        checks.push(Check {
            name: check_names::BINARY_NAME.to_string(),
            status: CheckStatus::Ok,
            message: format!("Running as {exe_name}"),
            fix: None,
        });
    } else {
        checks.push(Check {
            name: check_names::BINARY_NAME.to_string(),
            status: CheckStatus::Warning,
            message: format!("Unexpected binary name: {exe_name}"),
            fix: Some("Consider renaming binary to 'swissarmyhammer'".to_string()),
        });
    }

    Ok(())
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
    use std::process::Command;

    // First, manually check if claude is in PATH
    let path_var = env::var("PATH").unwrap_or_default();
    let paths: Vec<std::path::PathBuf> = env::split_paths(&path_var).collect();

    let mut claude_found_in_path = false;
    let mut claude_path = None;

    // Check for both 'claude' and 'claude.exe' (for Windows)
    let executables = if cfg!(windows) {
        vec!["claude.exe", "claude.cmd", "claude.bat"]
    } else {
        vec!["claude"]
    };

    for path in paths {
        for exe_name in &executables {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                claude_found_in_path = true;
                claude_path = Some(exe_path);
                break;
            }
        }
        if claude_found_in_path {
            break;
        }
    }

    // If claude is not found in PATH, provide detailed error
    if !claude_found_in_path {
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

    // Run `claude mcp list` to check if swissarmyhammer is configured
    match Command::new("claude").arg("mcp").arg("list").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);

                // Check if swissarmyhammer is in the list
                if stdout.contains("sah") {
                    checks.push(Check {
                        name: check_names::CLAUDE_CONFIG.to_string(),
                        status: CheckStatus::Ok,
                        message: format!(
                            "swissarmyhammer is configured in Claude Code (found claude at: {:?})",
                            claude_path.unwrap_or_else(|| PathBuf::from("claude"))
                        ),
                        fix: None,
                    });
                } else {
                    checks.push(Check {
                        name: check_names::CLAUDE_CONFIG.to_string(),
                        status: CheckStatus::Warning,
                        message: "swissarmyhammer not found in Claude Code MCP servers".to_string(),
                        fix: Some(get_claude_add_command()),
                    });
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                checks.push(Check {
                    name: check_names::CLAUDE_CONFIG.to_string(),
                    status: CheckStatus::Error,
                    message: format!("Failed to run 'claude mcp list': {}", stderr.trim()),
                    fix: Some(
                        "Ensure Claude Code is installed and the 'claude' command is available"
                            .to_string(),
                    ),
                });
            }
        }
        Err(e) => {
            // We already checked PATH above, so this error is something else
            checks.push(Check {
                name: check_names::CLAUDE_CONFIG.to_string(),
                status: CheckStatus::Error,
                message: format!(
                    "Found claude at {:?} but failed to execute it: {}",
                    claude_path.unwrap_or_else(|| PathBuf::from("claude")),
                    e
                ),
                fix: Some(
                    "Check that Claude Code is properly installed and executable".to_string(),
                ),
            });
        }
    }

    Ok(())
}

/// Check file permissions
///
/// Verifies that the current directory is readable, which is
/// essential for SwissArmyHammer operations.
pub fn check_file_permissions(checks: &mut Vec<Check>) -> Result<()> {
    // For now, just check that we can read the current directory
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_claude_path_detection() {
        // Create a temporary directory to simulate PATH entries
        let temp_dir = TempDir::new().unwrap();
        let fake_bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&fake_bin_dir).unwrap();

        // Create a fake claude executable
        let claude_path = fake_bin_dir.join("claude");
        fs::write(&claude_path, "#!/bin/sh\necho fake claude").unwrap();

        // Make it executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&claude_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&claude_path, perms).unwrap();
        }

        // Set up PATH environment variable
        let original_path = env::var("PATH").unwrap_or_default();
        let path_separator = if cfg!(windows) { ";" } else { ":" };
        let new_path = format!(
            "{}{}{}",
            fake_bin_dir.display(),
            path_separator,
            original_path
        );
        env::set_var("PATH", &new_path);

        // Run the check
        let mut checks = Vec::new();
        let result = check_claude_config(&mut checks);

        // Restore original PATH
        env::set_var("PATH", original_path);

        // Verify the result
        assert!(result.is_ok());
        assert!(!checks.is_empty());

        // The check should find claude in PATH
        let claude_check = checks
            .iter()
            .find(|c| c.name == check_names::CLAUDE_CONFIG)
            .unwrap();

        // It might fail to execute or find swissarmyhammer, but it should not say "command not found in PATH"
        assert!(!claude_check
            .message
            .contains("Claude Code command not found in PATH"));
    }
}
