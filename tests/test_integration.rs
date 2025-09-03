use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_command_help() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Test prompts interactively"))
        .stdout(predicate::str::contains("--raw"))
        .stdout(predicate::str::contains("--copy"))
        .stdout(predicate::str::contains("--debug"));
}

#[test]
fn test_command_with_nonexistent_prompt() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test").arg("nonexistent-prompt");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_command_with_invalid_file() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test").arg("-f").arg("nonexistent.md");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read"));
}

#[test]
fn test_command_with_invalid_arguments() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test").arg("help").arg("--var").arg("invalid");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid argument format"));
}

#[test]
fn test_command_with_both_name_and_file() {
    // Create a temporary file
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("test.md");
    fs::write(&temp_file, "# Test\nHello").unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("-f")
        .arg(temp_file.to_str().unwrap());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Cannot specify both"));
}

#[test]
fn test_command_with_neither_name_nor_file() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Must specify either"));
}

#[test]
fn test_command_with_builtin_prompt_non_interactive() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("--var")
        .arg("topic=testing")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("testing"));
}

#[test]
fn test_command_with_simple_file() {
    // Create a temporary file with a simple prompt
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("simple.md");
    fs::write(
        &temp_file,
        r#"---
title: Simple Test
description: A simple test prompt
arguments:
  - name: name
    description: Your name
    required: true
---

Hello, {{name}}! This is a test."#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("-f")
        .arg(temp_file.to_str().unwrap())
        .arg("--var")
        .arg("name=World")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Hello, World!"));
}

#[test]
fn test_command_with_debug_flag() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("--var")
        .arg("topic=debugging")
        .arg("--debug")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Debug Information"))
        .stdout(predicate::str::contains("Prompt Details"))
        .stdout(predicate::str::contains("Template Content"))
        .stdout(predicate::str::contains("Arguments Provided"));
}

#[test]
fn test_command_with_save_flag() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("output.md");

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("--var")
        .arg("topic=save-test")
        .arg("--save")
        .arg(output_file.to_str().unwrap())
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Saved to"));

    // Verify the file was created and contains expected content
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("save-test"));
}

#[test]
fn test_command_with_liquid_features() {
    // Create a temporary file with Liquid template features
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("liquid.md");
    fs::write(
        &temp_file,
        r#"---
title: Liquid Test
description: Test Liquid template features
arguments:
  - name: items
    description: Comma-separated items
    required: true
  - name: prefix
    description: Prefix for each item
    default: "- "
---

{% assign item_list = items | split: "," %}
{% for item in item_list %}
{{ prefix }}{{ item | strip | capitalize }}
{% endfor %}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("-f")
        .arg(temp_file.to_str().unwrap())
        .arg("--var")
        .arg("items=apple,banana,cherry")
        .arg("--var")
        .arg("prefix=* ")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("* Apple"))
        .stdout(predicate::str::contains("* Banana"))
        .stdout(predicate::str::contains("* Cherry"));
}

#[test]
fn test_command_with_environment_variables() {
    // Create a temporary file that uses environment variables
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("env.md");
    fs::write(
        &temp_file,
        r#"---
title: Environment Test
description: Test environment variable access
---

Current user: {{ env.USER | default: "unknown" }}
Test variable: {{ env.TEST_VAR | default: "not_set" }}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.env("TEST_VAR", "test_value");
    cmd.arg("test")
        .arg("-f")
        .arg(temp_file.to_str().unwrap())
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("test_value"));
}

#[test]
fn test_command_with_missing_required_argument() {
    // Create a temporary file with required argument
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("required.md");
    fs::write(
        &temp_file,
        r#"---
title: Required Test
description: Test required argument validation
arguments:
  - name: required_arg
    description: This is required
    required: true
---

Value: {{required_arg}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("-f")
        .arg(temp_file.to_str().unwrap())
        .arg("--raw");

    // In non-interactive mode with missing required args, backward compatibility
    // preserves undefined variables as-is
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Value: {{ required_arg }}"));
}

#[test]
fn test_command_output_formatting() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("--var")
        .arg("topic=formatting");

    // Without --raw, should include formatting
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Rendered Prompt"))
        .stdout(predicate::str::contains("─"));
}

#[test]
fn test_command_raw_output() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("help")
        .arg("--var")
        .arg("topic=formatting")
        .arg("--raw");

    // With --raw, should not include formatting headers
    cmd.assert().success().stdout(
        predicate::str::contains("formatting")
            .and(predicate::str::contains("Rendered Prompt").not()),
    );
}

// Tests for new Liquid template example prompts

#[test]
fn test_array_processor_with_break_continue() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("data/array-processor")
        .arg("--var")
        .arg("items=one,skip_me,two,stop_here,three")
        .arg("--var")
        .arg("skip_pattern=skip")
        .arg("--var")
        .arg("stop_pattern=stop")
        .arg("--var")
        .arg("show_skipped=true")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(". one")) // Will match "1. one"
        .stdout(predicate::str::contains(". two")) // Will match "4. two" (after skipping)
        .stdout(predicate::str::contains(
            "Processing stopped at: \"stop_here\"",
        ))
        .stdout(predicate::str::contains("skip_me (matched pattern:"))
        .stdout(predicate::str::contains("three").not()); // Should not process after stop
}

#[test]
fn test_table_generator_with_cycle() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("formatting/table-generator")
        .arg("--var")
        .arg("headers=Name,Age,City")
        .arg("--var")
        .arg("rows=Alice,25,NYC;Bob,30,LA;Carol,28,Chicago")
        .arg("--var")
        .arg("style=html")
        .arg("--var")
        .arg("zebra=true")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("class=\"even\""))
        .stdout(predicate::str::contains("class=\"odd\""))
        .stdout(predicate::str::contains("<table>"))
        .stdout(predicate::str::contains("Primary Type"))
        .stdout(predicate::str::contains("Secondary Type"));
}

#[test]
fn test_email_composer_with_capture() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("communication/email-composer")
        .arg("--var")
        .arg("recipient_name=John")
        .arg("--var")
        .arg("sender_name=Jane")
        .arg("--var")
        .arg("email_type=welcome")
        .arg("--var")
        .arg("formal=true")
        .arg("--var")
        .arg("time_of_day=morning")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Good morning John,"))
        .stdout(predicate::str::contains("Warmest regards,"))
        .stdout(predicate::str::contains(
            "**Subject:** Welcome to our community, John!",
        ))
        .stdout(predicate::str::contains("Plain Text Version"));
}

#[test]
fn test_statistics_calculator_with_math() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("analysis/statistics-calculator")
        .arg("--var")
        .arg("numbers=5,10,15,20,25")
        .arg("--var")
        .arg("precision=1")
        .arg("--var")
        .arg("show_outliers=false")
        .arg("--var")
        .arg("visualization=false")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("**Count**: 5 values"))
        .stdout(predicate::str::contains("**Range**: 10 to 5")) // Note: sorted as strings
        .stdout(predicate::str::contains("**First Value**: 10"))
        .stdout(predicate::str::contains("**Last Value**: 5"))
        .stdout(predicate::str::contains("**Data Points**: 5"));
}

#[test]
fn test_liquid_backward_compatibility() {
    // Test that old {{variable}} syntax still works
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("old_syntax.md");
    fs::write(
        &temp_file,
        r#"---
title: Old Syntax Test
description: Test backward compatibility
arguments:
  - name: name
    description: Name
    required: false
---

Hello {{name}}, this uses old syntax.
But {{undefined}} should remain as-is."#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("test")
        .arg("-f")
        .arg(temp_file.to_str().unwrap())
        .arg("--var")
        .arg("name=World")
        .arg("--raw");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Hello World, this uses old syntax.",
        ))
        .stdout(predicate::str::contains(
            "But {{ undefined }} should remain as-is.",
        ));
}

#[test]
fn test_validation_excludes_root_level_documentation_files() {
    // Create a temporary directory with files that should be excluded from validation
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create files that should be excluded from validation
    fs::write(
        temp_path.join("README.md"),
        "# Test Project\nThis is a readme file.",
    )
    .unwrap();
    fs::write(
        temp_path.join("INSTALLATION.md"),
        "# Installation\nHow to install.",
    )
    .unwrap();
    fs::write(
        temp_path.join("lint_todo.md"),
        "# Lint Todo\nLinting issues to fix.",
    )
    .unwrap();

    // Create a subdirectory that should be excluded
    let docs_dir = temp_path.join("docs");
    fs::create_dir(&docs_dir).unwrap();
    fs::write(docs_dir.join("api.md"), "# API Docs\nAPI documentation.").unwrap();

    // Create a prompt file that should be validated
    let prompts_dir = temp_path.join("prompts");
    fs::create_dir(&prompts_dir).unwrap();
    fs::write(
        prompts_dir.join("valid-prompt.md"),
        r#"---
title: Valid Prompt
description: A valid prompt for testing
arguments:
  - name: input
    description: Some input
    required: true
---

Hello {{input}}!"#,
    )
    .unwrap();

    // Run validation on the temporary directory
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("validate").arg(temp_path.to_str().unwrap());

    // The validation should succeed and not report errors for excluded files
    cmd.assert().success().stdout(
        predicate::str::contains("README.md")
            .not()
            .and(predicate::str::contains("INSTALLATION.md").not())
            .and(predicate::str::contains("lint_todo.md").not())
            .and(predicate::str::contains("docs/api.md").not()),
    );
}

#[test]
fn test_validation_processes_prompt_files() {
    // Create a temporary directory with a prompt file that has validation errors
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create files that should be excluded (no errors expected)
    fs::write(
        temp_path.join("README.md"),
        "# Test Project\nThis is a readme file.",
    )
    .unwrap();

    // Create a prompt file with validation errors
    let prompts_dir = temp_path.join("prompts");
    fs::create_dir(&prompts_dir).unwrap();
    fs::write(
        prompts_dir.join("invalid-prompt.md"),
        r#"---
title: Invalid Prompt
description: A prompt with validation errors
arguments:
  - name: input
    description: Some input
    # missing required field
---

Hello {{input}}!"#,
    )
    .unwrap();

    // Run validation on the temporary directory
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("validate").arg(temp_path.to_str().unwrap());

    // The validation should fail due to the prompt file error, but not mention README.md
    cmd.assert().failure().stderr(
        predicate::str::contains("invalid-prompt.md")
            .and(predicate::str::contains("missing field `required`"))
            .and(predicate::str::contains("README.md").not()),
    );
}

#[test]
fn test_system_prompt_model_variable() {
    // Test that the .system prompt correctly renders with the model variable set
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("prompt")
        .arg("test")
        .arg(".system")
        .arg("--raw");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("You are powered by the"));
}

#[test]
fn test_system_prompt_model_variable_claude_code() {
    // Test that the .system prompt shows "Claude Code" by default
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("prompt")
        .arg("test")
        .arg(".system")
        .arg("--raw");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("You are powered by the Claude Code model"));
}
