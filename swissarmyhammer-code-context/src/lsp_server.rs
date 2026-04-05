//! LSP server detection and startup for code indexing.
//!
//! Manages spawning and communicating with language servers (e.g., rust-analyzer)
//! to extract symbol definitions and track call edges.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::CodeContextError;

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
/// # Arguments
/// * `language` - Language to start server for (e.g., "rust")
/// * `project_root` - Root directory of the project
///
/// # Returns
/// LspServerHandle with startup status
pub fn start_lsp_server(language: &str, project_root: &Path) -> LspServerHandle {
    debug!("Starting LSP server for language: {}", language);

    let config = match language {
        "rust" => create_rust_analyzer_config(),
        _ => {
            warn!("Unsupported language: {}", language);
            return LspServerHandle {
                language: language.to_string(),
                started: false,
                error: Some(format!("Unsupported language: {}", language)),
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

/// Create configuration for rust-analyzer.
fn create_rust_analyzer_config() -> LspServerConfig {
    LspServerConfig {
        language: "rust".to_string(),
        executable: PathBuf::from("rust-analyzer"),
        args: vec![],
        init_timeout: 30,
    }
}

/// Spawn an LSP server process.
///
/// # Arguments
/// * `config` - Server configuration
/// * `project_root` - Working directory for the server
///
/// # Returns
/// Result indicating success or error
fn spawn_server(config: &LspServerConfig, project_root: &Path) -> Result<(), CodeContextError> {
    use std::io;

    // Check if executable exists
    if !config.executable.exists() {
        // Try to find it in PATH
        if let Some(exe_path) =
            find_executable(config.executable.file_name().unwrap().to_str().unwrap())
        {
            // Executable found in PATH, use that
            let mut cmd = Command::new(&exe_path);
            cmd.current_dir(project_root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            for arg in &config.args {
                cmd.arg(arg);
            }

            // Spawn but don't wait - we just want to verify it starts
            match cmd.spawn() {
                Ok(mut child) => {
                    debug!("LSP server process spawned with PID: {:?}", child.id());

                    // Try to wait briefly with timeout to catch immediate errors
                    // In a real implementation, this would establish JSON-RPC communication
                    std::thread::sleep(Duration::from_millis(100));

                    // Check if process is still running
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            Err(io::Error::other("LSP server process exited immediately"))?
                        }
                        Ok(None) => {
                            // Process still running, good!
                            // In a production implementation, we'd establish stdio channels here
                            debug!("LSP server process is running");
                            Ok(())
                        }
                        Err(e) => Err(e)?,
                    }
                }
                Err(e) => Err(e)?,
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "LSP server executable not found: {}",
                    config.executable.display()
                ),
            ))?
        }
    } else {
        let mut cmd = Command::new(&config.executable);
        cmd.current_dir(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for arg in &config.args {
            cmd.arg(arg);
        }

        match cmd.spawn() {
            Ok(mut child) => {
                debug!("LSP server process spawned with PID: {:?}", child.id());

                // Brief wait to catch immediate errors
                std::thread::sleep(Duration::from_millis(100));

                match child.try_wait() {
                    Ok(Some(_)) => Err(io::Error::other("LSP server process exited immediately"))?,
                    Ok(None) => {
                        debug!("LSP server process is running");
                        Ok(())
                    }
                    Err(e) => Err(e)?,
                }
            }
            Err(e) => Err(e)?,
        }
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
    fn test_unsupported_language() {
        let tmp = tempfile::tempdir().unwrap();
        let result = start_lsp_server("unsupported_lang", tmp.path());
        assert!(!result.started, "Unsupported language should fail to start");
        assert!(result.error.is_some(), "Error message should be provided");
        assert!(
            result.error.unwrap().contains("Unsupported language"),
            "Error should mention unsupported language"
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
    fn test_rust_analyzer_config() {
        let config = create_rust_analyzer_config();
        assert_eq!(config.language, "rust");
        assert_eq!(config.executable.file_name().unwrap(), "rust-analyzer");
        assert_eq!(config.init_timeout, 30);
    }

    #[test]
    fn test_default_config() {
        let config = LspServerConfig::default();
        assert_eq!(config.language, "rust");
        assert_eq!(config.executable, PathBuf::from("rust-analyzer"));
        assert!(config.args.is_empty());
        assert_eq!(config.init_timeout, 30);
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
    fn test_start_lsp_server_rust_without_analyzer() {
        // If rust-analyzer is not installed, start_lsp_server("rust", ...) should
        // return a handle with started=false and an error. If it IS installed,
        // the process will be spawned (and may or may not succeed).
        let tmp = tempfile::tempdir().unwrap();
        let result = start_lsp_server("rust", tmp.path());
        // Regardless of whether rust-analyzer is installed, the handle should have
        // the correct language field.
        assert_eq!(result.language, "rust");
        // We can't assert started true/false since it depends on the environment,
        // but we verify the fields are consistent.
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
}
