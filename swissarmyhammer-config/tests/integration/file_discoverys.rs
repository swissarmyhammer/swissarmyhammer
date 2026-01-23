//! File discovery tests for the configuration system
//!
//! Tests that configuration files are discovered correctly in the right locations
//! with proper name matching (short and long forms) and precedence handling.

use serde_json::json;
use std::env;
use std::fs;
use std::sync::Mutex;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_config::{ConfigurationDiscovery, TemplateContext};

/// Global mutex to serialize tests that modify global state (current directory, HOME environment variable)
/// This prevents race conditions when multiple tests run in parallel
static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());

/// Test helper for isolated file discovery testing
struct IsolatedDiscoveryTest {
    _env: IsolatedTestEnvironment,
    original_cwd: std::path::PathBuf,
    original_home: Option<String>,
    _lock_guard: std::sync::MutexGuard<'static, ()>,
}

impl IsolatedDiscoveryTest {
    fn new() -> Self {
        // Acquire global lock to prevent race conditions with other tests
        let lock_guard = GLOBAL_STATE_LOCK.lock().unwrap_or_else(|poisoned| {
            // If the lock is poisoned (due to a panic in another test), recover it
            eprintln!("Warning: Global state lock was poisoned, recovering");
            poisoned.into_inner()
        });

        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let original_cwd = std::env::current_dir().expect("Failed to get current dir");
        let original_home = std::env::var("HOME").ok();

        // Set up isolated environment
        let home_dir = env.temp_dir().join("home");
        fs::create_dir(&home_dir).expect("Failed to create home dir");
        std::env::set_var("HOME", &home_dir);
        std::env::set_current_dir(env.temp_dir()).expect("Failed to set current dir");

        Self {
            _env: env,
            original_cwd,
            original_home,
            _lock_guard: lock_guard,
        }
    }

    fn project_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self
            ._env
            .temp_dir()
            .join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }

    fn temp_dir(&self) -> std::path::PathBuf {
        self._env.temp_dir()
    }

    fn home_config_dir(&self) -> std::path::PathBuf {
        let home_path = env::var("HOME").expect("HOME not set");
        let config_dir =
            std::path::Path::new(&home_path).join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&config_dir).expect("Failed to create home config dir");
        config_dir
    }
}

impl Drop for IsolatedDiscoveryTest {
    fn drop(&mut self) {
        // Restore original environment
        let _ = env::set_current_dir(&self.original_cwd);
        if let Some(home) = &self.original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    }
}

#[test]
fn test_short_form_names_discovery() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Test all short form names with different formats
    let test_files = [
        ("sah.toml", r#"short_toml = "found""#),
        ("sah.yaml", "short_yaml: found"),
        ("sah.yml", "short_yml: found"),
        ("sah.json", r#"{"short_json": "found"}"#),
    ];

    // Create each file one at a time and test discovery
    for (filename, content) in &test_files {
        let file_path = project_config_dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write config file");

        let context = TemplateContext::load_for_cli().expect("Failed to load config");

        // Check that the appropriate value was loaded
        match *filename {
            "sah.toml" => assert_eq!(context.get("short_toml"), Some(&json!("found"))),
            "sah.yaml" => assert_eq!(context.get("short_yaml"), Some(&json!("found"))),
            "sah.yml" => assert_eq!(context.get("short_yml"), Some(&json!("found"))),
            "sah.json" => assert_eq!(context.get("short_json"), Some(&json!("found"))),
            _ => panic!("Unexpected filename: {}", filename),
        }

        // Clean up for next test
        fs::remove_file(&file_path).expect("Failed to remove config file");
    }
}

#[test]
fn test_long_form_names_discovery() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Test all long form names with different formats
    let test_files = [
        ("swissarmyhammer.toml", r#"long_toml = "found""#),
        ("swissarmyhammer.yaml", "long_yaml: found"),
        ("swissarmyhammer.yml", "long_yml: found"),
        ("swissarmyhammer.json", r#"{"long_json": "found"}"#),
    ];

    // Create each file one at a time and test discovery
    for (filename, content) in &test_files {
        let file_path = project_config_dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write config file");

        let context = TemplateContext::load_for_cli().expect("Failed to load config");

        // Check that the appropriate value was loaded
        match *filename {
            "swissarmyhammer.toml" => assert_eq!(context.get("long_toml"), Some(&json!("found"))),
            "swissarmyhammer.yaml" => assert_eq!(context.get("long_yaml"), Some(&json!("found"))),
            "swissarmyhammer.yml" => assert_eq!(context.get("long_yml"), Some(&json!("found"))),
            "swissarmyhammer.json" => assert_eq!(context.get("long_json"), Some(&json!("found"))),
            _ => panic!("Unexpected filename: {}", filename),
        }

        // Clean up for next test
        fs::remove_file(&file_path).expect("Failed to remove config file");
    }
}

#[test]
fn test_project_directory_discovery() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Create config file in project directory
    let project_config = r#"
source = "project"
project_specific = true
database_url = "postgresql://localhost/project_db"
"#;

    let config_file = project_config_dir.join("sah.toml");
    fs::write(&config_file, project_config).expect("Failed to write project config");

    let context = TemplateContext::load_for_cli().expect("Failed to load project config");

    // Verify project config was loaded
    assert_eq!(context.get("source"), Some(&json!("project")));
    assert_eq!(context.get("project_specific"), Some(&json!(true)));
    assert_eq!(
        context.get("database_url"),
        Some(&json!("postgresql://localhost/project_db"))
    );
}

#[test]
fn test_home_directory_discovery() {
    let test = IsolatedDiscoveryTest::new();
    let home_config_dir = test.home_config_dir();

    // Create config file in home directory
    let home_config = r#"
source = "home"
global_setting = true
default_database = "postgresql://localhost/global_db"
"#;

    let config_file = home_config_dir.join("sah.toml");
    fs::write(&config_file, home_config).expect("Failed to write home config");

    let context = TemplateContext::load_for_cli().expect("Failed to load home config");

    // Verify home config was loaded
    assert_eq!(context.get("source"), Some(&json!("home")));
    assert_eq!(context.get("global_setting"), Some(&json!(true)));
    assert_eq!(
        context.get("default_database"),
        Some(&json!("postgresql://localhost/global_db"))
    );
}

#[test]
fn test_project_overrides_home_config() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Create home config (global)
    let home_config = r#"
source = "home"
shared_value = "from_home"
home_only = "home_value"
database_url = "postgresql://localhost/global_db"
"#;
    let home_config_file = home_config_dir.join("sah.toml");
    fs::write(&home_config_file, home_config).expect("Failed to write home config");

    // Create project config (should override)
    let project_config = r#"
source = "project"
shared_value = "from_project"
project_only = "project_value"
database_url = "postgresql://localhost/project_db"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    let context = TemplateContext::load_for_cli().expect("Failed to load merged config");

    // Project values should override home values
    assert_eq!(context.get("source"), Some(&json!("project")));
    assert_eq!(context.get("shared_value"), Some(&json!("from_project")));
    assert_eq!(
        context.get("database_url"),
        Some(&json!("postgresql://localhost/project_db"))
    );

    // Both project-only and home-only values should be present
    assert_eq!(context.get("project_only"), Some(&json!("project_value")));
    assert_eq!(context.get("home_only"), Some(&json!("home_value")));
}

#[test]
fn test_missing_directories_graceful_handling() {
    let _test = IsolatedDiscoveryTest::new();

    // Don't create any config directories or files
    // Should succeed with empty context
    let _context =
        TemplateContext::load_for_cli().expect("Should handle missing directories gracefully");

    // Context might be empty or contain defaults - both are acceptable
    // Context should be valid even without config files (just verify it was created successfully)
}

#[test]
fn test_discovery_with_nested_project_structure() {
    let test = IsolatedDiscoveryTest::new();

    // Create nested directory structure
    let nested_dir = test.temp_dir().join("workspace/project/subdir");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested dirs");

    // Create config at the workspace level
    let workspace_config_dir = test
        .temp_dir()
        .join("workspace")
        .join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&workspace_config_dir).expect("Failed to create workspace config dir");

    let workspace_config = r#"
source = "workspace"
workspace_setting = true
"#;
    let workspace_config_file = workspace_config_dir.join("sah.toml");
    fs::write(&workspace_config_file, workspace_config).expect("Failed to write workspace config");

    // Create config at the project level
    let project_config_dir = test
        .temp_dir()
        .join("workspace/project")
        .join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&project_config_dir).expect("Failed to create project config dir");

    let project_config = r#"
source = "project"
project_setting = true
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Change to nested subdirectory
    env::set_current_dir(&nested_dir).expect("Failed to change to nested dir");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config from nested structure");

    // Should find the closest config (project level)
    // The exact behavior depends on the implementation - it might find one or both
    let source = context.get("source").expect("Should find a source");
    let source_str = source.as_str().expect("Source should be string");

    // Should find either workspace or project config (or both merged)
    assert!(
        source_str == "workspace" || source_str == "project",
        "Should find config from workspace or project level, got: {}",
        source_str
    );

    // At least one of the settings should be present
    assert!(
        context.get("workspace_setting").is_some() || context.get("project_setting").is_some(),
        "Should find at least one config setting"
    );
}

#[test]
fn test_file_name_precedence() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Create both short and long form files with different values
    let short_config = r#"
name_type = "short"
value = "from_sah"
"#;
    let short_file = project_config_dir.join("sah.toml");
    fs::write(&short_file, short_config).expect("Failed to write short form config");

    let long_config = r#"
name_type = "long"
value = "from_swissarmyhammer"
"#;
    let long_file = project_config_dir.join("swissarmyhammer.toml");
    fs::write(&long_file, long_config).expect("Failed to write long form config");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with both file types");

    // Should load one of the files (precedence depends on implementation)
    let name_type = context.get("name_type").expect("Should have name_type");
    let value = context.get("value").expect("Should have value");

    let name_type_str = name_type.as_str().expect("name_type should be string");
    let value_str = value.as_str().expect("value should be string");

    // Verify consistency - both values should come from the same file
    match name_type_str {
        "short" => assert_eq!(value_str, "from_sah"),
        "long" => assert_eq!(value_str, "from_swissarmyhammer"),
        _ => panic!("Unexpected name_type: {}", name_type_str),
    }
}

#[test]
fn test_multiple_formats_same_location() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Create multiple format files in same location with different values
    let configs = [
        ("sah.toml", r#"format = "toml""#, "toml"),
        ("sah.yaml", r#"format: yaml"#, "yaml"),
        ("sah.json", r#"{"format": "json"}"#, "json"),
    ];

    for (filename, content, _expected_format) in &configs {
        let file_path = project_config_dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write config file");
    }

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with multiple formats");

    // Should load one of the formats
    let format = context.get("format").expect("Should have format value");
    let format_str = format.as_str().expect("format should be string");

    assert!(
        format_str == "toml" || format_str == "yaml" || format_str == "json",
        "Should load one of the config formats, got: {}",
        format_str
    );
}

#[test]
fn test_discovery_api_directly() {
    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Create test files
    let home_file = home_config_dir.join("sah.toml");
    fs::write(&home_file, r#"source = "home""#).expect("Failed to write home config");

    let project_file = project_config_dir.join("sah.yaml");
    fs::write(&project_file, r#"source: project"#).expect("Failed to write project config");

    // Test discovery API directly
    let discovery = ConfigurationDiscovery::for_cli().expect("Failed to create discovery");
    let discovered_files = discovery.discover_config_files();

    // Should discover both files
    assert!(
        !discovered_files.is_empty(),
        "Should discover at least one config file"
    );

    // Files should exist
    for file_path in &discovered_files {
        assert!(
            file_path.exists(),
            "Discovered file should exist: {:?}",
            file_path
        );
    }

    // Should include both home and project files (order depends on implementation)
    let file_names: Vec<String> = discovered_files
        .iter()
        .filter_map(|p| p.file_name()?.to_str())
        .map(|s| s.to_string())
        .collect();

    assert!(
        file_names.contains(&"sah.toml".to_string())
            || file_names.contains(&"sah.yaml".to_string()),
        "Should discover at least one of the config files, found: {:?}",
        file_names
    );
}

#[test]
fn test_nonexistent_config_directories() {
    let test = IsolatedDiscoveryTest::new();

    // Ensure config directories don't exist
    let project_config_dir = test.temp_dir().join(SwissarmyhammerDirectory::dir_name());
    let home_path = env::var("HOME").expect("HOME not set");
    let home_config_dir =
        std::path::Path::new(&home_path).join(SwissarmyhammerDirectory::dir_name());

    assert!(
        !project_config_dir.exists(),
        "Project config dir should not exist"
    );
    assert!(
        !home_config_dir.exists(),
        "Home config dir should not exist"
    );

    // Should handle gracefully
    let _context =
        TemplateContext::load_for_cli().expect("Should handle missing config directories");

    // Context might be empty or contain defaults
    // Context should be valid (just verify it was created successfully)

    // Discovery should also handle missing directories
    let discovery =
        ConfigurationDiscovery::for_cli().expect("Discovery should handle missing directories");
    let _files = discovery.discover_config_files();

    // Should return empty list or handle gracefully
    // Discovery should handle missing directories gracefully (just verify it was created successfully)
}

#[test]
fn test_permission_denied_directories() {
    use std::os::unix::fs::PermissionsExt;

    let test = IsolatedDiscoveryTest::new();
    let project_config_dir = test.project_config_dir();

    // Create a config file
    let config_content = r#"test = "value""#;
    let config_file = project_config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config file");

    // Make the directory unreadable
    let mut perms = fs::metadata(&project_config_dir)
        .expect("Failed to get dir metadata")
        .permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&project_config_dir, perms).expect("Failed to set dir permissions");

    // Should handle permission errors gracefully
    // Note: This might still succeed depending on the user/system, but should not panic
    let _result = TemplateContext::load_for_cli();

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&project_config_dir)
        .expect("Failed to get dir metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&project_config_dir, perms).expect("Failed to restore dir permissions");

    // Test should complete without panicking
}
