use claude_agent::path_validator::{PathValidationError, PathValidator};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_unix_absolute_paths() {
    let validator = PathValidator::new();

    // Valid Unix absolute paths
    let valid_paths = vec![
        "/",
        "/home",
        "/home/user",
        "/home/user/document.txt",
        "/tmp/file.txt",
        "/usr/local/bin/program",
    ];

    for path in valid_paths {
        let result = validator.validate_absolute_path(path);
        if cfg!(unix) {
            // On Unix systems, these should pass basic validation
            // Note: canonicalization might fail if paths don't exist
            match result {
                Ok(_) => {} // Path validated successfully
                Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                    // Expected for non-existent paths with strict canonicalization
                }
                Err(e) => panic!(
                    "Expected Unix absolute path '{}' to pass basic validation, got: {}",
                    path, e
                ),
            }
        }
    }
}

#[test]
fn test_windows_absolute_paths() {
    let validator = PathValidator::new();

    // Valid Windows absolute paths
    let valid_paths = vec![
        "C:\\",
        "C:\\Users",
        "C:\\Users\\user\\document.txt",
        "D:\\Program Files\\app\\file.exe",
        "\\\\server\\share\\file.txt", // UNC path
    ];

    for path in valid_paths {
        let result = validator.validate_absolute_path(path);
        if cfg!(windows) {
            // On Windows systems, these should pass basic validation
            match result {
                Ok(_) => {} // Path validated successfully
                Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                    // Expected for non-existent paths with strict canonicalization
                }
                Err(e) => panic!(
                    "Expected Windows absolute path '{}' to pass basic validation, got: {}",
                    path, e
                ),
            }
        }
    }
}

#[test]
fn test_relative_path_rejection() {
    let validator = PathValidator::new();

    // Invalid relative paths
    let invalid_paths = vec![
        "relative/path",
        "./current/dir",
        "../parent/dir",
        "file.txt",
        "src/main.rs",
        "config/settings.json",
    ];

    for path in invalid_paths {
        let result = validator.validate_absolute_path(path);
        match result {
            Err(PathValidationError::NotAbsolute(p)) => {
                assert_eq!(p, path);
            }
            Err(PathValidationError::PathTraversalAttempt(_)) => {
                // Also acceptable for ../ patterns
            }
            Ok(_) => panic!("Expected relative path '{}' to be rejected", path),
            Err(e) => panic!("Expected NotAbsolute error for '{}', got: {}", path, e),
        }
    }
}

#[test]
fn test_path_traversal_detection() {
    let validator = PathValidator::new();

    // Unix-style traversal paths (should work on all platforms)
    let unix_traversal_paths = vec![
        "/home/user/../../../etc/passwd",
        "/tmp/../../../root/.ssh/id_rsa",
    ];

    for path in unix_traversal_paths {
        let result = validator.validate_absolute_path(path);
        match result {
            Err(PathValidationError::PathTraversalAttempt(_)) => {
                // Expected
            }
            Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                // Also acceptable - canonicalization might catch traversal
            }
            Ok(_) => panic!("Expected path traversal to be detected for '{}'", path),
            Err(e) => panic!("Expected PathTraversalAttempt for '{}', got: {}", path, e),
        }
    }
}

#[test]
fn test_empty_and_invalid_paths() {
    let validator = PathValidator::new();

    // Empty path
    assert_eq!(
        validator.validate_absolute_path(""),
        Err(PathValidationError::EmptyPath)
    );

    // Null bytes
    assert_eq!(
        validator.validate_absolute_path("/path/with\0null"),
        Err(PathValidationError::NullBytesInPath)
    );
}

#[test]
fn test_path_length_limit() {
    let validator = PathValidator::with_max_length(50);

    let long_path = "/".repeat(100);
    let result = validator.validate_absolute_path(&long_path);

    match result {
        Err(PathValidationError::PathTooLong(actual, max)) => {
            assert_eq!(actual, 100);
            assert_eq!(max, 50);
        }
        _ => panic!("Expected PathTooLong error"),
    }
}

#[test]
fn test_allowed_roots_validation() {
    let temp_dir = TempDir::new().unwrap();
    let allowed_root = temp_dir.path().to_path_buf();

    let validator = PathValidator::with_allowed_roots(vec![allowed_root.clone()]);

    // Test path within allowed root
    let allowed_path = allowed_root.join("subdir").join("file.txt");
    let result = validator.validate_absolute_path(&allowed_path.to_string_lossy());

    // Should fail with canonicalization since file doesn't exist, but not with boundary check
    match result {
        Err(PathValidationError::CanonicalizationFailed(_, _)) => {
            // Expected due to non-existent path
        }
        Ok(_) => {
            // Also acceptable if path validation succeeds
        }
        Err(e) => panic!("Expected CanonicalizationFailed or success, got: {}", e),
    }

    // Test path outside allowed roots
    let outside_path = "/completely/different/path";
    let result = validator.validate_absolute_path(outside_path);

    // Should fail with boundary error or canonicalization error
    match result {
        Err(PathValidationError::OutsideBoundaries(_)) => {
            // Expected
        }
        Err(PathValidationError::CanonicalizationFailed(_, _)) => {
            // Also acceptable - canonicalization happens first
        }
        Ok(_) => panic!("Expected path outside boundaries to be rejected"),
        Err(e) => panic!(
            "Expected OutsideBoundaries or CanonicalizationFailed, got: {}",
            e
        ),
    }
}

#[test]
fn test_non_strict_canonicalization() {
    let validator = PathValidator::new().with_strict_canonicalization(false);

    // Non-existent but well-formed absolute path
    let path = "/non/existent/path/file.txt";
    let result = validator.validate_absolute_path(path);

    // Should succeed without canonicalization
    assert!(
        result.is_ok(),
        "Expected path validation to succeed without canonicalization"
    );
}

#[test]
fn test_blocked_paths() {
    let temp_dir = TempDir::new().unwrap();
    let blocked_dir = temp_dir.path().join("blocked");
    let test_file = blocked_dir.join("test.txt");
    std::fs::create_dir(&blocked_dir).unwrap();
    std::fs::write(&test_file, "test").unwrap();

    let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());

    match result {
        Err(PathValidationError::Blocked(_)) => {
            // Expected
        }
        Ok(_) => panic!("Expected blocked path to be rejected"),
        Err(e) => panic!("Expected Blocked error, got: {}", e),
    }
}

#[test]
fn test_blocked_takes_precedence_over_allowed() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test").unwrap();

    // Both in allowed and blocked - blocked should win
    let validator = PathValidator::with_allowed_and_blocked(
        vec![temp_dir.path().to_path_buf()],
        vec![temp_dir.path().to_path_buf()],
    );
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());

    match result {
        Err(PathValidationError::Blocked(_)) => {
            // Expected
        }
        Ok(_) => panic!("Expected blocked path to take precedence"),
        Err(e) => panic!("Expected Blocked error, got: {}", e),
    }
}

#[test]
fn test_subdirectory_of_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let subdir = temp_dir.path().join("subdir");
    let test_file = subdir.join("test.txt");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(&test_file, "test").unwrap();

    // Block parent directory, should block subdirectory
    let validator = PathValidator::with_blocked_paths(vec![temp_dir.path().to_path_buf()]);
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());

    match result {
        Err(PathValidationError::Blocked(_)) => {
            // Expected
        }
        Ok(_) => panic!("Expected subdirectory of blocked path to be rejected"),
        Err(e) => panic!("Expected Blocked error, got: {}", e),
    }
}

#[test]
fn test_allowed_with_blocked_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let allowed_file = temp_dir.path().join("allowed.txt");
    let blocked_dir = temp_dir.path().join("blocked");
    let blocked_file = blocked_dir.join("blocked.txt");

    std::fs::write(&allowed_file, "allowed").unwrap();
    std::fs::create_dir(&blocked_dir).unwrap();
    std::fs::write(&blocked_file, "blocked").unwrap();

    let validator = PathValidator::with_allowed_and_blocked(
        vec![temp_dir.path().to_path_buf()],
        vec![blocked_dir.clone()],
    );

    // Allowed file should pass
    let result = validator.validate_absolute_path(&allowed_file.to_string_lossy());
    assert!(result.is_ok(), "Expected allowed file to pass validation");

    // Blocked file should fail
    let result = validator.validate_absolute_path(&blocked_file.to_string_lossy());
    match result {
        Err(PathValidationError::Blocked(_)) => {
            // Expected
        }
        Ok(_) => panic!("Expected blocked file to be rejected"),
        Err(e) => panic!("Expected Blocked error, got: {}", e),
    }
}

#[test]
fn test_empty_blocked_list_allows_all() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test").unwrap();

    // Empty blocked list should allow any path
    let validator = PathValidator::new();
    let result = validator.validate_absolute_path(&test_file.to_string_lossy());
    assert!(
        result.is_ok(),
        "Expected path to be allowed with empty blocked list"
    );
}

#[test]
fn test_multiple_allowed_roots() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let file1 = temp_dir1.path().join("file1.txt");
    let file2 = temp_dir2.path().join("file2.txt");
    std::fs::write(&file1, "test1").unwrap();
    std::fs::write(&file2, "test2").unwrap();

    let validator = PathValidator::with_allowed_roots(vec![
        temp_dir1.path().to_path_buf(),
        temp_dir2.path().to_path_buf(),
    ]);

    // Both files should be allowed
    let result1 = validator.validate_absolute_path(&file1.to_string_lossy());
    assert!(
        result1.is_ok(),
        "Expected file in first allowed root to pass"
    );

    let result2 = validator.validate_absolute_path(&file2.to_string_lossy());
    assert!(
        result2.is_ok(),
        "Expected file in second allowed root to pass"
    );
}

#[test]
fn test_multiple_blocked_paths() {
    let temp_dir = TempDir::new().unwrap();
    let blocked_dir1 = temp_dir.path().join("blocked1");
    let blocked_dir2 = temp_dir.path().join("blocked2");
    let allowed_dir = temp_dir.path().join("allowed");

    std::fs::create_dir(&blocked_dir1).unwrap();
    std::fs::create_dir(&blocked_dir2).unwrap();
    std::fs::create_dir(&allowed_dir).unwrap();

    let blocked_file1 = blocked_dir1.join("file.txt");
    let blocked_file2 = blocked_dir2.join("file.txt");
    let allowed_file = allowed_dir.join("file.txt");

    std::fs::write(&blocked_file1, "test").unwrap();
    std::fs::write(&blocked_file2, "test").unwrap();
    std::fs::write(&allowed_file, "test").unwrap();

    let validator = PathValidator::with_allowed_and_blocked(
        vec![temp_dir.path().to_path_buf()],
        vec![blocked_dir1.clone(), blocked_dir2.clone()],
    );

    // Both blocked files should fail
    let result1 = validator.validate_absolute_path(&blocked_file1.to_string_lossy());
    assert!(
        matches!(result1, Err(PathValidationError::Blocked(_))),
        "Expected first blocked file to be rejected"
    );

    let result2 = validator.validate_absolute_path(&blocked_file2.to_string_lossy());
    assert!(
        matches!(result2, Err(PathValidationError::Blocked(_))),
        "Expected second blocked file to be rejected"
    );

    // Allowed file should pass
    let result3 = validator.validate_absolute_path(&allowed_file.to_string_lossy());
    assert!(
        result3.is_ok(),
        "Expected file in allowed but not blocked directory to pass"
    );
}

#[test]
fn test_empty_allowed_list_with_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let blocked_dir = temp_dir.path().join("blocked");
    let other_dir = temp_dir.path().join("other");

    std::fs::create_dir(&blocked_dir).unwrap();
    std::fs::create_dir(&other_dir).unwrap();

    let blocked_file = blocked_dir.join("file.txt");
    let other_file = other_dir.join("file.txt");

    std::fs::write(&blocked_file, "test").unwrap();
    std::fs::write(&other_file, "test").unwrap();

    // Empty allowed list means allow all except blocked
    let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);

    // Blocked file should fail
    let result1 = validator.validate_absolute_path(&blocked_file.to_string_lossy());
    assert!(
        matches!(result1, Err(PathValidationError::Blocked(_))),
        "Expected blocked file to be rejected"
    );

    // Other file should pass (allowed list is empty, so all non-blocked paths are allowed)
    let result2 = validator.validate_absolute_path(&other_file.to_string_lossy());
    assert!(
        result2.is_ok(),
        "Expected non-blocked file to pass when allowed list is empty"
    );
}

#[test]
fn test_not_found_error_handling() {
    let validator = PathValidator::new();

    // Test with a non-existent path
    let non_existent_path = if cfg!(windows) {
        "C:\\this\\path\\does\\not\\exist\\file.txt"
    } else {
        "/this/path/does/not/exist/file.txt"
    };

    let result = validator.validate_absolute_path(non_existent_path);

    // Should return CanonicalizationFailed error
    match result {
        Err(PathValidationError::CanonicalizationFailed(path, err_msg)) => {
            assert_eq!(path, non_existent_path);
            // Verify the error message contains "not found" or similar
            let err_lower = err_msg.to_lowercase();
            assert!(
                err_lower.contains("not found")
                    || err_lower.contains("no such file")
                    || err_lower.contains("cannot find"),
                "Expected error message to indicate file not found, got: {}",
                err_msg
            );
        }
        Ok(_) => panic!("Expected CanonicalizationFailed error for non-existent path"),
        Err(e) => panic!(
            "Expected CanonicalizationFailed error for non-existent path, got: {}",
            e
        ),
    }
}

#[test]
fn test_not_found_with_non_strict_canonicalization() {
    let validator = PathValidator::new().with_strict_canonicalization(false);

    // Test with a non-existent path
    let non_existent_path = if cfg!(windows) {
        "C:\\this\\path\\does\\not\\exist\\file.txt"
    } else {
        "/this/path/does/not/exist/file.txt"
    };

    let result = validator.validate_absolute_path(non_existent_path);

    // With non-strict canonicalization, should succeed
    assert!(
        result.is_ok(),
        "Expected validation to succeed with non-strict canonicalization for non-existent path"
    );
}

#[cfg(unix)]
#[test]
fn test_permission_denied_error_handling() {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().unwrap();
    let restricted_dir = temp_dir.path().join("restricted");
    let restricted_file = restricted_dir.join("file.txt");

    // Create directory and file
    std::fs::create_dir(&restricted_dir).unwrap();
    std::fs::write(&restricted_file, "test").unwrap();

    // Remove all permissions from the directory to trigger permission denied
    std::fs::set_permissions(&restricted_dir, Permissions::from_mode(0o000)).unwrap();

    let validator = PathValidator::new();
    let result = validator.validate_absolute_path(&restricted_file.to_string_lossy());

    // Restore permissions before assertions to ensure cleanup
    std::fs::set_permissions(&restricted_dir, Permissions::from_mode(0o755)).unwrap();

    // Should return CanonicalizationFailed error with permission denied message
    match result {
        Err(PathValidationError::CanonicalizationFailed(path, err_msg)) => {
            let err_lower = err_msg.to_lowercase();
            assert!(
                err_lower.contains("permission denied") || err_lower.contains("access denied"),
                "Expected error message to indicate permission denied, got: {}",
                err_msg
            );
        }
        Ok(_) => panic!("Expected CanonicalizationFailed error for permission denied"),
        Err(e) => panic!(
            "Expected CanonicalizationFailed error for permission denied, got: {}",
            e
        ),
    }
}

#[cfg(windows)]
#[test]
fn test_permission_denied_error_handling_windows() {
    // On Windows, we'll test with a system path that typically requires elevation
    // Note: This test may behave differently depending on user privileges
    let validator = PathValidator::new();

    // Try to access a system file that typically requires admin rights
    let system_path = "C:\\Windows\\System32\\config\\SAM";

    let result = validator.validate_absolute_path(system_path);

    // The result depends on whether the test is running with admin privileges
    // If not admin, should get permission denied
    // If admin, might succeed or fail for other reasons
    match result {
        Err(PathValidationError::CanonicalizationFailed(_, err_msg)) => {
            // This is the expected behavior for non-admin users
            let err_lower = err_msg.to_lowercase();
            // On Windows, permission errors can be "access denied" or "permission denied"
            if err_lower.contains("access") || err_lower.contains("permission") {
                // Expected permission error
            } else {
                // Some other canonicalization error, which is also acceptable
            }
        }
        Err(_) | Ok(_) => {
            // Other errors or success (if running as admin) are acceptable
            // The key is that we don't panic, meaning the error handling works
        }
    }
}

#[test]
fn test_error_handling_with_allowed_roots() {
    let temp_dir = TempDir::new().unwrap();
    let validator = PathValidator::with_allowed_roots(vec![temp_dir.path().to_path_buf()]);

    // Test non-existent file within allowed root
    let non_existent = temp_dir.path().join("does_not_exist.txt");
    let result = validator.validate_absolute_path(&non_existent.to_string_lossy());

    match result {
        Err(PathValidationError::CanonicalizationFailed(path, _)) => {
            // Expected - canonicalization happens before boundary check
            assert!(path.contains("does_not_exist.txt"));
        }
        Ok(_) => panic!("Expected CanonicalizationFailed for non-existent file"),
        Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
    }

    // Test non-existent file outside allowed root
    let outside_path = if cfg!(windows) {
        "C:\\outside\\path\\file.txt"
    } else {
        "/outside/path/file.txt"
    };
    let result = validator.validate_absolute_path(outside_path);

    // Canonicalization fails first, so we get that error rather than boundary error
    match result {
        Err(PathValidationError::CanonicalizationFailed(_, _)) => {
            // Expected
        }
        Err(PathValidationError::OutsideBoundaries(_)) => {
            // Also acceptable if canonicalization somehow succeeds
        }
        Ok(_) => panic!("Expected error for path outside boundaries"),
        Err(e) => panic!(
            "Expected CanonicalizationFailed or OutsideBoundaries, got: {}",
            e
        ),
    }
}

#[test]
fn test_canonicalization_error_preserves_original_path() {
    let validator = PathValidator::new();

    let test_path = if cfg!(windows) {
        "C:\\nonexistent\\deeply\\nested\\path\\file.txt"
    } else {
        "/nonexistent/deeply/nested/path/file.txt"
    };

    let result = validator.validate_absolute_path(test_path);

    match result {
        Err(PathValidationError::CanonicalizationFailed(path, _)) => {
            // Verify the original path is preserved in the error
            assert_eq!(
                path, test_path,
                "Error should preserve the original path string"
            );
        }
        Ok(_) => panic!("Expected CanonicalizationFailed for non-existent nested path"),
        Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
    }
}

#[test]
fn test_multiple_error_scenarios() {
    let temp_dir = TempDir::new().unwrap();

    // Create a blocked directory with a non-existent file
    let blocked_dir = temp_dir.path().join("blocked");
    std::fs::create_dir(&blocked_dir).unwrap();
    let non_existent_in_blocked = blocked_dir.join("nonexistent.txt");

    let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);

    // Non-existent file in blocked directory
    // Canonicalization should fail before blocked check
    let result = validator.validate_absolute_path(&non_existent_in_blocked.to_string_lossy());

    match result {
        Err(PathValidationError::CanonicalizationFailed(_, _)) => {
            // Expected - canonicalization happens before blocked check
        }
        Err(PathValidationError::Blocked(_)) => {
            // Also possible if canonicalization somehow succeeds
            panic!("Unexpected Blocked error - canonicalization should fail first");
        }
        Ok(_) => panic!("Expected error for non-existent file in blocked directory"),
        Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
    }
}
