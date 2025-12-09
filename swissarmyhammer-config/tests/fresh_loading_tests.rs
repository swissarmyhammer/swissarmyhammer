//! Fresh loading tests for the configuration system
//!
//! Tests that configuration changes are picked up immediately with no caching behavior.
//! Verifies that TemplateContext always loads fresh config as specified in the requirements.

use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use std::thread;
use std::time::Duration;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::TemplateContext;

/// Test helper for isolated fresh loading testing
struct IsolatedFreshLoadTest {
    _env: IsolatedTestEnvironment,
    original_cwd: std::path::PathBuf,
    env_vars_to_restore: Vec<(String, Option<String>)>,
}

impl IsolatedFreshLoadTest {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let original_cwd = env::current_dir().expect("Failed to get current dir");

        // Set current directory to temp dir for these tests
        env::set_current_dir(env.temp_dir()).expect("Failed to set current dir");

        Self {
            _env: env,
            original_cwd,
            env_vars_to_restore: Vec::new(),
        }
    }

    fn set_env_var(&mut self, key: &str, value: &str) {
        // Store original value for restoration
        let original = env::var(key).ok();
        self.env_vars_to_restore.push((key.to_string(), original));

        env::set_var(key, value);
    }

    fn project_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self._env.temp_dir().join(".swissarmyhammer");
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }

    fn home_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self._env.swissarmyhammer_dir();
        fs::create_dir_all(&config_dir).expect("Failed to create home config dir");
        config_dir
    }
}

impl Drop for IsolatedFreshLoadTest {
    fn drop(&mut self) {
        // Restore environment variables
        for (key, original_value) in &self.env_vars_to_restore {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }

        // Restore original directory - IsolatedTestEnvironment handles HOME restoration
        let _ = env::set_current_dir(&self.original_cwd);
    }
}

#[test]
#[serial]
fn test_config_file_changes_picked_up_immediately() {
    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Create initial config
    let initial_config = r#"
version = "1.0.0"
app_name = "InitialApp"
"#;
    fs::write(&config_file, initial_config).expect("Failed to write initial config");

    // Load config first time
    let context1 = TemplateContext::load_for_cli().expect("Failed to load initial config");
    assert_eq!(context1.get("version"), Some(&json!("1.0.0")));
    assert_eq!(context1.get("app_name"), Some(&json!("InitialApp")));

    // Modify config file
    let updated_config = r#"
version = "2.0.0"
app_name = "UpdatedApp"
new_setting = "added"
"#;
    fs::write(&config_file, updated_config).expect("Failed to write updated config");

    // Load config second time - should pick up changes immediately
    let context2 = TemplateContext::load_for_cli().expect("Failed to load updated config");
    assert_eq!(context2.get("version"), Some(&json!("2.0.0")));
    assert_eq!(context2.get("app_name"), Some(&json!("UpdatedApp")));
    assert_eq!(context2.get("new_setting"), Some(&json!("added")));
}

#[test]
#[serial]
fn test_multiple_successive_loads_always_fresh() {
    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Perform multiple rapid loads with config changes
    for i in 1..=5 {
        let config_content = format!(
            r#"
iteration = {}
timestamp = "load_{}"
counter = {}
"#,
            i,
            i,
            i * 10
        );
        fs::write(&config_file, &config_content).expect("Failed to write config");

        let context = TemplateContext::load_for_cli().expect("Failed to load config");
        assert_eq!(context.get("iteration"), Some(&json!(i)));
        assert_eq!(
            context.get("timestamp"),
            Some(&json!(format!("load_{}", i)))
        );
        assert_eq!(context.get("counter"), Some(&json!(i * 10)));

        // Small delay to ensure file system changes are visible
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[serial]
fn test_environment_variable_changes_picked_up() {
    let mut test = IsolatedFreshLoadTest::new();

    // Set initial environment variable
    test.set_env_var("SAH_DYNAMIC_VALUE", "initial");

    let context1 = TemplateContext::load_for_cli().expect("Failed to load with initial env");
    assert_eq!(context1.get("dynamic.value"), Some(&json!("initial")));

    // Change environment variable
    test.set_env_var("SAH_DYNAMIC_VALUE", "updated");

    let context2 = TemplateContext::load_for_cli().expect("Failed to load with updated env");
    assert_eq!(context2.get("dynamic.value"), Some(&json!("updated")));

    // Add new environment variable
    test.set_env_var("SAH_NEW_VARIABLE", "new_value");

    let context3 = TemplateContext::load_for_cli().expect("Failed to load with new env var");
    assert_eq!(context3.get("dynamic.value"), Some(&json!("updated")));
    assert_eq!(context3.get("new.variable"), Some(&json!("new_value")));
}

#[test]
#[serial]
fn test_config_file_deletion_and_recreation() {
    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Create initial config
    let initial_config = r#"
present = "yes"
file_exists = true
"#;
    fs::write(&config_file, initial_config).expect("Failed to write initial config");

    let context1 = TemplateContext::load_for_cli().expect("Failed to load initial config");
    assert_eq!(context1.get("present"), Some(&json!("yes")));
    assert_eq!(context1.get("file_exists"), Some(&json!(true)));

    // Delete config file
    fs::remove_file(&config_file).expect("Failed to delete config file");

    let context2 = TemplateContext::load_for_cli().expect("Failed to load after deletion");
    // Config values should no longer be present
    assert_eq!(context2.get("present"), None);
    assert_eq!(context2.get("file_exists"), None);

    // Recreate config file with different content
    let new_config = r#"
present = "recreated"
file_exists = true
new_after_recreation = "new_value"
"#;
    fs::write(&config_file, new_config).expect("Failed to recreate config file");

    let context3 = TemplateContext::load_for_cli().expect("Failed to load recreated config");
    assert_eq!(context3.get("present"), Some(&json!("recreated")));
    assert_eq!(context3.get("file_exists"), Some(&json!(true)));
    assert_eq!(
        context3.get("new_after_recreation"),
        Some(&json!("new_value"))
    );
}

#[test]
#[serial]
fn test_multiple_config_files_fresh_loading() {
    let test = IsolatedFreshLoadTest::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Create initial global and project configs
    let global_config = r#"
source = "global"
global_value = "initial_global"
shared = "from_global"
"#;
    let global_file = home_config_dir.join("sah.toml");
    fs::write(&global_file, global_config).expect("Failed to write global config");

    let project_config = r#"
source = "project"
project_value = "initial_project"
shared = "from_project"
"#;
    let project_file = project_config_dir.join("sah.toml");
    fs::write(&project_file, project_config).expect("Failed to write project config");

    let context1 = TemplateContext::load_for_cli().expect("Failed to load initial configs");
    assert_eq!(context1.get("source"), Some(&json!("project"))); // Project should override
    assert_eq!(
        context1.get("project_value"),
        Some(&json!("initial_project"))
    );
    assert_eq!(context1.get("global_value"), Some(&json!("initial_global")));
    assert_eq!(context1.get("shared"), Some(&json!("from_project")));

    // Update global config
    let updated_global = r#"
source = "global"
global_value = "updated_global"
shared = "updated_from_global"
new_global = "new_global_value"
"#;
    fs::write(&global_file, updated_global).expect("Failed to update global config");

    let context2 = TemplateContext::load_for_cli().expect("Failed to load after global update");
    assert_eq!(context2.get("source"), Some(&json!("project"))); // Still project
    assert_eq!(context2.get("global_value"), Some(&json!("updated_global")));
    assert_eq!(context2.get("new_global"), Some(&json!("new_global_value")));
    assert_eq!(context2.get("shared"), Some(&json!("from_project"))); // Project still wins

    // Update project config
    let updated_project = r#"
source = "project"
project_value = "updated_project"
shared = "updated_from_project"
new_project = "new_project_value"
"#;
    fs::write(&project_file, updated_project).expect("Failed to update project config");

    let context3 = TemplateContext::load_for_cli().expect("Failed to load after project update");
    assert_eq!(context3.get("source"), Some(&json!("project")));
    assert_eq!(
        context3.get("project_value"),
        Some(&json!("updated_project"))
    );
    assert_eq!(
        context3.get("new_project"),
        Some(&json!("new_project_value"))
    );
    assert_eq!(context3.get("shared"), Some(&json!("updated_from_project")));

    // Global updates should still be present
    assert_eq!(context3.get("global_value"), Some(&json!("updated_global")));
    assert_eq!(context3.get("new_global"), Some(&json!("new_global_value")));
}

#[test]
#[serial]
fn test_concurrent_fresh_loading() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Create initial config
    let initial_config = r#"
thread_test = "initial"
counter = 0
"#;
    fs::write(&config_file, initial_config).expect("Failed to write initial config");

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // Spawn multiple threads that load config concurrently
    for thread_id in 0..3 {
        let barrier_clone = Arc::clone(&barrier);
        let config_file_clone = config_file.clone();

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            // Update config file with thread-specific content
            let thread_config = format!(
                r#"
thread_test = "thread_{}"
counter = {}
thread_id = {}
"#,
                thread_id,
                thread_id * 10,
                thread_id
            );

            // Write config (there might be race conditions, but that's expected)
            let _ = fs::write(&config_file_clone, &thread_config);

            // Small delay to let file system settle
            thread::sleep(Duration::from_millis(50));

            // Load config - should get fresh data
            let context = TemplateContext::load_for_cli().expect("Failed to load config in thread");

            // Return the loaded values for verification
            (
                context.get("thread_test").cloned(),
                context.get("counter").cloned(),
                context.get("thread_id").cloned(),
            )
        });

        handles.push(handle);
    }

    // Collect results from all threads
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify that each thread got some valid result (not necessarily the same)
    for (i, (thread_test, counter, thread_id)) in results.iter().enumerate() {
        assert!(
            thread_test.is_some(),
            "Thread {} should have loaded thread_test",
            i
        );
        assert!(counter.is_some(), "Thread {} should have loaded counter", i);

        // The values might be from any thread due to race conditions,
        // but they should be consistent within each load
        if let (Some(test_val), Some(counter_val), Some(id_val)) = (thread_test, counter, thread_id)
        {
            if let (Some(test_str), Some(id_num)) = (test_val.as_str(), id_val.as_i64()) {
                assert_eq!(test_str, format!("thread_{}", id_num));
                assert_eq!(counter_val.as_i64(), Some(id_num * 10));
            }
        }
    }
}

#[test]
#[serial]
fn test_config_format_changes_fresh_loading() {
    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();

    // Start with TOML
    let toml_file = config_dir.join("sah.toml");
    let toml_config = r#"
format = "toml"
value = "from_toml"
"#;
    fs::write(&toml_file, toml_config).expect("Failed to write TOML config");

    let context1 = TemplateContext::load_for_cli().expect("Failed to load TOML config");
    assert_eq!(context1.get("format"), Some(&json!("toml")));
    assert_eq!(context1.get("value"), Some(&json!("from_toml")));

    // Remove TOML and add YAML
    fs::remove_file(&toml_file).expect("Failed to remove TOML file");

    let yaml_file = config_dir.join("sah.yaml");
    let yaml_config = r#"
format: yaml
value: from_yaml
new_in_yaml: yaml_specific
"#;
    fs::write(&yaml_file, yaml_config).expect("Failed to write YAML config");

    let context2 = TemplateContext::load_for_cli().expect("Failed to load YAML config");
    assert_eq!(context2.get("format"), Some(&json!("yaml")));
    assert_eq!(context2.get("value"), Some(&json!("from_yaml")));
    assert_eq!(context2.get("new_in_yaml"), Some(&json!("yaml_specific")));

    // Remove YAML and add JSON
    fs::remove_file(&yaml_file).expect("Failed to remove YAML file");

    let json_file = config_dir.join("sah.json");
    let json_config = r#"{
  "format": "json",
  "value": "from_json",
  "new_in_json": "json_specific"
}"#;
    fs::write(&json_file, json_config).expect("Failed to write JSON config");

    let context3 = TemplateContext::load_for_cli().expect("Failed to load JSON config");
    assert_eq!(context3.get("format"), Some(&json!("json")));
    assert_eq!(context3.get("value"), Some(&json!("from_json")));
    assert_eq!(context3.get("new_in_json"), Some(&json!("json_specific")));

    // Previous format-specific values should not be present
    assert_eq!(context3.get("new_in_yaml"), None);
}

#[test]
#[serial]
fn test_no_caching_with_rapid_changes() {
    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Perform very rapid changes to ensure no caching occurs
    let iterations = 20;

    for i in 0..iterations {
        let config_content = format!(
            r#"
rapid_change = {}
timestamp = {}
iteration_mod_5 = {}
"#,
            i,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            i % 5
        );

        fs::write(&config_file, &config_content).expect("Failed to write rapid config");

        let context = TemplateContext::load_for_cli().expect("Failed to load rapid config");

        // Each load should see the exact current value
        assert_eq!(context.get("rapid_change"), Some(&json!(i)));
        assert_eq!(context.get("iteration_mod_5"), Some(&json!(i % 5)));

        // Timestamp should be present (actual value will vary)
        assert!(context.get("timestamp").is_some());

        // Minimal delay to avoid overwhelming the file system
        if i % 5 == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
#[serial]
fn test_fresh_loading_with_file_permissions_changes() {
    use std::os::unix::fs::PermissionsExt;

    let test = IsolatedFreshLoadTest::new();
    let config_dir = test.project_config_dir();
    let config_file = config_dir.join("sah.toml");

    // Create config with readable permissions
    let config_content = r#"
permissions_test = "readable"
access_level = "full"
"#;
    fs::write(&config_file, config_content).expect("Failed to write config");

    let context1 = TemplateContext::load_for_cli().expect("Failed to load readable config");
    assert_eq!(context1.get("permissions_test"), Some(&json!("readable")));
    assert_eq!(context1.get("access_level"), Some(&json!("full")));

    // Make file unreadable
    let mut perms = fs::metadata(&config_file)
        .expect("Failed to get file metadata")
        .permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&config_file, perms).expect("Failed to set file permissions");

    // Should handle permission error gracefully (might succeed or fail depending on user)
    let _result2 = TemplateContext::load_for_cli();

    // Restore readable permissions
    let mut perms = fs::metadata(&config_file)
        .expect("Failed to get file metadata")
        .permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&config_file, perms).expect("Failed to restore file permissions");

    // Update config content
    let updated_content = r#"
permissions_test = "restored"
access_level = "full_again"
"#;
    fs::write(&config_file, updated_content).expect("Failed to write updated config");

    let context3 = TemplateContext::load_for_cli().expect("Failed to load restored config");
    assert_eq!(context3.get("permissions_test"), Some(&json!("restored")));
    assert_eq!(context3.get("access_level"), Some(&json!("full_again")));

    // Should not have any cached values from before permissions issue
    // This verifies that no caching occurred during the permission problem
}
