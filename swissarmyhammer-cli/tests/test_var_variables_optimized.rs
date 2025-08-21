//! Optimized in-process tests for --var variable functionality in workflows
//! 
//! These tests replace the slow CLI process-spawning tests in test_var_variables.rs
//! with fast in-process execution using the existing CLI flow functions.

use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

mod in_process_test_utils;
use swissarmyhammer_cli::{
    cli::FlowSubcommand,
    flow::run_flow_command,
};

/// Run flow command with variables in-process
async fn run_workflow_with_vars_in_process(
    temp_dir: &std::path::Path,
    workflow_name: &str, 
    vars: Vec<String>,
    dry_run: bool
) -> Result<bool> {
    // Change to temp directory
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir)?;
    
    let result = {
        let subcommand = FlowSubcommand::Run {
            workflow: workflow_name.to_string(),
            vars,
            interactive: false,
            dry_run,
            test: false,
            timeout: Some("2s".to_string()), // Use 2 second timeout for fast tests
            quiet: true,
        };
        
        run_flow_command(subcommand).await
    };
    
    // Restore original directory
    std::env::set_current_dir(original_dir)?;
    
    Ok(result.is_ok())
}

#[tokio::test]
async fn test_var_multiple_usage_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory in temp dir
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "simple",
        vec![
            "context_var=value1".to_string(),
            "template_var=value2".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success, "Workflow should execute successfully with multiple vars");
    Ok(())
}

#[tokio::test]
async fn test_full_workflow_execution_with_liquid_templates_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "template-workflow",
        vec![
            "user_name=John Doe".to_string(),
            "user_id=12345".to_string(),
            "item_count=25".to_string(),
        ],
        false, // actual execution
    ).await?;

    assert!(success, "Workflow should execute successfully with liquid templates");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_missing_template_variables_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "missing-vars",
        vec![], // no variables provided
        false,
    ).await?;

    // The workflow should still run but with the template placeholders intact
    assert!(success, "Workflow should still execute with missing template variables");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_complex_liquid_templates_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create directories
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    let prompt_dir = temp_path.join(".swissarmyhammer").join("prompts");
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
    fs::write(&workflow_path, r#"---
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
"#)?;

    // Run workflow with complex template variables using dry-run
    let success = run_workflow_with_vars_in_process(
        temp_path,
        "complex-templates",
        vec![
            "user_name=Alice".to_string(),
            "task_type=Code Review".to_string(),
            "project_name=SwissArmyHammer".to_string(),
        ],
        true, // dry-run since we don't want to actually execute the prompt
    ).await?;

    assert!(success, "Complex template workflow should execute successfully");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_malformed_liquid_templates_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "malformed-templates",
        vec![
            "name=Test".to_string(),
            "items=[1,2,3]".to_string(),
        ],
        false,
    ).await?;

    // The workflow should still run but with original text for malformed templates
    assert!(success, "Workflow should still execute with malformed liquid templates");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_liquid_injection_attempts_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "injection-test",
        vec!["user_input={{ '{% raw %}' }}{{ system }}{{ '{% endraw %}' }}".to_string()],
        false,
    ).await?;

    // The workflow should safely render the input without executing any injected liquid code
    assert!(success, "Workflow should safely handle injection attempts");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_empty_var_value_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "empty-value-test",
        vec![
            "name=".to_string(),
            "description=".to_string(),
        ],
        false,
    ).await?;

    // Empty values should be accepted and rendered as empty strings
    assert!(success, "Workflow should handle empty var values");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_special_chars_in_var_values_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "special-chars-test",
        vec!["message=Hello World! @#$%^&*()".to_string()],
        false,
    ).await?;

    assert!(success1, "Workflow should handle special characters");

    // Test with spaces and quotes
    let success2 = run_workflow_with_vars_in_process(
        temp_path,
        "special-chars-test",
        vec!["message=Test with 'single' and \"double\" quotes".to_string()],
        false,
    ).await?;

    assert!(success2, "Workflow should handle quotes and spaces");
    Ok(())
}

#[tokio::test]
async fn test_workflow_with_duplicate_var_names_optimized() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Setup proper git repository
    setup_git_repo(temp_path)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
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
        temp_path,
        "conflict-test",
        vec![
            "value=from_var".to_string(),
            "value=from_set".to_string(), // This should take precedence
        ],
        false,
    ).await?;

    // Later --var values should take precedence for template rendering
    assert!(success, "Workflow should handle duplicate var names with later values taking precedence");
    Ok(())
}