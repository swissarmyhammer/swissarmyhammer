use claude_agent::path_validator::PathValidator;
use tempfile::TempDir;

#[test]
fn test_audit_logging_for_path_traversal() {
    // Initialize tracing subscriber for test
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let validator = PathValidator::new();

    // Test path traversal attempt - should log security event
    let result = validator.validate_absolute_path("/tmp/../etc/passwd");
    assert!(result.is_err(), "Path traversal should be rejected");

    // Test with Windows-style path traversal
    let result = validator.validate_absolute_path("/home\\..\\root");
    assert!(
        result.is_err(),
        "Windows-style path traversal should be rejected"
    );
}

#[test]
fn test_audit_logging_for_blocked_paths() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let temp_dir = TempDir::new().unwrap();
    let blocked_dir = temp_dir.path().join("blocked");
    let test_file = blocked_dir.join("test.txt");
    std::fs::create_dir(&blocked_dir).unwrap();
    std::fs::write(&test_file, "test").unwrap();

    let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);

    // Test blocked path access - should log security event
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());
    assert!(result.is_err(), "Blocked path should be rejected");
}

#[test]
fn test_audit_logging_for_boundary_violations() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let temp_dir = TempDir::new().unwrap();
    let validator = PathValidator::with_allowed_roots(vec![temp_dir.path().to_path_buf()]);

    // Test path outside allowed boundaries - should log security event
    let outside_path = if cfg!(windows) {
        "C:\\outside\\path.txt"
    } else {
        "/outside/path.txt"
    };
    let result = validator.validate_absolute_path(outside_path);
    assert!(
        result.is_err(),
        "Path outside boundaries should be rejected"
    );
}

#[test]
fn test_audit_logging_for_invalid_paths() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let validator = PathValidator::new();

    // Test empty path - should log security event
    let result = validator.validate_absolute_path("");
    assert!(result.is_err(), "Empty path should be rejected");

    // Test null bytes - should log security event
    let result = validator.validate_absolute_path("/path/with\0null");
    assert!(result.is_err(), "Path with null bytes should be rejected");

    // Test relative path - should log security event
    let result = validator.validate_absolute_path("relative/path");
    assert!(result.is_err(), "Relative path should be rejected");
}

#[test]
fn test_no_audit_logging_for_valid_paths() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("valid.txt");
    std::fs::write(&test_file, "content").unwrap();

    let validator = PathValidator::new();

    // Test valid path - should not log security event
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());
    assert!(result.is_ok(), "Valid path should be accepted");
}
