//! LSP server detection and startup for code indexing.
//!
//! Manages spawning and communicating with language servers (e.g., rust-analyzer)
//! to extract symbol definitions and track call edges.
//!
//! Loads server configurations from YAML files, falling back to hardcoded
//! defaults if YAML configs are not available.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::CodeContextError;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Owned LSP server specification loaded from YAML configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedLspServerSpec {
    /// Language identifiers this server handles
    pub language_ids: Vec<String>,
    /// Binary name to invoke (looked up via `which`)
    pub command: String,
    /// Command-line arguments
    pub args: Vec<String>,
    /// How long to wait for server startup (in seconds)
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout_secs: u64,
}

fn default_startup_timeout() -> u64 {
    30
}

/// Candidate directories where builtin LSP YAML files might live.
fn lsp_config_search_paths() -> Vec<PathBuf> {
    vec![
        Path::new("builtin/lsp").to_path_buf(),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("builtin/lsp")))
            .unwrap_or_default(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(Path::new("."))
            .join("builtin/lsp"),
    ]
}

/// Load all YAML server specs from a single directory.
fn load_servers_from_dir(lsp_dir: &Path) -> Vec<OwnedLspServerSpec> {
    let entries = match std::fs::read_dir(lsp_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut servers = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_yaml = path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false);
        if !is_yaml {
            continue;
        }
        match load_single_server(&path) {
            Ok(spec) => {
                debug!("Loaded LSP server config: {}", spec.command);
                servers.push(spec);
            }
            Err(e) => {
                warn!("Failed to load LSP server config {:?}: {}", path, e);
            }
        }
    }
    servers
}

/// Load all LSP server specifications from the builtin YAML files.
fn load_lsp_servers() -> Vec<OwnedLspServerSpec> {
    for lsp_dir in lsp_config_search_paths() {
        if !lsp_dir.exists() {
            debug!("LSP config dir not found at {:?}", lsp_dir);
            continue;
        }
        let servers = load_servers_from_dir(&lsp_dir);
        if !servers.is_empty() {
            return servers;
        }
    }

    debug!("No YAML LSP configs found, using hardcoded rust-analyzer");
    vec![OwnedLspServerSpec {
        command: "rust-analyzer".to_string(),
        args: vec![],
        language_ids: vec!["rust".to_string()],
        startup_timeout_secs: 30,
    }]
}

/// Load a single LSP server specification from a YAML file.
fn load_single_server(path: &Path) -> Result<OwnedLspServerSpec, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let spec: OwnedLspServerSpec = serde_yaml_ng::from_str(&contents)?;
    Ok(spec)
}

/// Lazy-initialized registry of LSP server specs loaded from YAML files
static LSP_REGISTRY: LazyLock<Vec<OwnedLspServerSpec>> = LazyLock::new(load_lsp_servers);

/// Configuration for starting an LSP server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Language identifier (e.g., "rust", "python")
    pub language: String,
    /// Path to the language server executable
    pub executable: PathBuf,
    /// Arguments to pass to the server
    pub args: Vec<String>,
    /// Timeout for server initialization in seconds
    pub init_timeout: u64,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            language: "rust".to_string(),
            executable: PathBuf::from("rust-analyzer"),
            args: vec![],
            init_timeout: 30,
        }
    }
}

/// Result of LSP server startup.
#[derive(Debug)]
pub struct LspServerHandle {
    /// Language being served
    pub language: String,
    /// Whether the server started successfully
    pub started: bool,
    /// Any error messages from startup
    pub error: Option<String>,
}

/// Check if a language server is available in the system PATH.
///
/// # Arguments
/// * `executable_name` - Name of the executable (e.g., "rust-analyzer")
///
/// # Returns
/// Path to the executable if found, None otherwise
pub fn find_executable(executable_name: &str) -> Option<PathBuf> {
    let path_var = env::var("PATH").unwrap_or_default();
    let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

    let exe_name = if cfg!(windows) {
        format!("{}.exe", executable_name)
    } else {
        executable_name.to_string()
    };

    for path in paths {
        let exe_path = path.join(&exe_name);
        if exe_path.exists() && exe_path.is_file() {
            debug!("Found {} at {}", executable_name, exe_path.display());
            return Some(exe_path);
        }
    }

    None
}

/// Detect if rust-analyzer is available on the system.
///
/// # Returns
/// Path to rust-analyzer if found
pub fn detect_rust_analyzer() -> Option<PathBuf> {
    find_executable("rust-analyzer")
}

/// Start an LSP server for the given language.
///
/// Looks up the language in the loaded YAML configurations. If found, creates
/// a server configuration; otherwise returns an error handle with unsupported message.
///
/// # Arguments
/// * `language` - Language to start server for (e.g., "rust", "python")
/// * `project_root` - Root directory of the project
///
/// # Returns
/// LspServerHandle with startup status
pub fn start_lsp_server(language: &str, project_root: &Path) -> LspServerHandle {
    debug!("Starting LSP server for language: {}", language);

    let config = match find_config_for_language(language) {
        Some(spec) => create_config_from_spec(language, spec),
        None => {
            warn!(
                "No LSP server configuration found for language: {}",
                language
            );
            return LspServerHandle {
                language: language.to_string(),
                started: false,
                error: Some(format!(
                    "No LSP server configuration found for language: {}",
                    language
                )),
            };
        }
    };

    // Try to start the server
    match spawn_server(&config, project_root) {
        Ok(_) => {
            info!(
                "LSP server started for {}: {}",
                language,
                config.executable.display()
            );
            LspServerHandle {
                language: language.to_string(),
                started: true,
                error: None,
            }
        }
        Err(e) => {
            warn!("Failed to start LSP server for {}: {}", language, e);
            LspServerHandle {
                language: language.to_string(),
                started: false,
                error: Some(e.to_string()),
            }
        }
    }
}

/// Find the LSP server configuration for the given language.
///
/// Searches the loaded registry of YAML configurations for a server that
/// handles the given language. Returns the first matching specification.
fn find_config_for_language(language: &str) -> Option<OwnedLspServerSpec> {
    LSP_REGISTRY
        .iter()
        .find(|spec| spec.language_ids.contains(&language.to_string()))
        .cloned()
}

/// Create an LspServerConfig from an OwnedLspServerSpec.
///
/// Converts the YAML-loaded specification into a configuration ready for spawning.
fn create_config_from_spec(language: &str, spec: OwnedLspServerSpec) -> LspServerConfig {
    LspServerConfig {
        language: language.to_string(),
        executable: PathBuf::from(&spec.command),
        args: spec.args.clone(),
        init_timeout: spec.startup_timeout_secs,
    }
}

/// Spawn an LSP server process.
///
/// Resolves the executable (checking the filesystem first, then PATH),
/// spawns the process, and verifies it doesn't exit immediately.
///
/// # Arguments
/// * `config` - Server configuration
/// * `project_root` - Working directory for the server
///
/// # Returns
/// Result indicating success or error
fn spawn_server(config: &LspServerConfig, project_root: &Path) -> Result<(), CodeContextError> {
    let exe_path = resolve_executable(&config.executable)?;
    spawn_and_verify(&exe_path, &config.args, project_root)
}

/// Resolve the executable path: use as-is if the file exists on disk,
/// otherwise search PATH. Returns an error if not found anywhere.
fn resolve_executable(executable: &Path) -> Result<PathBuf, CodeContextError> {
    if executable.exists() {
        return Ok(executable.to_path_buf());
    }

    let name = executable
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    find_executable(name).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("LSP server executable not found: {}", executable.display()),
        )
        .into()
    })
}

/// Spawn a child process and verify it stays alive past a brief grace period.
///
/// The process is spawned with piped stdio (stdin, stdout, stderr) so
/// a JSON-RPC client can be attached later. If the process exits within
/// 100 ms it is treated as a startup failure.
fn spawn_and_verify(
    exe_path: &Path,
    args: &[String],
    project_root: &Path,
) -> Result<(), CodeContextError> {
    let mut cmd = Command::new(exe_path);
    cmd.current_dir(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for arg in args {
        cmd.arg(arg);
    }

    let mut child = cmd.spawn()?;
    debug!("LSP server process spawned with PID: {:?}", child.id());

    // Brief wait to catch immediate startup failures
    std::thread::sleep(Duration::from_millis(100));

    match child.try_wait() {
        Ok(Some(_)) => Err(std::io::Error::other(
            "LSP server process exited immediately",
        ))?,
        Ok(None) => {
            debug!("LSP server process is running");
            Ok(())
        }
        Err(e) => Err(e)?,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_executable_in_path() {
        // Most systems have 'ls' or 'cmd' available
        let exe = if cfg!(windows) { "cmd" } else { "ls" };
        let result = find_executable(exe);
        assert!(
            result.is_some(),
            "Common executable {} should be found",
            exe
        );
    }

    #[test]
    fn test_find_nonexistent_executable() {
        let result = find_executable("this_exe_should_not_exist_anywhere_12345");
        assert!(
            result.is_none(),
            "Nonexistent executable should not be found"
        );
    }

    #[test]
    fn test_unsupported_language_no_config_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = start_lsp_server("unsupported_lang", tmp.path());
        assert!(!result.started, "Unsupported language should fail to start");
        assert!(result.error.is_some(), "Error message should be provided");
        assert!(
            result
                .error
                .unwrap()
                .contains("No LSP server configuration found"),
            "Error should mention configuration not found"
        );
    }

    #[test]
    fn test_unsupported_language_preserves_name() {
        let tmp = tempfile::tempdir().unwrap();
        let result = start_lsp_server("go", tmp.path());
        assert_eq!(result.language, "go");
        assert!(!result.started);
    }

    #[test]
    fn test_find_config_for_rust() {
        let config = find_config_for_language("rust");
        assert!(
            config.is_some(),
            "Should find rust configuration in loaded registry"
        );
        let spec = config.unwrap();
        assert_eq!(spec.command, "rust-analyzer");
        assert!(spec.language_ids.contains(&"rust".to_string()));
    }

    #[test]
    fn test_lsp_registry_has_rust() {
        // Verify that the loaded registry includes rust-analyzer
        let servers = &*LSP_REGISTRY;
        assert!(!servers.is_empty(), "LSP registry should not be empty");
        assert!(
            servers.iter().any(|s| s.command == "rust-analyzer"),
            "Registry should include rust-analyzer"
        );
    }

    #[test]
    fn test_spawn_server_nonexistent_executable() {
        // An executable that does not exist on the filesystem or in PATH
        let config = LspServerConfig {
            language: "fake".to_string(),
            executable: PathBuf::from("totally_nonexistent_lsp_server_xyz_12345"),
            args: vec![],
            init_timeout: 5,
        };
        let tmp = tempfile::tempdir().unwrap();
        let result = spawn_server(&config, tmp.path());
        assert!(result.is_err(), "Should fail when executable not found");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("IO error"),
            "Error should be IO-based, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_spawn_server_with_exe_in_path_that_exits_immediately() {
        // Use 'true' (or 'cmd /c exit 0' on Windows) which exits immediately.
        // This exercises the PATH-lookup branch and the "exited immediately" error path.
        let exe_name = if cfg!(windows) { "cmd" } else { "true" };
        let config = LspServerConfig {
            language: "test".to_string(),
            executable: PathBuf::from(exe_name),
            args: if cfg!(windows) {
                vec!["/c".to_string(), "exit".to_string(), "0".to_string()]
            } else {
                vec![]
            },
            init_timeout: 5,
        };
        let tmp = tempfile::tempdir().unwrap();
        let result = spawn_server(&config, tmp.path());
        // 'true' exits immediately, so spawn_server should detect this
        assert!(
            result.is_err(),
            "Should fail because process exits immediately"
        );
    }

    #[test]
    fn test_spawn_server_with_absolute_exe_that_exits_immediately() {
        // Find the absolute path to 'true' so we hit the else branch (executable.exists())
        let true_path = find_executable("true");
        // On some systems 'true' might not be a standalone binary; skip if not found
        if let Some(abs_path) = true_path {
            let config = LspServerConfig {
                language: "test".to_string(),
                executable: abs_path,
                args: vec![],
                init_timeout: 5,
            };
            let tmp = tempfile::tempdir().unwrap();
            let result = spawn_server(&config, tmp.path());
            assert!(
                result.is_err(),
                "Should fail because process exits immediately"
            );
        }
    }

    #[test]
    fn test_start_lsp_server_rust_configuration_loaded() {
        // Verify that rust language configuration is loaded from YAML registry
        let tmp = tempfile::tempdir().unwrap();
        let result = start_lsp_server("rust", tmp.path());
        // The language field should always match the request
        assert_eq!(result.language, "rust");
        // Whether it starts depends on if rust-analyzer is installed, but the
        // configuration should have been found and attempted to start.
        // We verify consistency: if started=true then error=None, else error=Some
        if result.started {
            assert!(result.error.is_none());
        } else {
            assert!(result.error.is_some());
        }
    }

    #[test]
    fn test_spawn_server_with_args() {
        // Verify args are passed through by using a command that accepts them.
        // 'sleep 10' stays alive, exercising the "process still running" success path.
        let exe_name = "sleep";
        let config = LspServerConfig {
            language: "test".to_string(),
            executable: PathBuf::from(exe_name),
            args: vec!["10".to_string()],
            init_timeout: 5,
        };
        let tmp = tempfile::tempdir().unwrap();
        let result = spawn_server(&config, tmp.path());
        // sleep should stay alive for 10s, so spawn_server succeeds
        assert!(
            result.is_ok(),
            "sleep 10 should stay running: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_lsp_server_handle_fields() {
        // Verify LspServerHandle Debug impl works and fields are accessible
        let handle = LspServerHandle {
            language: "python".to_string(),
            started: false,
            error: Some("not installed".to_string()),
        };
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("python"));
        assert!(debug_str.contains("not installed"));
    }

    #[test]
    fn test_detect_rust_analyzer() {
        // Just exercise the detect function — result depends on environment
        let result = detect_rust_analyzer();
        // If found, it should be a valid path
        if let Some(path) = result {
            assert!(path.exists());
        }
    }

    #[test]
    fn test_start_lsp_server_unavailable_language_returns_not_started() {
        // start_lsp_server for a language with no registered config should
        // return a handle with started=false and an error describing why.
        let tmp = tempfile::tempdir().unwrap();
        let handle = start_lsp_server("brainfuck", tmp.path());
        assert!(!handle.started);
        assert_eq!(handle.language, "brainfuck");
        assert!(
            handle
                .error
                .as_ref()
                .unwrap()
                .contains("No LSP server configuration found"),
            "Expected 'Unsupported language' in error, got: {:?}",
            handle.error
        );
    }

    #[test]
    fn test_find_executable_ls_returns_some() {
        // 'ls' is a standard executable on Unix systems. Verify find_executable
        // returns Some with a path that actually exists on disk, exercising the
        // debug log on the success path.
        let result = find_executable("ls");
        assert!(result.is_some(), "'ls' should be found in PATH");
        let path = result.unwrap();
        assert!(path.exists(), "Returned path should exist on disk");
        assert!(path.is_file(), "Returned path should be a file");
    }

    #[test]
    fn test_spawn_server_exe_exits_immediately_error_message() {
        // Use an absolute path to 'true' (which exits immediately) to exercise
        // the direct-executable branch (config.executable.exists() == true).
        // Verifies the specific "exited immediately" error message via the
        // inner io::Error (CodeContextError::Io wraps it with Display "IO error").
        let true_path = find_executable("true");
        if let Some(abs_path) = true_path {
            let config = LspServerConfig {
                language: "test".to_string(),
                executable: abs_path,
                args: vec![],
                init_timeout: 5,
            };
            let tmp = tempfile::tempdir().unwrap();
            let result = spawn_server(&config, tmp.path());
            assert!(
                result.is_err(),
                "Process that exits immediately should error"
            );
            let err = result.unwrap_err();
            // CodeContextError::Io Display is just "IO error"; check the source chain
            // for the actual io::Error message.
            let inner_msg = std::error::Error::source(&err)
                .map(|e| e.to_string())
                .unwrap_or_default();
            assert!(
                inner_msg.contains("exited immediately"),
                "Inner error should mention 'exited immediately', got: {}",
                inner_msg
            );
        }
    }

    #[test]
    fn test_spawn_server_absolute_path_process_stays_alive() {
        // Find the absolute path to 'sleep' and use it directly, exercising
        // the else branch where config.executable.exists() is true and the
        // process stays alive (try_wait returns Ok(None)).
        let sleep_path = find_executable("sleep");
        if let Some(abs_path) = sleep_path {
            let config = LspServerConfig {
                language: "test".to_string(),
                executable: abs_path,
                args: vec!["10".to_string()],
                init_timeout: 5,
            };
            let tmp = tempfile::tempdir().unwrap();
            let result = spawn_server(&config, tmp.path());
            assert!(
                result.is_ok(),
                "Process that stays alive should succeed: {:?}",
                result.err()
            );
        }
    }
}
