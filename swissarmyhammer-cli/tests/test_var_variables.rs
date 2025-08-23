//! Tests for --var variable functionality in workflows

use anyhow::Result;
use predicates::prelude::*;
use std::fs;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::{cli::FlowSubcommand, flow::run_flow_command};

mod in_process_test_utils;
mod test_utils;

use in_process_test_utils::run_sah_command_in_process;

/// Run flow command with variables in-process
async fn run_workflow_with_vars_in_process(
    workflow_name: &str,
    vars: Vec<String>,
    dry_run: bool,
) -> Result<bool> {
    let subcommand = FlowSubcommand::Run {
        workflow: workflow_name.to_string(),
        vars,
        interactive: false,
        dry_run,
        test: false,
        timeout: Some("2s".to_string()), // Use 2 second timeout for fast tests
        quiet: true,
    };

    let result = run_flow_command(subcommand).await;

    Ok(result.is_ok())
}

/// Sync wrapper for CLI commands that returns an assert_cmd::Command-compatible interface
/// This allows existing .assert() patterns to work with in-process execution
fn run_sah_sync_assert(args: &[&str]) -> TestCommandResult {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(run_sah_command_in_process(args)).unwrap();
    TestCommandResult {
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
    }
}

/// Simple result wrapper that provides assert methods like assert_cmd::Assert
struct TestCommandResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

impl TestCommandResult {
    fn success(&self) -> &Self {
        assert_eq!(
            self.exit_code, 0,
            "Command failed with stderr: {}",
            self.stderr
        );
        self
    }

    fn failure(&self) -> &Self {
        assert_ne!(self.exit_code, 0, "Command unexpectedly succeeded");
        self
    }

    fn stdout(&self, predicate: predicates::str::ContainsPredicate) -> &Self {
        assert!(predicate.eval(&self.stdout), "stdout assertion failed");
        self
    }

    fn stderr(&self, predicate: predicates::str::ContainsPredicate) -> &Self {
        assert!(predicate.eval(&self.stderr), "stderr assertion failed");
        self
    }
}

/// Helper to create a test prompt for the workflow
fn create_test_prompt() -> String {
    r#"---
name: Test Prompt
title: Test Prompt
description: A test prompt for liquid template testing
parameters:
  - name: message
    description: The message
    required: true
  - name: count
    description: The count
    required: false
    default: "1"
---

This is a test prompt with message: {{ message }} and count: {{ count }}
"#
    .to_string()
}

/// Helper to create a test workflow that uses template variables
fn create_test_workflow_with_templates() -> String {
    r#"---
title: Test Workflow with Templates
description: A workflow that uses template variables
version: 1.0.0
---

# Test Workflow

```mermaid
stateDiagram-v2
    [*] --> start
    start --> process: Always
    process --> end: Always
    end --> [*]
```

## Actions

- start: Log "Starting with {{ greeting | default: 'Hello' }}"
- process: Execute prompt "test-prompt" with message="{{ message }}" count="{{ count | default: '1' }}"
- end: Log "Finished processing {{ count }} items for {{ message }}"
"#.to_string()
}

#[test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
fn test_workflow_with_var_variables() {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir).unwrap();

    // Create test workflow
    let workflow_path = workflow_dir.join("test-template.md");
    fs::write(&workflow_path, create_test_workflow_with_templates()).unwrap();

    // Create test prompt that the workflow uses
    let prompt_dir = env.swissarmyhammer_dir().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt_path = prompt_dir.join("test-prompt.md");
    fs::write(&prompt_path, create_test_prompt()).unwrap();

    // Run workflow with --var variables using in-process execution
    run_sah_sync_assert(&[
        "flow",
        "run",
        "test-template",
        "--var",
        "greeting=Bonjour",
        "--var",
        "message=TestMessage",
        "--var",
        "count=5",
        "--dry-run",
    ])
    .success()
    .stdout(predicate::str::contains("Dry run mode"))
    .stdout(predicate::str::contains("test-template"));
}

#[test]
fn test_invalid_var_variable_format() {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir).unwrap();

    // Create a minimal workflow so we get past the "workflow not found" error
    let workflow_path = workflow_dir.join("some-workflow.md");
    fs::write(
        &workflow_path,
        r#"---
title: Test
description: Test
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> end
    end --> [*]
```

## Actions

- end: Log "Done"
"#,
    )
    .unwrap();

    run_sah_sync_assert(&["flow", "run", "some-workflow", "--var", "invalid_format"])
        .failure()
        .stderr(predicate::str::contains("Invalid variable format"))
        .stderr(predicate::str::contains("key=value"));
}

#[tokio::test]
async fn test_workflow_with_special_chars_in_var_values() -> anyhow::Result<()> {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow that uses template variables
    let workflow_path = workflow_dir.join("special-chars-test.md");
    fs::write(
        &workflow_path,
        r#"---
title: Special Characters Test
description: Tests behavior with special characters in var values
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "Message: {{ message }}"
"#,
    )?;

    // Test with special characters
    let success1 = run_workflow_with_vars_in_process(
        "special-chars-test",
        vec!["message=Hello World! @#$%^&*()".to_string()],
        false,
    )
    .await?;

    assert!(success1, "Workflow should handle special characters");

    // Test with spaces and quotes
    let success2 = run_workflow_with_vars_in_process(
        "special-chars-test",
        vec!["message=Test with 'single' and \"double\" quotes".to_string()],
        false,
    )
    .await?;

    assert!(success2, "Workflow should handle quotes and spaces");
    Ok(())
}

#[test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
fn test_workflow_with_equals_sign_in_var_value() {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir).unwrap();

    // Create workflow
    let workflow_path = workflow_dir.join("equals-test.md");
    fs::write(
        &workflow_path,
        r#"---
title: Equals Sign Test
description: Tests values containing equals signs
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "Formula: {{ formula }}"
"#,
    )
    .unwrap();

    // Run workflow with equals sign in value
    run_sah_sync_assert(&["flow", "run", "equals-test", "--var", "formula=x=y+z"])
        .success()
        .stderr(predicate::str::contains("Formula: x=y+z"));
}

#[test]
fn test_prompt_test_with_empty_var_value() {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create prompt directory
    let prompt_dir = env.swissarmyhammer_dir().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();

    // Create test prompt
    let prompt_path = prompt_dir.join("empty-test.md");
    fs::write(
        &prompt_path,
        r#"---
title: Empty Test Prompt
description: Tests empty var values
parameters:
  - name: content
    required: true
---

Content: {{ content }}
Author: {{ author | default: "Anonymous" }}
Version: {{ version | default: "1.0" }}
"#,
    )
    .unwrap();

    // Test with empty var value using in-process execution
    run_sah_sync_assert(&[
        "prompt",
        "test",
        "empty-test",
        "--var",
        "content=Main content",
        "--var",
        "author=",
        "--var",
        "version=",
    ])
    .success()
    .stdout(predicate::str::contains("Content: Main content"))
    .stdout(predicate::str::contains("Author: "))
    .stdout(predicate::str::contains("Version: "));
}

#[test]
fn test_prompt_test_with_var_overriding_arg() {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create prompt directory
    let prompt_dir = env.swissarmyhammer_dir().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();

    // Create test prompt
    let prompt_path = prompt_dir.join("override-test.md");
    fs::write(
        &prompt_path,
        r#"---
title: Override Test Prompt
description: Tests var overriding arg
parameters:
  - name: message
    required: true
---

Message: {{ message }}
"#,
    )
    .unwrap();

    // Test with later --var overriding earlier --var using optimized command
    run_sah_sync_assert(&[
        "prompt",
        "test",
        "override-test",
        "--var",
        "message=Original message",
        "--var",
        "message=Overridden message",
    ])
    .success()
    .stdout(predicate::str::contains("Message: Overridden message"));
}

#[tokio::test]
async fn test_var_multiple_usage() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create simple workflow
    let workflow_path = workflow_dir.join("simple.md");
    fs::write(
        &workflow_path,
        r#"---
title: Simple Workflow
description: Test workflow
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> end
    end --> [*]
```

## Actions

- end: Log "Done"
"#,
    )?;

    // Run workflow with multiple --var using in-process execution
    let success = run_workflow_with_vars_in_process(
        "simple",
        vec![
            "context_var=value1".to_string(),
            "template_var=value2".to_string(),
        ],
        true, // dry-run
    )
    .await?;

    assert!(
        success,
        "Workflow should execute successfully with multiple vars"
    );
    Ok(())
}

#[tokio::test]
async fn test_full_workflow_execution_with_liquid_templates() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create test workflow with liquid templates
    let workflow_path = workflow_dir.join("template-workflow.md");
    fs::write(
        &workflow_path,
        r#"---
title: Template Test Workflow
description: Tests liquid template rendering during actual execution
version: 1.0.0
---

# Template Test Workflow

```mermaid
stateDiagram-v2
    [*] --> start
    start --> greeting: Always
    greeting --> counting: Always
    counting --> finish: Always
    finish --> [*]
```

## Actions

- start: Log "Starting workflow for {{ user_name | default: 'Unknown User' }}"
- greeting: Log "Hello {{ user_name }}! You are user number {{ user_id }}"
- counting: Log "Processing {{ item_count | default: '10' }} items"
- finish: Log "Workflow completed for {{ user_name }} (ID: {{ user_id }})"
"#,
    )?;

    // Run workflow with template variables
    let success = run_workflow_with_vars_in_process(
        "template-workflow",
        vec![
            "user_name=John Doe".to_string(),
            "user_id=12345".to_string(),
            "item_count=25".to_string(),
        ],
        false, // actual execution
    )
    .await?;

    assert!(
        success,
        "Workflow should execute successfully with liquid templates"
    );
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_missing_template_variables() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow that uses variables not provided
    let workflow_path = workflow_dir.join("missing-vars.md");
    fs::write(
        &workflow_path,
        r#"---
title: Missing Variables Test
description: Tests behavior when template variables are missing
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> start
    start --> [*]
```

## Actions

- start: Log "User: {{ username }}, Email: {{ email }}"
"#,
    )?;

    // Run workflow without providing required variables
    let success = run_workflow_with_vars_in_process(
        "missing-vars",
        vec![], // no variables provided
        false,
    )
    .await?;

    // The workflow should still run but with the template placeholders intact
    assert!(
        success,
        "Workflow should still execute with missing template variables"
    );
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_complex_liquid_templates() -> Result<()> {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create directories
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    let prompt_dir = env.swissarmyhammer_dir().join("prompts");
    fs::create_dir_all(&prompt_dir)?;

    // Create a prompt that uses template variables
    let prompt_path = prompt_dir.join("template-prompt.md");
    fs::write(
        &prompt_path,
        r#"---
name: Template Prompt
title: Template Test Prompt
description: A prompt that uses template variables
parameters:
  - name: user
    description: User name
    required: true
  - name: task
    description: Task description
    required: true
---

Processing task "{{ task }}" for user {{ user }}.
"#,
    )?;

    // Create workflow with complex templates
    let workflow_path = workflow_dir.join("complex-templates.md");
    fs::write(
        &workflow_path,
        r#"---
title: Complex Template Workflow
description: Tests complex liquid template features
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> start
    start --> process: Always
    process --> [*]
```

## Actions

- start: Set task_description="{{ task_type }} for {{ project_name | default: 'Default Project' }}"
- process: Execute prompt "template-prompt" with user="{{ user_name }}" task="{{ task_type }} for {{ project_name }}"
"#,
    )?;

    // Run workflow with complex template variables using dry-run
    let success = run_workflow_with_vars_in_process(
        "complex-templates",
        vec![
            "user_name=Alice".to_string(),
            "task_type=Code Review".to_string(),
            "project_name=SwissArmyHammer".to_string(),
        ],
        true, // dry-run since we don't want to actually execute the prompt
    )
    .await?;

    assert!(
        success,
        "Complex template workflow should execute successfully"
    );
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_malformed_liquid_templates() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow with various malformed liquid templates
    let workflow_path = workflow_dir.join("malformed-templates.md");
    fs::write(
        &workflow_path,
        r#"---
title: Malformed Templates Test
description: Tests various malformed liquid template scenarios
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> unclosed
    unclosed --> invalid_filter: Always
    invalid_filter --> nested_error: Always
    nested_error --> [*]
```

## Actions

- unclosed: Log "Unclosed tag: {{ name"
- invalid_filter: Log "Invalid filter: {{ name | nonexistent_filter }}"
- nested_error: Log "Nested error: {% for item in {{ items }} %}{{ item }}{% endfor %}"
"#,
    )?;

    // Run workflow with template variables
    let success = run_workflow_with_vars_in_process(
        "malformed-templates",
        vec!["name=Test".to_string(), "items=[1,2,3]".to_string()],
        false,
    )
    .await?;

    // The workflow should still run but with original text for malformed templates
    assert!(
        success,
        "Workflow should still execute with malformed liquid templates"
    );
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_liquid_injection_attempts() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow that tests injection attempts
    let workflow_path = workflow_dir.join("injection-test.md");
    fs::write(
        &workflow_path,
        r#"---
title: Injection Test
description: Tests liquid template security
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "User input: {{ user_input }}"
"#,
    )?;

    // Run workflow with potentially malicious input
    let success = run_workflow_with_vars_in_process(
        "injection-test",
        vec!["user_input={{ '{% raw %}' }}{{ system }}{{ '{% endraw %}' }}".to_string()],
        false,
    )
    .await?;

    // The workflow should safely render the input without executing any injected liquid code
    assert!(success, "Workflow should safely handle injection attempts");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_empty_var_value() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow that uses template variables
    let workflow_path = workflow_dir.join("empty-value-test.md");
    fs::write(
        &workflow_path,
        r#"---
title: Empty Value Test
description: Tests behavior with empty var values
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "Name: '{{ name }}', Description: '{{ description | default: 'No description' }}'"
"#,
    )?;

    // Run workflow with empty var value
    let success = run_workflow_with_vars_in_process(
        "empty-value-test",
        vec!["name=".to_string(), "description=".to_string()],
        false,
    )
    .await?;

    // Empty values should be accepted and rendered as empty strings
    assert!(success, "Workflow should handle empty var values");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_duplicate_var_names() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().unwrap();

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = _env.swissarmyhammer_dir().join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create workflow that uses both context and template variables
    let workflow_path = workflow_dir.join("conflict-test.md");
    fs::write(
        &workflow_path,
        r#"---
title: Variable Conflict Test
description: Tests behavior when --var has duplicate names
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "Value from template: {{ value }}"
"#,
    )?;

    // Run workflow with conflicting names (later values should take precedence)
    let success = run_workflow_with_vars_in_process(
        "conflict-test",
        vec![
            "value=from_var".to_string(),
            "value=from_set".to_string(), // This should take precedence
        ],
        false,
    )
    .await?;

    // Later --var values should take precedence for template rendering
    assert!(
        success,
        "Workflow should handle duplicate var names with later values taking precedence"
    );
    Ok(())
}
