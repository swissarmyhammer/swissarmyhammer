//! The checks a configuration path and a directory must pass.
//!
//! An empty path, a path over the length limit, a suspicious path, a
//! directory given where a file is needed, and a directory that cannot be
//! written.

use super::*;

// ========================================================================
// validate_config_file_path and check_directory_writable tests
// ========================================================================

#[test]
fn test_validate_config_file_path_empty_path() {
    let result = ModelManager::validate_config_file_path(Path::new(""));
    assert!(result.is_err(), "Empty path should be rejected");
    match result.unwrap_err() {
        ModelError::InvalidPath(p) => {
            assert!(p.as_os_str().is_empty(), "Should return the empty path");
        }
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_config_file_path_too_long() {
    let long_path = "a".repeat(4097);
    let result = ModelManager::validate_config_file_path(Path::new(&long_path));
    assert!(
        result.is_err(),
        "Path exceeding 4096 chars should be rejected"
    );
    match result.unwrap_err() {
        ModelError::InvalidPath(_) => {}
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_config_file_path_exactly_max_length() {
    // 4096 chars should be accepted (boundary case)
    let max_path = "a".repeat(4096);
    let result = ModelManager::validate_config_file_path(Path::new(&max_path));
    // Should not fail due to length (may fail for other reasons like file not existing,
    // but the length check should pass)
    match &result {
        Err(ModelError::InvalidPath(p)) => {
            // If it failed, it should not be because of length
            assert_ne!(
                p.to_string_lossy().len(),
                4096,
                "4096-char path should pass the length check"
            );
        }
        _ => {
            // Either Ok or a different error is fine — length check passed
        }
    }
}

#[test]
fn test_validate_config_file_path_suspicious_null_byte() {
    let path_with_null = "config\0.yaml";
    let result = ModelManager::validate_config_file_path(Path::new(path_with_null));
    assert!(
        result.is_err(),
        "Path with null byte should be rejected by suspicious pattern check"
    );
}

#[test]
fn test_validate_config_file_path_directory_not_file() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let dir_path = temp_dir.path();

    // The path exists and is a directory, not a file
    let result = ModelManager::validate_config_file_path(dir_path);
    assert!(result.is_err(), "Directory path should be rejected");
    match result.unwrap_err() {
        ModelError::InvalidPath(p) => {
            assert!(
                p.is_dir() || p.is_absolute(),
                "Should return the canonical directory path"
            );
        }
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_config_file_path_valid_existing_file() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test-config.yaml");
    std::fs::write(&file_path, "model: test\n").expect("Failed to write test file");

    let result = ModelManager::validate_config_file_path(&file_path);
    assert!(result.is_ok(), "Valid file path should be accepted");
    let canonical = result.unwrap();
    assert!(
        canonical.is_absolute(),
        "Should return an absolute/canonical path"
    );
    assert!(canonical.is_file(), "Canonical path should point to a file");
}

#[test]
fn test_validate_config_file_path_nonexistent_file() {
    let result =
        ModelManager::validate_config_file_path(Path::new("/tmp/does-not-exist-config.yaml"));
    assert!(
        result.is_ok(),
        "Non-existent file path should be accepted (returned as-is)"
    );
    let returned = result.unwrap();
    assert_eq!(
        returned,
        PathBuf::from("/tmp/does-not-exist-config.yaml"),
        "Should return the path unchanged for non-existent files"
    );
}

#[test]
fn test_check_directory_writable_valid_dir() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let result = ModelManager::check_directory_writable(temp_dir.path());
    assert!(result.is_ok(), "Writable temp directory should pass");
}

#[test]
fn test_check_directory_writable_not_a_directory() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("regular-file.txt");
    std::fs::write(&file_path, "content").expect("Failed to write file");

    let result = ModelManager::check_directory_writable(&file_path);
    assert!(
        result.is_err(),
        "Regular file should not pass directory check"
    );
    match result.unwrap_err() {
        ModelError::InvalidPath(p) => {
            assert_eq!(p, file_path, "Should return the non-directory path");
        }
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_check_directory_writable_nonexistent_path() {
    let result =
        ModelManager::check_directory_writable(Path::new("/nonexistent/path/does/not/exist"));
    assert!(result.is_err(), "Non-existent path should fail");
    match result.unwrap_err() {
        ModelError::IoError(e) => {
            assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("Expected IoError(NotFound), got: {:?}", other),
    }
}

#[cfg(unix)]
#[test]
fn test_check_directory_writable_readonly_dir() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let readonly_dir = temp_dir.path().join("readonly");
    std::fs::create_dir(&readonly_dir).expect("Failed to create dir");

    // Remove write permission (owner read+execute only)
    std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o500))
        .expect("Failed to set permissions");

    let result = ModelManager::check_directory_writable(&readonly_dir);
    assert!(
        result.is_err(),
        "Read-only directory should fail write check"
    );
    match result.unwrap_err() {
        ModelError::IoError(e) => {
            assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
        }
        other => panic!("Expected IoError(PermissionDenied), got: {:?}", other),
    }

    // Restore permissions so temp_dir cleanup works
    std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o700))
        .expect("Failed to restore permissions");
}

// ── Directory loading pipeline tests ──────────────────────────────

#[test]
fn test_validate_directory_path_empty() {
    // Empty path should return InvalidPath error
    let result = ModelManager::validate_directory_path(Path::new(""));
    assert!(result.is_err(), "Empty path should be rejected");
    match result.unwrap_err() {
        ModelError::InvalidPath(p) => assert!(p.as_os_str().is_empty()),
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_directory_path_too_long() {
    // Path exceeding MAX_PATH_LENGTH (4096) should return InvalidPath error
    let long_component = "a".repeat(4097);
    let long_path = Path::new(&long_component);
    let result = ModelManager::validate_directory_path(long_path);
    assert!(result.is_err(), "Overly long path should be rejected");
    match result.unwrap_err() {
        ModelError::InvalidPath(_) => {} // expected
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_directory_path_nonexistent_returns_ok() {
    // A non-existent but otherwise valid path should return Ok with the
    // original path so that is_valid_directory can handle it gracefully.
    let result = ModelManager::validate_directory_path(Path::new("/tmp/no_such_dir_xyz_test"));
    assert!(
        result.is_ok(),
        "Non-existent path should return Ok (handled later by is_valid_directory)"
    );
}

#[test]
fn test_validate_directory_path_real_directory() {
    // A real, readable directory should canonicalize successfully
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let result = ModelManager::validate_directory_path(temp_dir.path());
    assert!(
        result.is_ok(),
        "Real directory should validate: {:?}",
        result
    );
    // The returned path should be canonical (absolute)
    let validated = result.unwrap();
    assert!(validated.is_absolute());
}

#[test]
fn test_check_directory_permissions_on_file() {
    // Passing a regular file (not a directory) should return InvalidPath
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("regular_file.txt");
    std::fs::write(&file_path, "content").expect("write file");

    let result = ModelManager::check_directory_permissions(&file_path);
    assert!(result.is_err(), "Regular file should fail directory check");
    match result.unwrap_err() {
        ModelError::InvalidPath(_) => {} // expected
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_validate_directory_path_empty_coverage() {
    let result = ModelManager::validate_directory_path(Path::new(""));
    assert!(result.is_err());
}

#[test]
fn test_validate_directory_path_too_long_coverage() {
    let long_path = "a".repeat(5000);
    let result = ModelManager::validate_directory_path(Path::new(&long_path));
    assert!(result.is_err());
}

#[test]
fn test_is_yaml_file() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();

    let yaml_file = temp_dir.path().join("test.yaml");
    std::fs::write(&yaml_file, "key: val").unwrap();
    assert!(ModelManager::is_yaml_file(&yaml_file));

    let txt_file = temp_dir.path().join("test.txt");
    std::fs::write(&txt_file, "text").unwrap();
    assert!(!ModelManager::is_yaml_file(&txt_file));

    // Directory should not count
    assert!(!ModelManager::is_yaml_file(temp_dir.path()));
}

#[test]
fn test_extract_model_name() {
    let path = PathBuf::from("/some/dir/my-model.yaml");
    let name = ModelManager::extract_model_name(&path).unwrap();
    assert_eq!(name, "my-model");
}

#[test]
fn test_check_suspicious_patterns_clean() {
    assert!(ModelManager::check_suspicious_patterns("/normal/path").is_ok());
}

#[test]
fn test_is_valid_directory_nonexistent() {
    assert!(!ModelManager::is_valid_directory(Path::new(
        "/nonexistent/dir"
    )));
}

#[test]
fn test_is_valid_directory_file() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("afile");
    std::fs::write(&file, "content").unwrap();
    assert!(!ModelManager::is_valid_directory(&file));
}

#[test]
fn test_validate_config_file_path_empty() {
    let result = ModelManager::validate_config_file_path(Path::new(""));
    assert!(result.is_err());
}

#[test]
fn test_validate_config_file_path_too_long_coverage() {
    let long_path = "a".repeat(5000);
    let result = ModelManager::validate_config_file_path(Path::new(&long_path));
    assert!(result.is_err());
}

#[test]
fn test_validate_config_file_path_nonexistent() {
    // Exercises the non-existent file path branch (just returns the path).
    let result = ModelManager::validate_config_file_path(Path::new("/tmp/nonexistent_config.yaml"));
    assert!(result.is_ok());
}

#[test]
fn test_validate_config_file_path_existing_file() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("config.yaml");
    std::fs::write(&file, "key: val").unwrap();
    let result = ModelManager::validate_config_file_path(&file);
    assert!(result.is_ok());
}

#[test]
fn test_validate_config_file_path_existing_directory() {
    /// Exercises the branch where an existing path is not a file.
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("subdir");
    std::fs::create_dir(&dir).unwrap();
    let result = ModelManager::validate_config_file_path(&dir);
    assert!(
        result.is_err(),
        "Directory should fail validation as config file"
    );
}

#[test]
fn test_check_directory_writable_valid() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    assert!(ModelManager::check_directory_writable(temp_dir.path()).is_ok());
}

#[test]
fn test_check_directory_writable_file() {
    /// Exercises the branch where path is not a directory.
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("afile");
    std::fs::write(&file, "content").unwrap();
    let result = ModelManager::check_directory_writable(&file);
    assert!(result.is_err());
}

#[test]
fn test_check_directory_writable_nonexistent() {
    let result = ModelManager::check_directory_writable(Path::new("/nonexistent/dir"));
    assert!(result.is_err());
}

#[test]
fn test_check_directory_permissions_not_a_directory() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("afile");
    std::fs::write(&file, "content").unwrap();
    let result = ModelManager::check_directory_permissions(&file);
    assert!(result.is_err());
}

#[test]
fn test_check_directory_permissions_nonexistent() {
    let result = ModelManager::check_directory_permissions(Path::new("/nonexistent/dir"));
    assert!(result.is_err());
}
