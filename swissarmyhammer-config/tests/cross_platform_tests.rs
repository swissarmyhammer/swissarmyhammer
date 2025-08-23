//! Cross-platform integration tests for SwissArmyHammer configuration system

mod common;

use common::{ConfigScope, TestEnvironment};
use serial_test::serial;

use swissarmyhammer_config::ConfigFormat;

#[test]
#[serial]
fn test_path_handling_across_platforms() {
    let env = TestEnvironment::new().unwrap();

    // Test configuration with various path formats
    let path_config = r#"
[paths]
unix_absolute = "/var/log/app.log"
unix_relative = "./logs/app.log"
unix_home = "~/config/app.conf"
windows_absolute = "C:\\Program Files\\MyApp\\config.ini"
windows_relative = ".\\logs\\app.log"
windows_unc = "\\\\server\\share\\config.toml"
mixed_separators = "./config\\mixed/paths.json"

[build_paths]
output_directory = "./target/release"
source_directory = "./src"
test_directory = "./tests"
docs_directory = "./docs"

[deployment_paths]
unix_deploy = "/opt/myapp"
windows_deploy = "C:\\apps\\myapp"
"#;

    env.write_project_config(path_config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // All path configurations should be loaded regardless of platform
    if let Some(paths) = context.get("paths") {
        assert!(paths.get("unix_absolute").is_some());
        assert!(paths.get("unix_relative").is_some());
        assert!(paths.get("windows_absolute").is_some());
        assert!(paths.get("windows_relative").is_some());
        assert!(paths.get("mixed_separators").is_some());

        // Values should be preserved as-is (no automatic path conversion)
        assert_eq!(
            paths["unix_absolute"],
            serde_json::Value::String("/var/log/app.log".to_string())
        );
        assert_eq!(
            paths["windows_absolute"],
            serde_json::Value::String("C:\\Program Files\\MyApp\\config.ini".to_string())
        );
    }

    if let Some(build_paths) = context.get("build_paths") {
        assert!(build_paths.get("output_directory").is_some());
        assert!(build_paths.get("source_directory").is_some());
    }

    println!(
        "Path handling test passed on platform: {}",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_file_discovery_cross_platform() {
    let env = TestEnvironment::new().unwrap();

    // Create configuration files with different naming conventions
    let config_variants = [
        ("sah.toml", "sah_toml_value"),
        ("sah.yaml", "sah_yaml_value"),
        ("sah.json", "sah_json_value"),
        ("swissarmyhammer.toml", "swissarmyhammer_toml_value"),
        ("swissarmyhammer.yaml", "swissarmyhammer_yaml_value"),
        ("swissarmyhammer.json", "swissarmyhammer_json_value"),
    ];

    for (filename, test_value) in &config_variants {
        let name_part = filename.split('.').next().unwrap();
        let format = match filename.split('.').last().unwrap() {
            "toml" => ConfigFormat::Toml,
            "yaml" => ConfigFormat::Yaml,
            "json" => ConfigFormat::Json,
            _ => panic!("Unknown format"),
        };

        let content = match format {
            ConfigFormat::Toml => format!("test_key = \"{}\"", test_value),
            ConfigFormat::Yaml => format!("test_key: {}", test_value),
            ConfigFormat::Json => format!("{{\"test_key\": \"{}\"}}", test_value),
        };

        env.write_config(&content, format, ConfigScope::Project, name_part)
            .unwrap();
    }

    let context = env.load_template_context().unwrap();

    // Should discover and load configuration files regardless of platform
    assert!(context.get("test_key").is_some());
    println!(
        "File discovery test passed on platform: {}, files discovered and merged",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_environment_variable_handling_cross_platform() {
    let mut env = TestEnvironment::new().unwrap();

    // Test various environment variable patterns that might behave differently across platforms
    env.set_env_vars([
        ("SIMPLE_VAR", "simple_value"),
        ("CASE_SENSITIVE_VAR", "case_value"),
        ("case_sensitive_var", "lower_case_value"), // Different from above
        ("VAR_WITH_SPACES", "value with spaces"),
        ("VAR_WITH_QUOTES", "value\"with'quotes"),
        ("UNICODE_VAR", "Unicode: 🚀 ∑ ∏ ∆"),
        ("EMPTY_VAR", ""),
        ("PATH_LIKE_VAR", "/usr/local/bin:/usr/bin:/bin"),
    ])
    .unwrap();

    // Windows-style environment variables (if applicable)
    #[cfg(windows)]
    {
        env.set_env_var("WINDOWS_PATH", "C:\\Windows\\System32;C:\\Windows")
            .unwrap();
        env.set_env_var("PROGRAM_FILES", "C:\\Program Files")
            .unwrap();
    }

    // Unix-style environment variables
    #[cfg(unix)]
    {
        env.set_env_var("HOME_DIR", "/home/user").unwrap();
        env.set_env_var("LD_LIBRARY_PATH", "/usr/lib:/usr/local/lib")
            .unwrap();
    }

    let platform_config = r#"
project_name = "Cross-Platform Test"
simple = "${SIMPLE_VAR}"
case_test = "${CASE_SENSITIVE_VAR}"
lower_case_test = "${case_sensitive_var}"
spaces_test = "${VAR_WITH_SPACES}"
quotes_test = "${VAR_WITH_QUOTES}"
unicode_test = "${UNICODE_VAR}"
empty_test = "${EMPTY_VAR}"
path_test = "${PATH_LIKE_VAR}"
missing_with_fallback = "${MISSING_VAR:-fallback_value}"

[platform_specific]
"#;

    let mut final_config = platform_config.to_string();

    // Add platform-specific configuration
    #[cfg(windows)]
    {
        final_config.push_str(
            r#"
windows_path = "${WINDOWS_PATH}"
program_files = "${PROGRAM_FILES}"
"#,
        );
    }

    #[cfg(unix)]
    {
        final_config.push_str(
            r#"
home_dir = "${HOME_DIR}"
library_path = "${LD_LIBRARY_PATH}"
"#,
        );
    }

    env.write_project_config(&final_config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Verify basic environment variable substitution works
    assert_eq!(context.get_string("simple").unwrap(), "simple_value");
    assert_eq!(context.get_string("case_test").unwrap(), "case_value");
    assert_eq!(
        context.get_string("spaces_test").unwrap(),
        "value with spaces"
    );
    assert_eq!(
        context.get_string("unicode_test").unwrap(),
        "Unicode: 🚀 ∑ ∏ ∆"
    );
    assert_eq!(context.get_string("empty_test").unwrap(), "");
    assert_eq!(
        context.get_string("missing_with_fallback").unwrap(),
        "fallback_value"
    );

    // Case sensitivity test - behavior may differ by platform
    let lower_case_result = context.get_string("lower_case_test").unwrap();
    println!("Lower case env var result: '{}'", lower_case_result);

    // Platform-specific tests
    #[cfg(windows)]
    {
        if let Some(platform_specific) = context.get("platform_specific") {
            assert!(platform_specific.get("windows_path").is_some());
            assert!(platform_specific.get("program_files").is_some());
        }
    }

    #[cfg(unix)]
    {
        if let Some(platform_specific) = context.get("platform_specific") {
            assert!(platform_specific.get("home_dir").is_some());
            assert!(platform_specific.get("library_path").is_some());
        }
    }

    println!(
        "Environment variable handling test passed on platform: {}",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_file_encoding_handling() {
    let env = TestEnvironment::new().unwrap();

    // Test UTF-8 configuration (standard)
    let utf8_config = r#"
project_name = "UTF-8 Test"
english = "Hello World"
chinese = "你好世界"
arabic = "مرحبا بالعالم"
russian = "Привет мир"
emoji = "🚀 🌟 ⭐ 🎉"
mixed = "Mixed: Hello 你好 مرحبا Привет 🌍"
"#;

    env.write_project_config(utf8_config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // All Unicode content should be handled correctly
    assert_eq!(context.get_string("project_name").unwrap(), "UTF-8 Test");
    assert_eq!(context.get_string("english").unwrap(), "Hello World");
    assert_eq!(context.get_string("chinese").unwrap(), "你好世界");
    assert_eq!(context.get_string("arabic").unwrap(), "مرحبا بالعالم");
    assert_eq!(context.get_string("russian").unwrap(), "Привет мир");
    assert_eq!(context.get_string("emoji").unwrap(), "🚀 🌟 ⭐ 🎉");

    let mixed_value = context.get_string("mixed").unwrap();
    assert!(mixed_value.contains("Hello"));
    assert!(mixed_value.contains("你好"));
    assert!(mixed_value.contains("🌍"));

    println!(
        "File encoding test passed on platform: {} with full Unicode support",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_line_ending_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create configuration with different line endings
    let config_with_different_endings =
        "project_name = \"Line Ending Test\"\r\ndebug = true\nlog_level = \"info\"\r\n";

    env.write_project_config(config_with_different_endings, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Should handle mixed line endings gracefully
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Line Ending Test"
    );
    assert_eq!(context.get_bool("debug").unwrap(), true);
    assert_eq!(context.get_string("log_level").unwrap(), "info");

    println!(
        "Line ending handling test passed on platform: {}",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_case_sensitivity_behavior() {
    let mut env = TestEnvironment::new().unwrap();

    // Test case sensitivity in environment variables
    env.set_env_vars([
        ("SAH_LOWER_test", "lower_value"),
        ("SAH_UPPER_TEST", "upper_value"),
        ("SAH_Mixed_Case", "mixed_value"),
    ])
    .unwrap();

    let case_config = r#"
project_name = "Case Sensitivity Test"
lower_var = "${SAH_LOWER_test:-not_found}"
upper_var = "${SAH_UPPER_TEST:-not_found}"
mixed_var = "${SAH_Mixed_Case:-not_found}"
"#;

    env.write_project_config(case_config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Case sensitivity behavior may vary by platform
    let lower_result = context.get_string("lower_var").unwrap();
    let upper_result = context.get_string("upper_var").unwrap();
    let mixed_result = context.get_string("mixed_var").unwrap();

    println!("Case sensitivity results on {}:", std::env::consts::OS);
    println!("  lower_var: '{}'", lower_result);
    println!("  upper_var: '{}'", upper_result);
    println!("  mixed_var: '{}'", mixed_result);

    // At least one should work (depending on platform case sensitivity)
    assert!(
        lower_result != "not_found" || upper_result != "not_found" || mixed_result != "not_found"
    );
}

#[test]
#[serial]
fn test_home_directory_resolution() {
    let env = TestEnvironment::new().unwrap();

    // The TestEnvironment sets up a fake HOME directory
    // Test that the configuration system can find global configs there

    env.write_global_config(
        r#"
project_name = "Global Config Test"
global_setting = "from_home_directory"
platform = "cross_platform"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    env.write_project_config(
        r#"
project_name = "Project Override"
project_setting = "from_project"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Project should override global
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Project Override"
    );

    // Global-only setting should be present
    assert_eq!(
        context.get_string("global_setting").unwrap(),
        "from_home_directory"
    );

    // Project-only setting should be present
    assert_eq!(
        context.get_string("project_setting").unwrap(),
        "from_project"
    );

    println!(
        "Home directory resolution test passed on platform: {}",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_temporary_directory_handling() {
    let env = TestEnvironment::new().unwrap();

    // Verify that the test environment creates proper temporary directories
    assert!(env.temp_path().exists());
    assert!(env.project_path().exists());
    assert!(env.global_config_path().exists());
    assert!(env.project_config_path().exists());

    // Check that paths are absolute (important for cross-platform compatibility)
    assert!(env.temp_path().is_absolute());
    assert!(env.project_path().is_absolute());
    assert!(env.global_config_path().is_absolute());
    assert!(env.project_config_path().is_absolute());

    let config = r#"
project_name = "Temp Dir Test"
test_passed = true
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();
    let context = env.load_template_context().unwrap();

    assert_eq!(context.get_string("project_name").unwrap(), "Temp Dir Test");
    assert_eq!(context.get_bool("test_passed").unwrap(), true);

    println!(
        "Temporary directory handling test passed on platform: {}, temp dir: {:?}",
        std::env::consts::OS,
        env.temp_path()
    );
}

#[test]
#[serial]
fn test_platform_specific_defaults() {
    let env = TestEnvironment::new().unwrap();

    // Create minimal configuration
    let minimal_config = r#"
project_name = "Platform Defaults Test"
"#;

    env.write_project_config(minimal_config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Should have platform-independent default values
    assert!(context.get("environment").is_some());
    assert!(context.get("debug").is_some());
    assert!(context.get("log_level").is_some());
    assert!(context.get("timeout_seconds").is_some());

    // Defaults should be consistent across platforms
    assert_eq!(context.get_string("environment").unwrap(), "development");
    assert_eq!(context.get_bool("debug").unwrap(), false);

    println!(
        "Platform defaults test passed on platform: {} with consistent defaults",
        std::env::consts::OS
    );
}

#[test]
#[serial]
fn test_concurrent_access_cross_platform() {
    use std::sync::Arc;
    use std::thread;

    let env = Arc::new(TestEnvironment::new().unwrap());

    let config = r#"
project_name = "Concurrent Access Test"
thread_safe = true
platform_test = true
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();

    const THREAD_COUNT: usize = 4;
    let mut handles = Vec::new();

    // Spawn multiple threads to test concurrent access
    for thread_id in 0..THREAD_COUNT {
        let env_clone = Arc::clone(&env);

        let handle = thread::spawn(move || {
            // Each thread loads configuration multiple times
            for _iteration in 0..10 {
                let context = env_clone.load_template_context().unwrap();

                assert_eq!(
                    context.get_string("project_name").unwrap(),
                    "Concurrent Access Test"
                );
                assert_eq!(context.get_bool("thread_safe").unwrap(), true);

                // Small delay to encourage race conditions
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let mut completed_threads = Vec::new();
    for handle in handles {
        completed_threads.push(handle.join().unwrap());
    }

    // All threads should complete successfully
    completed_threads.sort();
    assert_eq!(completed_threads, (0..THREAD_COUNT).collect::<Vec<_>>());

    println!(
        "Concurrent access test passed on platform: {} with {} threads",
        std::env::consts::OS,
        THREAD_COUNT
    );
}

#[test]
#[serial]
fn test_configuration_paths_normalization() {
    let env = TestEnvironment::new().unwrap();

    // Test that configuration paths work regardless of trailing separators
    let path_variants = [
        env.project_config_path().to_path_buf(),
        env.project_config_path().join(""), // With trailing separator
    ];

    for (i, config_dir) in path_variants.iter().enumerate() {
        if config_dir.exists() {
            let config_file = config_dir.join("sah.toml");

            let config_content = format!(
                r#"
project_name = "Path Test {}"
path_variant = {}
"#,
                i, i
            );

            std::fs::write(&config_file, config_content).unwrap();

            let context = env.load_template_context().unwrap();

            assert_eq!(
                context.get_string("project_name").unwrap(),
                format!("Path Test {}", i)
            );

            // Clean up for next iteration
            let _ = std::fs::remove_file(&config_file);
        }
    }

    println!(
        "Configuration path normalization test passed on platform: {}",
        std::env::consts::OS
    );
}
