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

    // Get path to the sah binary
    let sah_path = assert_cmd::cargo::cargo_bin("sah");

    // Start MCP server in temp directory with stdin piped (to trigger MCP mode)
    let mut child = Command::new(sah_path)
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
    let expected_log_dir = work_dir.join(".swissarmyhammer");
    let expected_log_file = expected_log_dir.join("mcp.log");

    // Wait for log file to be created, up to 5 seconds
    let mut attempts = 0;
    while !expected_log_file.exists() && attempts < 50 {
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

    // Logs should be created in .swissarmyhammer directory in current working directory
    assert!(
        expected_log_file.exists(),
        "Log file should be created at {expected_log_file:?}"
    );

    // Wait for logs to contain MCP server startup messages, up to 5 seconds
    let mut attempts = 0;
    let mut log_content = String::new();
    while attempts < 50 {
        if expected_log_file.exists() {
            log_content = std::fs::read_to_string(&expected_log_file).unwrap_or_default();
            if log_content.contains("MCP server") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

    assert!(
        log_content.contains("MCP server"),
        "Log should contain MCP server messages, but got: {log_content}"
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
    let sah_path = assert_cmd::cargo::cargo_bin("sah");
    let child = Command::new(sah_path)
        .args(["serve"])
        .current_dir(work_dir)
        .env("SWISSARMYHAMMER_LOG_FILE", custom_log_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let _child = ProcessGuard(child);

    // Verify custom log file was created
    let expected_log_dir = work_dir.join(".swissarmyhammer");
    let expected_log_file = expected_log_dir.join(custom_log_name);

    // Wait for log file to be created, up to 5 seconds
    let mut attempts = 0;
    while !expected_log_file.exists() && attempts < 50 {
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

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
    let sah_path = assert_cmd::cargo::cargo_bin("sah");
    let child = Command::new(sah_path)
        .args(["serve"])
        .current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let _child = ProcessGuard(child);

    // Wait for directory to be created, up to 5 seconds
    let mut attempts = 0;
    while !log_dir.exists() && attempts < 50 {
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

    // Verify directory was created
    assert!(log_dir.exists(), "Log directory should be created");
    assert!(log_dir.is_dir(), "Log directory should be a directory");

    // Verify log file was created
    let log_file = log_dir.join("mcp.log");

    // Wait for log file to be created
    let mut attempts = 0;
    while !log_file.exists() && attempts < 50 {
        std::thread::sleep(Duration::from_millis(100));
        attempts += 1;
    }

    assert!(log_file.exists(), "Log file should be created");
}
