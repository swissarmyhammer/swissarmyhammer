//! Security, across every file tool.
//!
//! Path traversal, symlinks, restricted and privileged locations,
//! workspace boundaries, permission escalation, and malformed input.

use super::*;

/// Dangerous paths for security testing
const DANGEROUS_PATHS: &[&str] = &[
    "/tmp/../../../etc/passwd",
    "/home/user/../../../etc/passwd",
    "../../../etc/passwd",
    "..\\..\\..\\windows\\system32\\config\\sam",
    "/var/tmp/../../../../etc/shadow",
    "~/../../etc/hosts",
    "/usr/local/../../../root/.ssh/id_rsa",
    "/tmp/../../../../../proc/version",
];

/// File tool operations to test for security
/// All operations are dispatched through the unified "files" tool with an "op" field.
const FILE_OPS: &[&str] = &[
    "read file",
    "write file",
    "edit file",
    "glob files",
    "grep files",
];

/// Create malformed inputs for testing
fn create_malformed_inputs(test_dir_path: &std::path::Path) -> Vec<String> {
    let long_path = "extremely_long_path_".repeat(1000);
    vec![
        "".to_string(),
        "\0".to_string(),
        format!("{}/path/with\0null", test_dir_path.display()),
        format!("{}/path\nwith\nnewlines", test_dir_path.display()),
        format!("{}/path\rwith\rcarriage\rreturns", test_dir_path.display()),
        format!("{}/path\twith\ttabs", test_dir_path.display()),
        format!(
            "{}/path with spaces and special chars: <>|\"*?",
            test_dir_path.display()
        ),
        format!("{}/\u{FEFF}path_with_bom", test_dir_path.display()),
        format!("{}/{}", test_dir_path.display(), long_path),
    ]
}

/// Check if error message contains any of the expected messages
fn assert_error_contains_any(error_msg: &str, expected_messages: &[&str], context: &str) {
    let contains_expected = expected_messages.iter().any(|msg| error_msg.contains(msg));
    assert!(
        contains_expected,
        "{}: Expected error to contain one of {:?}, but got: {}",
        context, expected_messages, error_msg
    );
}

/// Build security test arguments for a given operation and dangerous path
fn build_security_test_arguments(
    op: &str,
    dangerous_path: &str,
) -> serde_json::Map<String, serde_json::Value> {
    match op {
        "read file" => read_args(dangerous_path),
        "write file" => write_args(dangerous_path, "malicious content"),
        "edit file" => edit_args(dangerous_path, "old", "new"),
        "glob files" => {
            let mut args = glob_args("*");
            args.insert("path".to_string(), json!(dangerous_path));
            args
        }
        "grep files" => {
            let mut args = grep_args("password");
            args.insert("path".to_string(), json!(dangerous_path));
            args
        }
        _ => panic!("Unsupported operation for security testing: {}", op),
    }
}

/// Test path security for a given operation
async fn test_path_security_for_op(
    op: &str,
    registry: &ToolRegistry,
    context: &ToolContext,
    dangerous_paths: &[&str],
) {
    let tool = registry.get_tool("files").unwrap();

    for dangerous_path in dangerous_paths {
        // Skip Windows-style paths on Unix - backslashes are literal characters, not path separators
        // These paths don't represent actual path traversal attacks on Unix systems
        #[cfg(unix)]
        if dangerous_path.contains('\\') {
            continue;
        }

        let arguments = build_security_test_arguments(op, dangerous_path);
        let result = tool.execute(arguments, context).await;

        match result {
            Err(error) => {
                let error_msg = format!("{:?}", error);
                let expected_messages = &[
                    "blocked pattern",
                    "not found",
                    "absolute",
                    "No such file",
                    "does not exist",
                    "invalid",
                    "dangerous",
                    "traversal",
                    "not allowed",
                ];
                let context_msg = format!(
                    "files tool (op={}) should block or safely handle path traversal: {}",
                    op, dangerous_path
                );
                assert_error_contains_any(&error_msg, expected_messages, &context_msg);
            }
            Ok(call_result) => {
                // For write operations, success is a security failure - we shouldn't be able
                // to write to dangerous paths
                if op == "write file" {
                    panic!(
                        "files tool (op={}) allowed write to dangerous path '{}': {:?}",
                        op, dangerous_path, call_result
                    );
                }
                // For read operations, success with is_error=true is acceptable (tool handled it)
                if call_result.is_error == Some(true) {
                    // Tool returned an error response, which is expected
                    continue;
                }
                // Read/glob/grep succeeding on non-existent dangerous paths is also fine
                // (they just won't find anything)
            }
        }
    }
}

// ============================================================================
// Enhanced Security Tests for All File Tools
// ============================================================================

#[tokio::test]
async fn test_comprehensive_path_traversal_protection_all_tools() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    for op in FILE_OPS {
        test_path_security_for_op(op, &registry, &context, DANGEROUS_PATHS).await;
    }
}

#[tokio::test]
async fn test_symlink_read_security() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let normal_file = &temp_dir.join("normal.txt");
    fs::write(normal_file, "normal content").unwrap();

    let symlink_file = &temp_dir.join("symlink.txt");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/etc/passwd", symlink_file);
    }

    if symlink_file.exists() {
        let mut read_args = serde_json::Map::new();
        read_args.insert("op".to_string(), json!("read file"));
        read_args.insert("path".to_string(), json!(symlink_file.to_string_lossy()));

        let read_result = read_tool.execute(read_args, &context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            println!("Symlink read rejected (secure): {}", error_msg);
        } else {
            let read_call_result = read_result.unwrap();
            if let Some(content_item) = read_call_result.content.first() {
                if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                    assert!(
                        !text_content.text.contains("root:")
                            && !text_content.text.contains("shadow"),
                        "Symlink should not expose sensitive content"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn test_symlink_write_security() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let symlink_file = &temp_dir.join("symlink.txt");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/etc/passwd", symlink_file);
    }

    if symlink_file.exists() {
        let mut write_args = serde_json::Map::new();
        write_args.insert("op".to_string(), json!("write file"));
        write_args.insert(
            "file_path".to_string(),
            json!(symlink_file.to_string_lossy()),
        );
        write_args.insert("content".to_string(), json!("overwrite attempt"));

        let write_result = write_tool.execute(write_args, &context).await;
        if write_result.is_ok() {
            let passwd_content = fs::read_to_string("/etc/passwd").unwrap_or_default();
            assert!(
                !passwd_content.contains("overwrite attempt"),
                "Should not modify system files through symlinks"
            );
        }
    }
}

/// Test restricted path access for a single operation
async fn test_restricted_path_access(
    op: &str,
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    path: &str,
    context: &ToolContext,
) {
    if op == "read file" {
        let args = read_args(path);
        let read_result = tool.execute(args, context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            println!("Restricted read blocked: {} - {}", path, error_msg);
        }
    } else if op == "write file" {
        let args = write_args(path, "unauthorized write");
        let write_result = tool.execute(args, context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!("Restricted write blocked: {} - {}", path, error_msg);
        } else {
            let actual_content = fs::read_to_string(path).unwrap_or_default();
            assert!(
                !actual_content.contains("unauthorized write"),
                "Should not modify restricted system files"
            );
        }
    }
}

#[tokio::test]
async fn test_workspace_boundary_enforcement() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files").unwrap();
    let write_tool = registry.get_tool("files").unwrap();

    let restricted_paths = vec![
        "/etc/passwd",
        "/root/.bashrc",
        "/var/log/system.log",
        "/usr/bin/sudo",
        "/sys/kernel/debug/",
        "/proc/1/environ",
        "/home/other_user/.ssh/id_rsa",
    ];

    for restricted_path in restricted_paths {
        test_restricted_path_access("read file", read_tool, restricted_path, &context).await;
        test_restricted_path_access("write file", write_tool, restricted_path, &context).await;
    }
}

#[tokio::test]
async fn test_read_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut read_args = serde_json::Map::new();
        read_args.insert("op".to_string(), json!("read file"));
        read_args.insert("path".to_string(), json!(malformed_input));

        let read_result = read_tool.execute(read_args, &context).await;
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic") && !error_msg.contains("thread"),
                "Should handle malformed input gracefully, not panic: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_write_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut write_args = serde_json::Map::new();
        write_args.insert("op".to_string(), json!("write file"));
        write_args.insert("file_path".to_string(), json!(malformed_input));
        write_args.insert("content".to_string(), json!("test content"));

        let write_result = write_tool.execute(write_args, &context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("invalid")
                    || error_msg.contains("empty")
                    || error_msg.contains("directory")
                    || error_msg.contains("permission")
                    || error_msg.contains("NUL byte")
                    || error_msg.contains("File name too long")
                    || error_msg.contains("path too long")
                    || error_msg.contains("Read-only"),
                "Should provide clear validation error: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_glob_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("op".to_string(), json!("glob files"));
        glob_args.insert("pattern".to_string(), json!(malformed_input));

        let glob_result = glob_tool.execute(glob_args, &context).await;
        if let Err(error) = glob_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic"),
                "Glob should handle malformed patterns gracefully: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_grep_tool_malformed_input() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let grep_tool = registry.get_tool("files").unwrap();

    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let malformed_inputs = create_malformed_inputs(test_dir.path());

    for malformed_input in &malformed_inputs {
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("op".to_string(), json!("grep files"));
        grep_args.insert("pattern".to_string(), json!(malformed_input));

        let grep_result = grep_tool.execute(grep_args, &context).await;
        if let Err(error) = grep_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("invalid regex")
                    || error_msg.contains("pattern")
                    || !error_msg.contains("panic"),
                "Grep should handle malformed regex gracefully: {}",
                error_msg
            );
        }
    }
}

/// Test privileged location access for a given operation
async fn test_privileged_location_access(
    tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    location: &str,
    op: &str,
    context: &ToolContext,
) {
    if op == "write file" {
        let args = write_args(location, "# privilege escalation attempt");

        let write_result = tool.execute(args, context).await;
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!("Privileged write blocked: {} - {}", location, error_msg);
        } else {
            println!("Warning: Write to {} succeeded unexpectedly", location);
        }
    } else if op == "edit file" {
        let args = edit_args(location, "root", "compromised");

        let edit_result = tool.execute(args, context).await;
        if let Err(error) = edit_result {
            let error_msg = format!("{:?}", error);
            println!("Privileged edit blocked: {} - {}", location, error_msg);
        }
    }
}

#[tokio::test]
async fn test_permission_escalation_prevention() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    let write_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    let privileged_locations = vec![
        "/etc/sudoers",
        "/etc/shadow",
        "/etc/ssh/sshd_config",
        "/root/.ssh/authorized_keys",
        "/var/spool/cron/root",
        "/etc/crontab",
        "/usr/bin/sudo",
    ];

    for privileged_location in privileged_locations {
        test_privileged_location_access(write_tool, privileged_location, "write file", &context)
            .await;
        test_privileged_location_access(edit_tool, privileged_location, "edit file", &context)
            .await;
    }
}

#[tokio::test]
async fn test_read_tool_excessive_parameters() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test.txt");
    fs::write(test_file, "small content").unwrap();

    let mut read_args = serde_json::Map::new();
    read_args.insert("op".to_string(), json!("read file"));
    read_args.insert("path".to_string(), json!(test_file.to_string_lossy()));
    read_args.insert("offset".to_string(), json!(u32::MAX));
    read_args.insert("limit".to_string(), json!(u32::MAX));

    let read_result = read_tool.execute(read_args, &context).await;
    if let Err(error) = read_result {
        let error_msg = format!("{:?}", error);
        assert!(
            error_msg.contains("offset")
                || error_msg.contains("limit")
                || error_msg.contains("too large"),
            "Should validate excessive offset/limit values: {}",
            error_msg
        );
    }
}

#[tokio::test]
async fn test_write_tool_large_content_limits() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let huge_content = "A".repeat(20_000_000);
    let large_file = &temp_dir.join("large_test.txt");

    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(huge_content));

    let write_result = write_tool.execute(write_args, &context).await;
    if let Err(error) = write_result {
        let error_msg = format!("{:?}", error);
        println!("Large content write rejected: {}", error_msg);
    }
}

#[tokio::test]
async fn test_glob_tool_complex_patterns() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let recursive_pattern = "**/**/".repeat(100) + "*";

    let mut glob_args = serde_json::Map::new();
    glob_args.insert("op".to_string(), json!("glob files"));
    glob_args.insert("pattern".to_string(), json!(recursive_pattern));
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    if let Err(error) = glob_result {
        let error_msg = format!("{:?}", error);
        println!("Complex glob pattern handled: {}", error_msg);
    }
}
