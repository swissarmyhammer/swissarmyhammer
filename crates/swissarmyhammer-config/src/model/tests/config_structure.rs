//! `ensure_config_structure` against each layout it can meet.
//!
//! An existing YAML configuration, an AVP layout, a read-only parent, and
//! the new path it returns when no configuration exists yet.

use super::*;

// ========================================================================
// validate_agent_name_security tests
// ========================================================================

// ========================================================================
// use_agent security integration tests
// ========================================================================

// ---- Tests for load_or_create_config, save_config, check_file_readable, update_config_with_agent ----

// ========================================================================
// ensure_config_structure additional coverage tests
// ========================================================================

#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_with_existing_yaml_config() {
    // When a YAML config file already exists, ensure_config_structure should
    // detect it and return the canonicalized path to the existing file.
    // Exercises the detect_config_file -> validate_config_file_path -> return
    // existing config branch (lines 1607-1611).
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
    let existing_config = sah_dir.join("sah.yaml");
    fs::write(
        &existing_config,
        "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n",
    )
    .expect("Failed to write existing yaml config");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(
        result.is_ok(),
        "Should detect existing YAML config: {:?}",
        result
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should return existing yaml config file"
    );
    assert!(
        config_path.is_absolute(),
        "Should return canonical absolute path"
    );
    assert!(
        config_path.is_file(),
        "Returned path should point to an existing file"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_with_avp_paths() {
    // Verify ensure_config_structure works with AVP paths (.avp/avp.yaml)
    // to confirm it is not hardcoded to .sah paths. Exercises directory
    // creation (lines 1581-1601) and new config path validation (lines 1615-1623).
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let result = ModelManager::ensure_config_structure(&ModelPaths::avp());
    assert!(
        result.is_ok(),
        "Should successfully create AVP config structure: {:?}",
        result
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("avp.yaml")),
        "Should return path to avp.yaml"
    );
    assert!(
        config_path.ends_with(".avp/avp.yaml"),
        "Should end with .avp/avp.yaml, got: {}",
        config_path.display()
    );

    let avp_dir = temp_dir.path().join(".avp");
    assert!(avp_dir.exists(), "Should create .avp directory");
    assert!(avp_dir.is_dir(), "Should be a directory");
}

#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_avp_with_existing_config() {
    // Exercise the existing-config detection path with AVP paths and a
    // pre-existing YAML config, ensuring validate_config_file_path is called
    // on the detected file (lines 1609-1611).
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let avp_dir = temp_dir.path().join(".avp");
    fs::create_dir_all(&avp_dir).expect("Failed to create .avp dir");
    let existing_config = avp_dir.join("avp.yaml");
    fs::write(
        &existing_config,
        "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: true\n",
    )
    .expect("Failed to write avp config");

    let result = ModelManager::ensure_config_structure(&ModelPaths::avp());
    assert!(
        result.is_ok(),
        "Should handle existing AVP config: {:?}",
        result
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("avp.yaml")),
        "Should return existing avp.yaml"
    );
    assert!(
        config_path.is_file(),
        "Returned path should be an existing file"
    );
}

#[cfg(unix)]
#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_create_dir_fails_readonly_parent() {
    // When the parent directory is read-only, check_directory_writable should
    // fail and ensure_config_structure should propagate the error. Exercises
    // the permission check error path (line 1583).
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    std::fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    // Make the temp directory read-only so .sah cannot be created
    std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o500))
        .expect("Failed to set read-only permissions");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(
        result.is_err(),
        "Should fail when parent directory is read-only"
    );

    // Restore permissions so temp_dir cleanup works
    std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("Failed to restore permissions");
}

#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_returns_new_yaml_path_when_no_config_exists() {
    // When the config directory exists but has no config file,
    // ensure_config_structure should return the path for a new YAML config.
    // Exercises the new config path construction and validation (lines 1615-1623).
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(
        result.is_ok(),
        "Should succeed with empty config directory: {:?}",
        result
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should return path for new sah.yaml"
    );
    // The file should NOT exist yet (ensure_config_structure only returns the path)
    assert!(
        !config_path.exists(),
        "New config file should not be created by ensure_config_structure"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_ensure_config_structure_prefers_yaml_over_toml() {
    // When both .yaml and .toml config files exist, ensure_config_structure
    // should prefer the YAML file since detect_config_file checks YAML first.
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");

    fs::write(
        sah_dir.join("sah.yaml"),
        "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n",
    )
    .expect("Failed to write yaml config");
    fs::write(sah_dir.join("sah.toml"), "[existing]\nvalue = true\n")
        .expect("Failed to write toml config");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(result.is_ok(), "Should succeed: {:?}", result);

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should prefer YAML config over TOML"
    );
}
