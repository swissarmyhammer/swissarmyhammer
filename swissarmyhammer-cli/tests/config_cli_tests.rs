//! CLI integration tests for configuration commands
//!
//! This module tests the CLI configuration management commands including
//! show, variables, test, and env subcommands.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_show_no_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "show"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No sah.toml configuration file found"));
}

#[test]
fn test_config_show_with_file() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create a sample configuration file
    let config_content = concat!(
        "name = \"TestProject\"\n",
        "version = \"1.0.0\"\n",
        "debug = true\n",
        "\n",
        "[database]\n",
        "host = \"localhost\"\n",
        "port = 5432\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "show"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration Variables:"))
        .stdout(predicate::str::contains("name"))
        .stdout(predicate::str::contains("version"))
        .stdout(predicate::str::contains("debug"))
        .stdout(predicate::str::contains("database"));
}

#[test]
fn test_config_show_json_format() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "name = \"JSONTest\"\n",
        "version = \"2.0.0\"\n",
        "\n",
        "[settings]\n",
        "enabled = true\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "show", "--format", "json"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"JSONTest\""))
        .stdout(predicate::str::contains("\"version\": \"2.0.0\""))
        .stdout(predicate::str::contains("\"settings\""));
}

#[test]
fn test_config_show_yaml_format() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "name = \"YAMLTest\"\n",
        "version = \"3.0.0\"\n",
        "\n",
        "[build]\n",
        "optimized = false\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "show", "--format", "yaml"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("name: YAMLTest"))
        .stdout(predicate::str::contains("version: 3.0.0"))
        .stdout(predicate::str::contains("build:"));
}

#[test]
fn test_config_variables_no_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "variables"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No configuration variables available"));
}

#[test]
fn test_config_variables_with_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "project_name = \"VarTest\"\n",
        "author = \"Test Author\"\n",
        "tags = [\"tag1\", \"tag2\"]\n",
        "\n",
        "[metadata]\n",
        "created = \"2023-01-01\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "variables"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Available Variables:"))
        .stdout(predicate::str::contains("project_name"))
        .stdout(predicate::str::contains("author"))
        .stdout(predicate::str::contains("tags"))
        .stdout(predicate::str::contains("metadata"));
}

#[test]
fn test_config_variables_verbose() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "name = \"VerboseTest\"\n",
        "count = 42\n",
        "active = true\n",
        "items = [\"a\", \"b\", \"c\"]\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "variables", "--verbose"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("name"))
        .stdout(predicate::str::contains("string"))
        .stdout(predicate::str::contains("count"))
        .stdout(predicate::str::contains("integer"))
        .stdout(predicate::str::contains("active"))
        .stdout(predicate::str::contains("boolean"))
        .stdout(predicate::str::contains("items"))
        .stdout(predicate::str::contains("array"));
}

#[test]
fn test_config_variables_json_format() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "service = \"TestService\"\n",
        "port = 8080\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "variables", "--format", "json"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"service\""))
        .stdout(predicate::str::contains("\"port\""));
}

#[test]
fn test_config_test_template_from_stdin() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "app_name = \"TemplateTest\"\n",
        "version = \"1.5.0\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = "App: {{ app_name }} v{{ version }}";
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test"])
        .write_stdin(template_content);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("App: TemplateTest v1.5.0"));
}

#[test]
fn test_config_test_template_from_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "service_name = \"FileTemplateTest\"\n",
        "port = 9000\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = "Service {{ service_name }} running on port {{ port }}";
    let template_path = temp_dir.path().join("template.txt");
    fs::write(&template_path, template_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test", template_path.to_str().unwrap()]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Service FileTemplateTest running on port 9000"));
}

#[test]
fn test_config_test_with_variable_overrides() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "name = \"OverrideTest\"\n",
        "env = \"development\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = "{{ name }} in {{ env }} mode";
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test", "--var", "env=production"])
        .write_stdin(template_content);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OverrideTest in production mode"));
}

#[test]
fn test_config_test_debug_mode() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "debug_app = \"DebugTest\"\n",
        "level = \"info\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = "{{ debug_app }}: {{ level }}";
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test", "--debug"])
        .write_stdin(template_content);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Template variables (overrides):"))
        .stdout(predicate::str::contains("Configuration variables:"))
        .stdout(predicate::str::contains("debug_app"))
        .stdout(predicate::str::contains("Template content:"))
        .stdout(predicate::str::contains("Rendered output:"))
        .stdout(predicate::str::contains("DebugTest: info"));
}

#[test]
fn test_config_env_no_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "env"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No configuration file found"));
}

#[test]
fn test_config_env_with_variables() {
    let temp_dir = TempDir::new().unwrap();
    
    // Set up test environment variable
    std::env::set_var("TEST_CONFIG_VAR", "test_value");
    
    let config_content = concat!(
        "name = \"EnvTest\"\n",
        "database_url = \"${TEST_CONFIG_VAR}\"\n",
        "fallback = \"${MISSING_VAR:-default_value}\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "env"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No environment variables found in configuration"));
    
    // Clean up
    std::env::remove_var("TEST_CONFIG_VAR");
}

#[test]
fn test_config_env_missing_only() {
    let temp_dir = TempDir::new().unwrap();
    
    // Set up one test environment variable but not the other
    std::env::set_var("SET_VAR", "present");
    
    let config_content = concat!(
        "name = \"MissingTest\"\n",
        "set_var = \"${SET_VAR}\"\n",
        "missing_var = \"${MISSING_VAR:-default}\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "env", "--missing"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("All environment variables are set"));
    
    // Clean up
    std::env::remove_var("SET_VAR");
}

#[test]
fn test_config_env_json_format() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "name = \"JSONEnvTest\"\n",
        "api_key = \"${API_KEY:-default_key}\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "env", "--format", "json"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[]"));
}

#[test]
fn test_config_invalid_template_syntax() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = "name = \"InvalidTest\"\n";
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let invalid_template = "{{ unclosed tag";
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test"])
        .write_stdin(invalid_template);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Template parsing failed"));
}

#[test]
fn test_config_missing_template_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = "name = \"MissingFileTest\"\n";
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test", "nonexistent.txt"]);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read template file"));
}

#[test]
fn test_config_invalid_variable_format() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = "name = \"InvalidVarTest\"\n";
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = "{{ name }}";
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test", "--var", "invalid_format"])
        .write_stdin(template_content);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid variable format"));
}

#[test]
fn test_config_complex_template_with_nested_access() {
    let temp_dir = TempDir::new().unwrap();
    
    let config_content = concat!(
        "app_name = \"ComplexTest\"\n",
        "features = [\"auth\", \"api\", \"web\"]\n",
        "\n",
        "[database]\n",
        "host = \"localhost\"\n",
        "port = 5432\n",
        "\n",
        "[team]\n",
        "members = [\"Alice\", \"Bob\", \"Carol\"]\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    let template_content = concat!(
        "Application: {{ app_name }}\n",
        "Database: {{ database.host }}:{{ database.port }}\n",
        "Features: {% for feature in features %}{{ feature }}{% unless forloop.last %}, {% endunless %}{% endfor %}\n",
        "Team Size: {{ team.members | size }} members"
    );
    
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(&["config", "test"])
        .write_stdin(template_content);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Application: ComplexTest"))
        .stdout(predicate::str::contains("Database: localhost:5432"))
        .stdout(predicate::str::contains("Features: auth, api, web"))
        .stdout(predicate::str::contains("Team Size: 3 members"));
}