//! Performance tests for the configuration system
//!
//! Tests configuration loading performance with large config files,
//! fresh loading overhead, and ensures acceptable performance for typical usage.

use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use std::time::{Duration, Instant};
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

/// Test helper for isolated performance testing
struct IsolatedPerformanceTest {
    temp_dir: TempDir,
    original_cwd: std::path::PathBuf,
    original_home: Option<String>,
    env_vars_to_restore: Vec<(String, Option<String>)>,
}

impl IsolatedPerformanceTest {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_cwd = env::current_dir().expect("Failed to get current dir");
        let original_home = env::var("HOME").ok();

        // Set up isolated environment
        let home_dir = temp_dir.path().join("home");
        fs::create_dir(&home_dir).expect("Failed to create home dir");
        env::set_var("HOME", &home_dir);
        env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        Self {
            temp_dir,
            original_cwd,
            original_home,
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
        let config_dir = self.temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }

    fn home_config_dir(&self) -> std::path::PathBuf {
        let home_path = env::var("HOME").expect("HOME not set");
        let config_dir = std::path::Path::new(&home_path).join(".swissarmyhammer");
        fs::create_dir_all(&config_dir).expect("Failed to create home config dir");
        config_dir
    }
}

impl Drop for IsolatedPerformanceTest {
    fn drop(&mut self) {
        // Restore environment variables
        for (key, original_value) in &self.env_vars_to_restore {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }

        // Restore original environment
        let _ = env::set_current_dir(&self.original_cwd);
        if let Some(home) = &self.original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    }
}

/// Helper function to measure execution time
fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

#[test]
#[serial]
fn test_small_config_loading_performance() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create a small typical config file
    let config_content = r#"
app_name = "SwissArmyHammer"
version = "2.0.0"
debug = true
port = 8080

[database]
host = "localhost"
port = 5432
username = "admin"
password = "secret"

[logging]
level = "info"
file = "/var/log/app.log"

[features]
enabled = ["templating", "workflows", "config"]
"#;

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Measure loading time
    let (context, load_time) =
        measure_time(|| TemplateContext::load_for_cli().expect("Failed to load config"));

    // Performance expectations for small config
    assert!(
        load_time < Duration::from_millis(100),
        "Small config should load in < 100ms, took {:?}",
        load_time
    );

    // Verify config was loaded correctly
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
    assert_eq!(context.get("version"), Some(&json!("2.0.0")));
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));

    println!("Small config load time: {:?}", load_time);
}

#[test]
#[serial]
fn test_large_config_loading_performance() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create a large config file
    let mut config_content =
        String::from("app_name = \"SwissArmyHammer\"\nversion = \"2.0.0\"\n\n");

    // Add many sections and values
    for i in 0..1000 {
        config_content.push_str(&format!("[section_{}]\n", i));
        config_content.push_str(&format!("value_{} = \"test_value_{}\"\n", i, i));
        config_content.push_str(&format!("number_{} = {}\n", i, i * 10));
        config_content.push_str(&format!("boolean_{} = {}\n", i, i % 2 == 0));
        config_content.push('\n');
    }

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, &config_content).expect("Failed to write large config");

    // Measure loading time for large config
    let (context, load_time) =
        measure_time(|| TemplateContext::load_for_cli().expect("Failed to load large config"));

    // Performance expectations for large config (should still be reasonable)
    assert!(
        load_time < Duration::from_secs(2),
        "Large config should load in < 2s, took {:?}",
        load_time
    );

    // Verify some values were loaded correctly
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));

    // Check that large config was parsed correctly
    if let Some(section_0) = context.get("section_0") {
        if let Some(section_obj) = section_0.as_object() {
            assert_eq!(section_obj.get("value_0"), Some(&json!("test_value_0")));
            assert_eq!(section_obj.get("number_0"), Some(&json!(0)));
            assert_eq!(section_obj.get("boolean_0"), Some(&json!(true)));
        }
    }

    println!("Large config load time: {:?} (1000 sections)", load_time);
}

#[test]
#[serial]
fn test_multiple_config_files_performance() {
    let test = IsolatedPerformanceTest::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Create multiple config files that need to be merged

    // Global config
    let global_config = r#"
global_setting = "global_value"
shared_value = "from_global"

[global_section]
value1 = "global1"
value2 = "global2"
"#;
    let global_file = home_config_dir.join("sah.toml");
    fs::write(&global_file, global_config).expect("Failed to write global config");

    // Project config (TOML)
    let project_toml = r#"
project_setting = "project_value"
shared_value = "from_project"

[project_section]
value1 = "project1"
value2 = "project2"
"#;
    let project_toml_file = project_config_dir.join("sah.toml");
    fs::write(&project_toml_file, project_toml).expect("Failed to write project TOML");

    // Project config (YAML)
    let project_yaml = r#"
yaml_setting: yaml_value
nested:
  value1: yaml1
  value2: yaml2
"#;
    let project_yaml_file = project_config_dir.join("swissarmyhammer.yaml");
    fs::write(&project_yaml_file, project_yaml).expect("Failed to write project YAML");

    // Project config (JSON)
    let project_json = r#"{
  "json_setting": "json_value",
  "json_nested": {
    "value1": "json1",
    "value2": "json2"
  }
}"#;
    let project_json_file = project_config_dir.join("swissarmyhammer.json");
    fs::write(&project_json_file, project_json).expect("Failed to write project JSON");

    // Measure loading time with multiple files
    let (context, load_time) =
        measure_time(|| TemplateContext::load_for_cli().expect("Failed to load multiple configs"));

    // Should handle multiple files reasonably quickly
    assert!(
        load_time < Duration::from_millis(500),
        "Multiple config files should load in < 500ms, took {:?}",
        load_time
    );

    // Verify configs were merged correctly
    assert_eq!(context.get("global_setting"), Some(&json!("global_value")));
    assert_eq!(
        context.get("project_setting"),
        Some(&json!("project_value"))
    );

    // Project should override global for shared values
    assert_eq!(context.get("shared_value"), Some(&json!("from_project")));

    println!("Multiple config files load time: {:?}", load_time);
}

#[test]
#[serial]
#[ignore] // Disabled due to test isolation issues - passes individually but fails in test suite
fn test_fresh_loading_performance_overhead() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create a medium-sized config
    let config_content = r#"
app_name = "SwissArmyHammer"
version = "2.0.0"

[database]
host = "localhost"
port = 5432

[server]
host = "0.0.0.0"
port = 8080
workers = 4
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Measure multiple consecutive loads (fresh loading)
    let iterations = 100;
    let mut load_times = Vec::new();

    for i in 0..iterations {
        let (context, load_time) =
            measure_time(|| TemplateContext::load_for_cli().expect("Failed to load config"));

        load_times.push(load_time);

        // Verify consistency
        assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));

        // Small delay to avoid overwhelming file system
        if i % 10 == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // Calculate statistics
    let total_time: Duration = load_times.iter().sum();
    let avg_time = total_time / iterations as u32;
    let min_time = *load_times.iter().min().unwrap();
    let max_time = *load_times.iter().max().unwrap();

    // Performance expectations for fresh loading
    assert!(
        avg_time < Duration::from_millis(50),
        "Average fresh load time should be < 50ms, got {:?}",
        avg_time
    );

    assert!(
        max_time < Duration::from_millis(200),
        "Maximum fresh load time should be < 200ms, got {:?}",
        max_time
    );

    // Verify no significant performance degradation over time
    let first_10_avg: Duration = load_times[0..10].iter().sum::<Duration>() / 10;
    let last_10_avg: Duration = load_times[90..100].iter().sum::<Duration>() / 10;

    assert!(
        last_10_avg < first_10_avg * 3,
        "Performance should not degrade significantly: first 10 avg {:?}, last 10 avg {:?}",
        first_10_avg,
        last_10_avg
    );

    println!(
        "Fresh loading performance: avg {:?}, min {:?}, max {:?} ({} iterations)",
        avg_time, min_time, max_time, iterations
    );
}

#[test]
#[serial]
fn test_environment_variable_processing_performance() {
    let mut test = IsolatedPerformanceTest::new();

    // Set many environment variables
    for i in 0..1000 {
        test.set_env_var(&format!("SAH_VAR_{}", i), &format!("value_{}", i));
    }

    // Also set some SWISSARMYHAMMER_ variables
    for i in 0..100 {
        test.set_env_var(
            &format!("SWISSARMYHAMMER_LONG_VAR_{}", i),
            &format!("long_value_{}", i),
        );
    }

    // Measure loading time with many environment variables
    let (context, load_time) = measure_time(|| {
        TemplateContext::load_for_cli().expect("Failed to load config with many env vars")
    });

    // Should handle many environment variables reasonably
    assert!(
        load_time < Duration::from_secs(1),
        "Many environment variables should load in < 1s, took {:?}",
        load_time
    );

    // Verify some environment variables were processed
    assert_eq!(context.get("var.0"), Some(&json!("value_0")));
    assert_eq!(context.get("var.999"), Some(&json!("value_999")));
    assert_eq!(context.get("long.var.0"), Some(&json!("long_value_0")));

    println!(
        "Environment variable processing time: {:?} (1100 vars)",
        load_time
    );
}

#[test]
#[serial]
fn test_template_context_conversion_performance() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create config with complex nested structure
    let config_content = r#"
app_name = "SwissArmyHammer"

[database]
host = "localhost"
port = 5432
pools = [
    { name = "read", size = 10 },
    { name = "write", size = 5 },
    { name = "admin", size = 2 }
]

[servers]
web = { host = "web1", port = 8080, workers = 4 }
api = { host = "api1", port = 9000, workers = 8 }
admin = { host = "admin1", port = 9090, workers = 2 }

[features]
enabled = ["templating", "workflows", "config", "metrics", "logging"]
experimental = ["ai", "advanced_metrics", "real_time", "clustering"]

[logging]
level = "info"
targets = ["console", "file", "syslog"]
formats = { console = "pretty", file = "json", syslog = "structured" }
"#;

    let config_file = config_dir.join("complex.toml");
    fs::write(&config_file, config_content).expect("Failed to write complex config");

    let context = TemplateContext::load_for_cli().expect("Failed to load complex config");

    // Measure liquid context conversion performance
    let iterations = 1000;
    let (_, total_conversion_time) = measure_time(|| {
        for _ in 0..iterations {
            let _liquid_context = context.to_liquid_context();
        }
    });

    let avg_conversion_time = total_conversion_time / iterations as u32;

    // Conversion should be fast
    assert!(
        avg_conversion_time < Duration::from_millis(10),
        "Template context conversion should be < 10ms, got {:?}",
        avg_conversion_time
    );

    println!(
        "Template context conversion time: {:?} average ({} iterations)",
        avg_conversion_time, iterations
    );
}

#[test]
#[serial]
fn test_config_with_cli_args_performance() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create base config
    let config_content = r#"
app_name = "SwissArmyHammer"
version = "2.0.0"
debug = false

[database]
host = "localhost"
port = 5432

[server]
port = 8080
workers = 4
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Create substantial CLI args
    let cli_args = json!({
        "app_name": "CliApp",
        "version": "3.0.0",
        "debug": true,
        "database": {
            "host": "cli-host",
            "port": 3306,
            "ssl": true
        },
        "server": {
            "port": 9090,
            "workers": 8,
            "timeout": 30
        },
        "new_section": {
            "value1": "cli_value1",
            "value2": "cli_value2",
            "nested": {
                "deep_value": "deep_cli_value"
            }
        }
    });

    // Measure loading with CLI args
    let (context, load_time) = measure_time(|| {
        TemplateContext::load_with_cli_args(cli_args.clone())
            .expect("Failed to load config with CLI args")
    });

    // Should handle CLI args efficiently
    assert!(
        load_time < Duration::from_millis(100),
        "Config with CLI args should load in < 100ms, took {:?}",
        load_time
    );

    // Verify CLI args override config
    assert_eq!(context.get("app_name"), Some(&json!("CliApp")));
    assert_eq!(context.get("version"), Some(&json!("3.0.0")));
    assert_eq!(context.get("debug"), Some(&json!(true)));
    assert_eq!(context.get("database.host"), Some(&json!("cli-host")));
    assert_eq!(context.get("server.port"), Some(&json!(9090)));

    println!("Config with CLI args load time: {:?}", load_time);
}

#[test]
#[serial]
#[ignore] // Disabled due to thread isolation issues with current_dir changes
fn test_concurrent_config_loading_performance() {
    use std::sync::Arc;
    use std::thread;

    let test = Arc::new(IsolatedPerformanceTest::new());
    let config_dir = test.project_config_dir();

    // Create config file
    let config_content = r#"
app_name = "SwissArmyHammer"
version = "2.0.0"
concurrent_test = true

[section1]
value1 = "test1"
value2 = "test2"

[section2]
value1 = "test3"
value2 = "test4"
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    let num_threads = 10;
    let iterations_per_thread = 50;
    let mut handles = vec![];

    let start_time = Instant::now();

    // Spawn multiple threads that load config concurrently
    for thread_id in 0..num_threads {
        let temp_dir_path = test.temp_dir.path().to_path_buf();
        let handle = thread::spawn(move || {
            // Set the working directory in the thread context
            env::set_current_dir(&temp_dir_path).expect("Failed to set working dir in thread");

            let mut thread_load_times = Vec::new();

            for _ in 0..iterations_per_thread {
                let (context, load_time) = measure_time(|| {
                    TemplateContext::load_for_cli().expect("Failed to load config in thread")
                });

                thread_load_times.push(load_time);

                // Verify correct loading
                assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
                assert_eq!(context.get("concurrent_test"), Some(&json!(true)));
            }

            (thread_id, thread_load_times)
        });

        handles.push(handle);
    }

    // Collect results from all threads
    let mut all_load_times = Vec::new();
    for handle in handles {
        let (thread_id, load_times) = handle.join().expect("Thread panicked");
        all_load_times.extend(load_times);

        println!(
            "Thread {} completed {} loads",
            thread_id, iterations_per_thread
        );
    }

    let total_time = start_time.elapsed();
    let total_loads = num_threads * iterations_per_thread;

    // Calculate statistics
    let avg_load_time: Duration =
        all_load_times.iter().sum::<Duration>() / all_load_times.len() as u32;
    let min_load_time = *all_load_times.iter().min().unwrap();
    let max_load_time = *all_load_times.iter().max().unwrap();

    // Performance expectations for concurrent loading
    assert!(
        avg_load_time < Duration::from_millis(100),
        "Average concurrent load time should be < 100ms, got {:?}",
        avg_load_time
    );

    assert!(
        max_load_time < Duration::from_millis(500),
        "Maximum concurrent load time should be < 500ms, got {:?}",
        max_load_time
    );

    let throughput = total_loads as f64 / total_time.as_secs_f64();

    println!(
        "Concurrent loading: {} threads, {} loads each, {:?} total time",
        num_threads, iterations_per_thread, total_time
    );
    println!(
        "Stats: avg {:?}, min {:?}, max {:?}, throughput {:.1} loads/sec",
        avg_load_time, min_load_time, max_load_time, throughput
    );

    // Should achieve reasonable throughput
    assert!(
        throughput > 100.0,
        "Should achieve > 100 loads/sec concurrently, got {:.1}",
        throughput
    );
}

#[test]
#[serial]
#[ignore] // Disabled due to test isolation issues - passes individually but fails in test suite
fn test_memory_usage_characteristics() {
    let test = IsolatedPerformanceTest::new();
    let config_dir = test.project_config_dir();

    // Create a moderately large config to test memory usage efficiently
    let mut large_config = String::from("app_name = \"MemoryTest\"\n");

    // Add enough string values to test memory patterns without being excessive
    for i in 0..1000 {
        large_config.push_str(&format!(
            "key_{} = \"value_{}_with_some_longer_content_to_test_memory_usage\"\n",
            i, i
        ));
    }

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, &large_config).expect("Failed to write memory test config");

    // Load config and create liquid contexts multiple times - reduced for speed
    let iterations = 25;
    let (contexts, total_time) = measure_time(|| {
        let mut contexts = Vec::new();
        for _ in 0..iterations {
            let context = TemplateContext::load_for_cli().expect("Failed to load config");
            let _liquid_context = context.to_liquid_context();
            contexts.push(context);
        }
        contexts
    });

    let avg_time = total_time / iterations as u32;

    // Should complete without excessive memory usage or time (relaxed for smaller test)
    assert!(
        avg_time < Duration::from_millis(150),
        "Average load time should be < 150ms even with large config, got {:?}",
        avg_time
    );

    // Verify contexts are functional
    assert_eq!(contexts.len(), iterations);
    for context in &contexts[0..5] {
        // Check first 5 (fewer iterations)
        assert_eq!(context.get("app_name"), Some(&json!("MemoryTest")));
        assert_eq!(
            context.get("key_0"),
            Some(&json!(
                "value_0_with_some_longer_content_to_test_memory_usage"
            ))
        );
    }

    println!(
        "Memory test: {} contexts, avg time {:?}, total time {:?}",
        iterations, avg_time, total_time
    );

    // Test should complete without running out of memory or taking excessive time
}
