use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

mod test_utils;
use test_utils::ProcessGuard;

/// Test that MCP server logs to ./.swissarmyhammer/mcp.log by default
#[tokio::test]
async fn test_mcp_logging_to_current_directory() {
    // Clean up any existing home logs from previous tests
    if let Some(home_dir) = dirs::home_dir() {
        let home_log_file = home_dir.join(".swissarmyhammer").join("mcp.log");
        if home_log_file.exists() {
            let _ = std::fs::remove_file(&home_log_file);
        }
    }

    let temp_dir = TempDir::new().unwrap();
    let work_dir = temp_dir.path();

    // Start MCP server in temp directory with stdin piped (to trigger MCP mode)
    // Use the compiled binary instead of cargo run
    let binary_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("sah");

    let mut child = Command::new(&binary_path)
        .args(["serve"])
        .current_dir(work_dir) // Run in the temp directory
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    // Send an invalid JSON to stdin to trigger the logging setup
    // (the server will try to read from stdin and start logging)
    if let Some(mut stdin) = child.stdin.take() {
        let _ = std::io::Write::write_all(&mut stdin, b"invalid json\n");
        let _ = std::io::Write::flush(&mut stdin);
        // Don't drop stdin immediately - keep it open
        std::mem::forget(stdin);
    }

    let _child = ProcessGuard(child);

    // Give the server time to start and create logs
    std::thread::sleep(Duration::from_millis(2000));

    // Verify log file was created in current directory
    let expected_log_dir = work_dir.join(".swissarmyhammer");
    let expected_log_file = expected_log_dir.join("mcp.log");

    // Logs should be created in .swissarmyhammer directory in current working directory
    assert!(
        expected_log_file.exists(),
        "Log file should be created at {expected_log_file:?}"
    );

    // Verify logs contain MCP server startup messages
    let log_content = std::fs::read_to_string(&expected_log_file).unwrap();
    assert!(
        log_content.contains("MCP server"),
        "Log should contain MCP server messages"
    );

    // Verify no logs in home directory
    if let Some(home_dir) = dirs::home_dir() {
        let home_log_file = home_dir.join(".swissarmyhammer").join("mcp.log");
        assert!(
            !home_log_file.exists()
                || std::fs::read_to_string(&home_log_file)
                    .unwrap_or_default()
                    .is_empty(),
            "Home directory should not contain MCP logs"
        );
    }
}

/// Test that SWISSARMYHAMMER_LOG_FILE environment variable overrides log filename
#[tokio::test]
async fn test_mcp_logging_env_var_override() {
    let temp_dir = TempDir::new().unwrap();
    let work_dir = temp_dir.path();
    let custom_log_name = "custom-test.log";

    // Start MCP server with custom log file name
    // Use the compiled binary instead of cargo run
    let binary_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("sah");

    let child = Command::new(&binary_path)
        .args(["serve"])
        .current_dir(work_dir)
        .env("SWISSARMYHAMMER_LOG_FILE", custom_log_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let _child = ProcessGuard(child);

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(2000));

    // Verify custom log file was created
    let expected_log_dir = work_dir.join(".swissarmyhammer");
    let expected_log_file = expected_log_dir.join(custom_log_name);

    assert!(
        expected_log_file.exists(),
        "Custom log file should be created at {expected_log_file:?}"
    );

    // Verify default log file was NOT created
    let default_log_file = expected_log_dir.join("mcp.log");
    assert!(
        !default_log_file.exists(),
        "Default log file should not exist when custom name is used"
    );
}

/// Test that log directory is created if it doesn't exist
#[tokio::test]
async fn test_mcp_logging_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let work_dir = temp_dir.path();

    // Ensure .swissarmyhammer directory doesn't exist initially
    let log_dir = work_dir.join(".swissarmyhammer");
    assert!(
        !log_dir.exists(),
        "Log directory should not exist initially"
    );

    // Start MCP server
    // Use the compiled binary instead of cargo run
    let binary_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("sah");

    let child = Command::new(&binary_path)
        .args(["serve"])
        .current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let _child = ProcessGuard(child);

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(2000));

    // Verify directory was created
    assert!(log_dir.exists(), "Log directory should be created");
    assert!(log_dir.is_dir(), "Log directory should be a directory");

    // Verify log file was created
    let log_file = log_dir.join("mcp.log");
    assert!(log_file.exists(), "Log file should be created");
}
