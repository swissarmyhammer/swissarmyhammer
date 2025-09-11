//! Performance tests for prompt command architecture
//!
//! These tests ensure that prompt commands execute within reasonable time bounds
//! and handle large datasets efficiently.

use std::io::Write;
use std::time::Instant;
use swissarmyhammer_cli::commands::prompt::{cli, handle_command_typed, PromptCommand};
use swissarmyhammer_cli::context::CliContextBuilder;
use swissarmyhammer_config::TemplateContext;
use tempfile::NamedTempFile;

/// Helper to create a test context for performance testing
async fn create_performance_test_context(
    format: swissarmyhammer_cli::cli::OutputFormat,
) -> swissarmyhammer_cli::context::CliContext {
    let template_context = TemplateContext::new();
    let matches = clap::Command::new("test")
        .try_get_matches_from(["test"])
        .unwrap();

    CliContextBuilder::default()
        .template_context(template_context)
        .format(format)
        .format_option(Some(format))
        .verbose(false)
        .debug(false)
        .quiet(true) // Quiet mode for performance tests
        .matches(matches)
        .build_async()
        .await
        .unwrap()
}

/// Create a large prompt file for stress testing
fn create_large_prompt_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(file, "---").expect("Failed to write to temp file");
    writeln!(file, "title: Large Performance Test Prompt").expect("Failed to write to temp file");
    writeln!(file, "description: A large prompt for performance testing")
        .expect("Failed to write to temp file");
    writeln!(file, "---").expect("Failed to write to temp file");

    // Create a large template content
    for i in 0..size {
        writeln!(
            file,
            "Line {}: This is test content for performance testing {{ var_{} }}",
            i,
            i % 10
        )
        .expect("Failed to write to temp file");
    }

    file
}

#[tokio::test]
async fn test_list_command_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

    let duration = start.elapsed();

    assert_eq!(exit_code, 0, "List command should succeed");
    assert!(
        duration.as_millis() < 5000,
        "List command took too long: {:?} (should be < 5s)",
        duration
    );

    println!("List command completed in: {:?}", duration);
}

#[tokio::test]
async fn test_list_command_performance_verbose() {
    let start = Instant::now();

    let mut context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

    // Enable verbose mode for this test
    context.verbose = true;
    context.quiet = false;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

    let duration = start.elapsed();

    assert_eq!(exit_code, 0, "Verbose list command should succeed");
    assert!(
        duration.as_millis() < 7000,
        "Verbose list command took too long: {:?} (should be < 7s)",
        duration
    );

    println!("Verbose list command completed in: {:?}", duration);
}

#[tokio::test]
async fn test_list_command_json_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Json).await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

    let duration = start.elapsed();

    assert_eq!(exit_code, 0, "JSON list command should succeed");
    assert!(
        duration.as_millis() < 5000,
        "JSON list command took too long: {:?} (should be < 5s)",
        duration
    );

    println!("JSON list command completed in: {:?}", duration);
}

#[tokio::test]
async fn test_list_command_yaml_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Yaml).await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

    let duration = start.elapsed();

    assert_eq!(exit_code, 0, "YAML list command should succeed");
    assert!(
        duration.as_millis() < 5000,
        "YAML list command took too long: {:?} (should be < 5s)",
        duration
    );

    println!("YAML list command completed in: {:?}", duration);
}

#[tokio::test]
async fn test_test_command_large_file_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

    // Create a large prompt file (1000 lines)
    let temp_file = create_large_prompt_file(1000);
    let file_path = temp_file.path().to_str().unwrap().to_string();

    let test_cmd = cli::TestCommand {
        prompt_name: None,
        file: Some(file_path),
        vars: (0..10).map(|i| format!("var_{}=value_{}", i, i)).collect(),
        raw: true,
        copy: false,
        save: None,
        debug: false,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;

    let duration = start.elapsed();

    // The command may succeed or fail depending on the templating system
    // but it should complete within a reasonable time
    assert!(
        duration.as_millis() < 10000,
        "Large file test command took too long: {:?} (should be < 10s)",
        duration
    );

    println!(
        "Large file test command completed in: {:?} (exit code: {})",
        duration, exit_code
    );
}

#[tokio::test]
async fn test_multiple_commands_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Json).await;

    // Run multiple list commands in sequence
    for i in 0..10 {
        let cmd_start = Instant::now();

        let exit_code =
            handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

        let cmd_duration = cmd_start.elapsed();

        assert_eq!(exit_code, 0, "List command {} should succeed", i);
        assert!(
            cmd_duration.as_millis() < 3000,
            "Individual list command {} took too long: {:?}",
            i,
            cmd_duration
        );
    }

    let total_duration = start.elapsed();
    assert!(
        total_duration.as_millis() < 20000,
        "10 list commands took too long total: {:?} (should be < 20s)",
        total_duration
    );

    println!("10 list commands completed in: {:?}", total_duration);
}

#[tokio::test]
async fn test_context_creation_performance() {
    let start = Instant::now();

    // Create multiple contexts to test context creation performance
    for i in 0..5 {
        let ctx_start = Instant::now();

        let _context =
            create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

        let ctx_duration = ctx_start.elapsed();
        assert!(
            ctx_duration.as_millis() < 1000,
            "Context creation {} took too long: {:?}",
            i,
            ctx_duration
        );
    }

    let total_duration = start.elapsed();
    assert!(
        total_duration.as_millis() < 3000,
        "5 context creations took too long total: {:?}",
        total_duration
    );

    println!("5 context creations completed in: {:?}", total_duration);
}

#[tokio::test]
async fn test_validate_command_performance() {
    let start = Instant::now();

    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

    let exit_code =
        handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context).await;

    let duration = start.elapsed();

    // Validate command may succeed or fail, but should complete reasonably fast
    assert!(
        duration.as_millis() < 15000,
        "Validate command took too long: {:?} (should be < 15s)",
        duration
    );

    println!(
        "Validate command completed in: {:?} (exit code: {})",
        duration, exit_code
    );
}

#[tokio::test]
async fn test_memory_usage_stress() {
    // This test runs many operations to check for memory leaks
    let context =
        create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Json).await;

    let start = Instant::now();

    // Run a mix of commands
    for i in 0..20 {
        if i % 3 == 0 {
            let _exit_code =
                handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
        } else if i % 3 == 1 {
            let _exit_code =
                handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context)
                    .await;
        } else {
            // Test command with nonexistent prompt (should fail quickly)
            let test_cmd = cli::TestCommand {
                prompt_name: Some(format!("nonexistent_{}", i)),
                file: None,
                vars: vec![],
                raw: false,
                copy: false,
                save: None,
                debug: false,
            };
            let _exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
        }
    }

    let duration = start.elapsed();
    assert!(
        duration.as_millis() < 30000,
        "Stress test took too long: {:?} (should be < 30s)",
        duration
    );

    println!("Memory stress test completed in: {:?}", duration);
}

#[tokio::test]
async fn test_sequential_context_usage() {
    let start = Instant::now();

    // Create multiple contexts and use them sequentially (since CliContext isn't Send)
    for i in 0..5 {
        let context =
            create_performance_test_context(swissarmyhammer_cli::cli::OutputFormat::Table).await;

        let exit_code =
            handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;

        println!(
            "Sequential task {} completed with success: {}",
            i,
            exit_code == 0
        );
    }

    let duration = start.elapsed();
    assert!(
        duration.as_millis() < 15000,
        "Sequential context usage took too long: {:?} (should be < 15s)",
        duration
    );

    println!("Sequential context usage completed in: {:?}", duration);
}

#[tokio::test]
async fn test_display_conversion_performance() {
    use std::collections::HashMap;
    use swissarmyhammer_cli::commands::prompt::display;

    let start = Instant::now();

    // Create a large number of test prompts
    let mut prompts = Vec::new();
    for i in 0..1000 {
        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::json!(format!("Test Prompt {}", i)),
        );

        let prompt = swissarmyhammer_prompts::Prompt {
            name: format!("test-prompt-{}", i),
            description: Some(format!("Description for prompt {}", i)),
            category: Some("performance".to_string()),
            tags: vec![format!("tag{}", i % 5)],
            template: format!("Template content for prompt {}: {{{{ var_{} }}}}", i, i),
            parameters: vec![],
            source: Some(std::path::PathBuf::from(format!(
                "/test/path/prompt-{}.md",
                i
            ))),
            metadata,
        };
        prompts.push(prompt);
    }

    // Test standard display conversion
    let display_rows = display::prompts_to_display_rows(prompts.clone(), false);
    match display_rows {
        display::DisplayRows::Standard(rows) => {
            assert_eq!(rows.len(), 1000, "Should convert all prompts");
        }
        _ => panic!("Expected standard display rows"),
    }

    // Test verbose display conversion
    let display_rows = display::prompts_to_display_rows(prompts, true);
    match display_rows {
        display::DisplayRows::Verbose(rows) => {
            assert_eq!(rows.len(), 1000, "Should convert all prompts");
        }
        _ => panic!("Expected verbose display rows"),
    }

    let duration = start.elapsed();
    assert!(
        duration.as_millis() < 5000,
        "Display conversion took too long: {:?} (should be < 5s)",
        duration
    );

    println!(
        "Display conversion for 1000 prompts completed in: {:?}",
        duration
    );
}
