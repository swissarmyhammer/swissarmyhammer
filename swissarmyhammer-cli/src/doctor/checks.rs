//! Check implementations for the doctor module

use super::types::*;
use super::utils::*;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Minimum disk space in MB before warning
///
/// This threshold is set to 100MB which provides enough space for:
/// - Several workflow run outputs (typically 1-10MB each)
/// - Temporary files created during workflow execution
/// - Log files and diagnostic information
///
/// This conservative threshold helps ensure smooth operation while avoiding
/// false alarms on systems with limited but adequate disk space.
pub const LOW_DISK_SPACE_MB: u64 = 100;

/// Check names constants to avoid typos and improve maintainability
pub mod check_names {
    pub const INSTALLATION_METHOD: &str = "Installation Method";
    pub const BINARY_PERMISSIONS: &str = "Binary Permissions";
    pub const BINARY_NAME: &str = "Binary Name";
    pub const IN_PATH: &str = "swissarmyhammer in PATH";
    pub const CLAUDE_CONFIG: &str = "Claude Code MCP configuration";
    pub const BUILTIN_PROMPTS: &str = "Built-in prompts";
    pub const USER_PROMPTS_DIR: &str = "User prompts directory";
    pub const LOCAL_PROMPTS_DIR: &str = "Local prompts directory";
    pub const YAML_PARSING: &str = "YAML parsing";
    pub const FILE_PERMISSIONS: &str = "File permissions";
    pub const WORKFLOW_PARSING: &str = "Workflow parsing";
    pub const WORKFLOW_RUN_STORAGE_ACCESS: &str = "Workflow run storage accessibility";
    pub const WORKFLOW_RUN_STORAGE_SPACE: &str = "Workflow run storage space";
    pub const WORKFLOW_NAME_CONFLICTS: &str = "Workflow name conflicts";
    pub const WORKFLOW_CIRCULAR_DEPS: &str = "Workflow circular dependencies";
}

/// Format strings used throughout the module
pub mod format_strings {
    #[cfg(not(unix))]
    pub const WORKFLOW_DIR_ACCESS: &str = "Workflow directory access: {:?}";
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

    if exe_name == "swissarmyhammer" || exe_name == "swissarmyhammer.exe" {
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

    let exe_name = "swissarmyhammer";
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
            message: "swissarmyhammer not found in PATH".to_string(),
            fix: Some(
                "Add swissarmyhammer to your PATH or use the full path in Claude Code config"
                    .to_string(),
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

/// Check prompt directories
///
/// Verifies the existence and accessibility of:
/// - Built-in prompts (embedded in binary)
/// - User prompts directory (~/.swissarmyhammer/prompts)
/// - Local prompts directory (./.swissarmyhammer/prompts)
pub fn check_prompt_directories(checks: &mut Vec<Check>) -> Result<()> {
    // Check builtin prompts (embedded in binary)
    checks.push(Check {
        name: check_names::BUILTIN_PROMPTS.to_string(),
        status: CheckStatus::Ok,
        message: "Built-in prompts are embedded in the binary".to_string(),
        fix: None,
    });

    // Check user prompts directory
    if let Some(home) = dirs::home_dir() {
        let user_prompts = home.join(SWISSARMYHAMMER_DIR).join("prompts");
        if user_prompts.exists() {
            let count = count_markdown_files(&user_prompts);
            checks.push(Check {
                name: check_names::USER_PROMPTS_DIR.to_string(),
                status: CheckStatus::Ok,
                message: format!("Found {count} prompts in {user_prompts:?}"),
                fix: None,
            });
        } else {
            checks.push(Check {
                name: check_names::USER_PROMPTS_DIR.to_string(),
                status: CheckStatus::Ok,
                message: format!(
                    "{} directory not found (optional): {:?}",
                    "User prompts", user_prompts
                ),
                fix: Some(format!("Create directory: mkdir -p {user_prompts:?}")),
            });
        }
    }

    // Check local prompts directory
    let local_prompts = PathBuf::from(SWISSARMYHAMMER_DIR).join("prompts");
    if local_prompts.exists() {
        let count = count_markdown_files(&local_prompts);
        checks.push(Check {
            name: check_names::LOCAL_PROMPTS_DIR.to_string(),
            status: CheckStatus::Ok,
            message: format!("Found {count} prompts in {local_prompts:?}"),
            fix: None,
        });
    } else {
        checks.push(Check {
            name: check_names::LOCAL_PROMPTS_DIR.to_string(),
            status: CheckStatus::Ok,
            message: format!(
                "{} directory not found (optional): {:?}",
                "Local prompts", local_prompts
            ),
            fix: Some(format!("Create directory: mkdir -p {local_prompts:?}")),
        });
    }

    Ok(())
}

/// Check for YAML parsing errors
///
/// Scans all markdown files in prompt directories and validates
/// their YAML front matter for syntax errors.
pub fn check_yaml_parsing(checks: &mut Vec<Check>) -> Result<()> {
    let mut yaml_errors = Vec::new();

    // Check all prompt directories
    let mut dirs_to_check = vec![PathBuf::from(SWISSARMYHAMMER_DIR).join("prompts")];

    // Add user directory if it exists
    if let Some(home) = dirs::home_dir() {
        dirs_to_check.push(home.join(SWISSARMYHAMMER_DIR).join("prompts"));
    }

    for dir in dirs_to_check {
        if !dir.exists() {
            continue;
        }

        for entry in WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        {
            match fs::read_to_string(entry.path()) {
                Ok(content) => {
                    // Try to parse YAML front matter
                    if content.starts_with("---") {
                        let parts: Vec<&str> = content.splitn(3, "---").collect();
                        if parts.len() >= 3 {
                            let yaml_content = parts[1];
                            if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(yaml_content)
                            {
                                yaml_errors.push((entry.path().to_path_buf(), e.to_string()));
                            }
                        }
                    }
                }
                Err(e) => {
                    yaml_errors.push((
                        entry.path().to_path_buf(),
                        format!("Failed to read file: {e}"),
                    ));
                }
            }
        }
    }

    if yaml_errors.is_empty() {
        checks.push(Check {
            name: check_names::YAML_PARSING.to_string(),
            status: CheckStatus::Ok,
            message: "All prompt YAML front matter is valid".to_string(),
            fix: None,
        });
    } else {
        for (path, error) in yaml_errors {
            checks.push(Check {
                name: format!("YAML parsing: {:?}", path.file_name().unwrap_or_default()),
                status: CheckStatus::Error,
                message: error,
                fix: Some(format!("Fix the YAML syntax in {path:?}")),
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

/// Check workflow directories exist
///
/// Verifies the existence of workflow directories:
/// - User workflows (~/.swissarmyhammer/workflows)
/// - Local workflows (./.swissarmyhammer/workflows)
/// - Run storage directory (~/.swissarmyhammer/runs)
pub fn check_workflow_directories(checks: &mut Vec<Check>) -> Result<()> {
    // Check workflow directories
    for dir_info in get_workflow_directories() {
        if dir_info.path.path().exists() {
            let count = count_files_with_extension(dir_info.path.path(), "mermaid");
            checks.push(Check {
                name: format!("{} workflows directory", dir_info.category),
                status: CheckStatus::Ok,
                message: format!("Found {} workflows in {}", count, dir_info.path),
                fix: None,
            });
        } else {
            checks.push(Check {
                name: format!("{} workflows directory", dir_info.category),
                status: CheckStatus::Ok,
                message: format!(
                    "{} workflows directory not found (optional): {}",
                    dir_info.category, dir_info.path
                ),
                fix: Some(format!("Create directory: mkdir -p {}", dir_info.path)),
            });
        }
    }

    // Check workflow run storage directory
    if let Some(home) = dirs::home_dir() {
        let run_storage = home.join(SWISSARMYHAMMER_DIR).join("runs");
        if run_storage.exists() {
            checks.push(Check {
                name: "Workflow run storage directory".to_string(),
                status: CheckStatus::Ok,
                message: format!("Run storage directory exists: {run_storage:?}"),
                fix: None,
            });
        } else {
            checks.push(Check {
                name: "Workflow run storage directory".to_string(),
                status: CheckStatus::Warning,
                message: format!("Run storage directory not found: {run_storage:?}"),
                fix: Some(format!("Create directory: mkdir -p {run_storage:?}")),
            });
        }
    }

    Ok(())
}

/// Check workflow file permissions
///
/// Ensures all workflow directories have appropriate read/write
/// permissions for the current user. On Unix systems, checks for
/// 700 (rwx------) permissions.
pub fn check_workflow_permissions(checks: &mut Vec<Check>) -> Result<()> {
    let mut dirs_to_check = Vec::new();

    // Add workflow directories
    for dir_info in get_workflow_directories() {
        if dir_info.path.path().exists() {
            dirs_to_check.push(dir_info.path.path().to_path_buf());
        }
    }

    // Add run storage directory if it exists
    if let Some(home) = dirs::home_dir() {
        let run_storage = home.join(SWISSARMYHAMMER_DIR).join("runs");
        if run_storage.exists() {
            dirs_to_check.push(run_storage);
        }
    }

    // Check permissions on each directory
    for dir in dirs_to_check {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&dir) {
                let permissions = metadata.permissions();
                let mode = permissions.mode();

                // Check if directory is readable and writable
                if (mode & 0o700) == 0o700 {
                    checks.push(Check {
                        name: format!(
                            "Workflow directory permissions: {:?}",
                            dir.file_name().unwrap_or_default()
                        ),
                        status: CheckStatus::Ok,
                        message: format!("Directory has correct permissions: {:o}", mode & 0o777),
                        fix: None,
                    });
                } else {
                    checks.push(Check {
                        name: format!(
                            "Workflow directory permissions: {:?}",
                            dir.file_name().unwrap_or_default()
                        ),
                        status: CheckStatus::Warning,
                        message: format!(
                            "Directory permissions may be insufficient: {:o}",
                            mode & 0o777
                        ),
                        fix: Some(format!("Run: chmod 755 {dir:?}")),
                    });
                }
            } else {
                checks.push(Check {
                    name: format!(
                        "Workflow directory permissions: {:?}",
                        dir.file_name().unwrap_or_default()
                    ),
                    status: CheckStatus::Warning,
                    message: "Failed to check directory permissions".to_string(),
                    fix: None,
                });
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, just check if directory is accessible
            if std::fs::read_dir(&dir).is_ok() {
                checks.push(Check {
                    name: format!(
                        format_strings::WORKFLOW_DIR_ACCESS,
                        dir.file_name().unwrap_or_default()
                    ),
                    status: CheckStatus::Ok,
                    message: "Directory is accessible".to_string(),
                    fix: None,
                });
            } else {
                checks.push(Check {
                    name: format!(
                        format_strings::WORKFLOW_DIR_ACCESS,
                        dir.file_name().unwrap_or_default()
                    ),
                    status: CheckStatus::Error,
                    message: "Failed to access directory".to_string(),
                    fix: Some("Check directory permissions and ownership".to_string()),
                });
            }
        }
    }

    Ok(())
}

/// Check workflow parsing
///
/// Scans all .mermaid files in workflow directories and verifies
/// they are readable and not empty.
pub fn check_workflow_parsing(checks: &mut Vec<Check>) -> Result<()> {
    let mut workflow_errors = Vec::new();

    for dir_info in get_workflow_directories() {
        if !dir_info.path.path().exists() {
            continue;
        }

        for entry in WalkDir::new(dir_info.path.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mermaid"))
        {
            // Validate path before reading
            if let Err(e) = validate_path_no_traversal(entry.path()) {
                workflow_errors.push((entry.path().to_path_buf(), format!("Invalid path: {e}")));
                continue;
            }

            match fs::read_to_string(entry.path()) {
                Ok(content) => {
                    // Check if file is readable and not empty
                    if content.trim().is_empty() {
                        workflow_errors.push((
                            entry.path().to_path_buf(),
                            "Workflow file is empty".to_string(),
                        ));
                    }
                }
                Err(e) => {
                    workflow_errors.push((
                        entry.path().to_path_buf(),
                        format!("Failed to read workflow file: {e}"),
                    ));
                }
            }
        }
    }

    if workflow_errors.is_empty() {
        checks.push(Check {
            name: check_names::WORKFLOW_PARSING.to_string(),
            status: CheckStatus::Ok,
            message: "All workflow files are readable".to_string(),
            fix: None,
        });
    } else {
        for (path, error) in workflow_errors {
            checks.push(Check {
                name: format!(
                    "Workflow parsing: {:?}",
                    path.file_name().unwrap_or_default()
                ),
                status: CheckStatus::Error,
                message: error,
                fix: Some(format!("Fix or remove the workflow file: {path:?}")),
            });
        }
    }

    Ok(())
}

/// Check workflow run storage
///
/// Verifies the workflow run storage directory:
/// - Exists and is accessible
/// - Has write permissions
/// - Has adequate disk space
pub fn check_workflow_run_storage(checks: &mut Vec<Check>) -> Result<()> {
    if let Some(home) = dirs::home_dir() {
        let run_storage = home.join(SWISSARMYHAMMER_DIR).join("runs");

        if run_storage.exists() {
            check_run_storage_write_access(checks, &run_storage)?;
            check_run_storage_disk_space(checks, &run_storage)?;
        } else {
            checks.push(Check {
                name: check_names::WORKFLOW_RUN_STORAGE_ACCESS.to_string(),
                status: CheckStatus::Warning,
                message: "Run storage directory does not exist".to_string(),
                fix: Some(format!("Create directory: mkdir -p {run_storage:?}")),
            });
        }
    }

    Ok(())
}

/// Check if workflow run storage is writable
fn check_run_storage_write_access(checks: &mut Vec<Check>, run_storage: &Path) -> Result<()> {
    let test_file = run_storage.join(".doctor_test");
    match fs::write(&test_file, "test") {
        Ok(_) => {
            // Clean up test file - ignore errors as the file may have already been removed
            // or we may lack permissions (which was the point of the test)
            let _ = fs::remove_file(&test_file);

            checks.push(Check {
                name: check_names::WORKFLOW_RUN_STORAGE_ACCESS.to_string(),
                status: CheckStatus::Ok,
                message: "Run storage is accessible and writable".to_string(),
                fix: None,
            });
        }
        Err(e) => {
            checks.push(Check {
                name: check_names::WORKFLOW_RUN_STORAGE_ACCESS.to_string(),
                status: CheckStatus::Error,
                message: format!("Run storage is not writable: {e}"),
                fix: Some(format!("Check permissions on {run_storage:?}")),
            });
        }
    }

    Ok(())
}

/// Check available disk space for workflow run storage
fn check_run_storage_disk_space(checks: &mut Vec<Check>, run_storage: &Path) -> Result<()> {
    match check_disk_space(run_storage) {
        Ok((available, _)) => {
            if available.is_low(LOW_DISK_SPACE_MB) {
                checks.push(Check {
                    name: check_names::WORKFLOW_RUN_STORAGE_SPACE.to_string(),
                    status: CheckStatus::Warning,
                    message: format!("Low disk space: {available}"),
                    fix: Some(
                        "Consider cleaning up old workflow runs or freeing disk space".to_string(),
                    ),
                });
            } else {
                checks.push(Check {
                    name: check_names::WORKFLOW_RUN_STORAGE_SPACE.to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Adequate disk space: {available}"),
                    fix: None,
                });
            }
        }
        Err(e) => {
            checks.push(Check {
                name: check_names::WORKFLOW_RUN_STORAGE_SPACE.to_string(),
                status: CheckStatus::Warning,
                message: format!("Failed to check disk space: {e}"),
                fix: None,
            });
        }
    }

    Ok(())
}

/// Check for workflow circular dependencies and conflicts
///
/// Detects potential issues in the workflow system:
/// - Name conflicts (same workflow name in multiple locations)
/// - Circular dependencies (requires runtime analysis)
pub fn check_workflow_dependencies(checks: &mut Vec<Check>) -> Result<()> {
    let workflow_names = collect_workflow_names()?;
    check_name_conflicts(checks, &workflow_names);
    check_circular_dependencies(checks);
    Ok(())
}

/// Collect all workflow names and their locations
fn collect_workflow_names() -> Result<std::collections::HashMap<String, Vec<PathBuf>>> {
    use std::collections::HashMap;

    let mut workflow_names = HashMap::new();

    for dir_info in get_workflow_directories() {
        if !dir_info.path.path().exists() {
            continue;
        }

        for entry in WalkDir::new(dir_info.path.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mermaid"))
        {
            if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                workflow_names
                    .entry(stem.to_string())
                    .or_insert_with(Vec::new)
                    .push(entry.path().to_path_buf());
            }
        }
    }

    Ok(workflow_names)
}

/// Check for workflow name conflicts
fn check_name_conflicts(
    checks: &mut Vec<Check>,
    workflow_names: &std::collections::HashMap<String, Vec<PathBuf>>,
) {
    let mut has_conflicts = false;

    for (name, paths) in workflow_names.iter() {
        if paths.len() > 1 {
            has_conflicts = true;
            let locations = paths
                .iter()
                .map(|p| format!("{p:?}"))
                .collect::<Vec<_>>()
                .join(", ");

            checks.push(Check {
                name: format!("Workflow name conflict: {name}"),
                status: CheckStatus::Warning,
                message: format!("Workflow '{name}' exists in multiple locations: {locations}"),
                fix: Some("Rename or remove duplicate workflows to avoid conflicts".to_string()),
            });
        }
    }

    if !has_conflicts {
        checks.push(Check {
            name: check_names::WORKFLOW_NAME_CONFLICTS.to_string(),
            status: CheckStatus::Ok,
            message: "No workflow name conflicts detected".to_string(),
            fix: None,
        });
    }
}

/// Check for circular dependencies
fn check_circular_dependencies(checks: &mut Vec<Check>) {
    // Note: Actual circular dependency checking would require parsing the workflow files
    // and analyzing their transition dependencies, which is beyond the scope of a simple check
    checks.push(Check {
        name: check_names::WORKFLOW_CIRCULAR_DEPS.to_string(),
        status: CheckStatus::Ok,
        message: "Circular dependency checking requires workflow execution".to_string(),
        fix: None,
    });
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

    #[test]
    fn test_claude_not_in_path() {
        // Save original PATH
        let original_path = env::var("PATH").unwrap_or_default();

        // Set PATH to a directory that definitely doesn't contain claude
        let temp_dir = TempDir::new().unwrap();
        env::set_var("PATH", temp_dir.path().display().to_string());

        // Run the check
        let mut checks = Vec::new();
        let result = check_claude_config(&mut checks);

        // Restore original PATH
        env::set_var("PATH", original_path);

        // Verify the result
        assert!(result.is_ok());
        assert!(!checks.is_empty());

        // The check should report claude not found in PATH
        let claude_check = checks
            .iter()
            .find(|c| c.name == check_names::CLAUDE_CONFIG)
            .unwrap();
        assert_eq!(claude_check.status, CheckStatus::Error);
        assert!(claude_check
            .message
            .contains("Claude Code command not found in PATH"));
    }
}
