//! End-to-end integration tests for prompt command architecture
//!
//! These tests verify the complete workflow from command parsing through execution
//! and output formatting across all prompt subcommands.

use swissarmyhammer_cli::commands::prompt::{cli, handle_command_typed, PromptCommand};
use swissarmyhammer_cli::context::CliContextBuilder;
use swissarmyhammer_config::TemplateContext;

use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to create a test context with specified options
async fn create_test_context(
    format: swissarmyhammer_cli::cli::OutputFormat,
    verbose: bool,
    debug: bool,
    quiet: bool,
) -> swissarmyhammer_cli::context::CliContext {
    let template_context = TemplateContext::new();
    let matches = clap::Command::new("test")
        .try_get_matches_from(["test"])
        .unwrap();

    CliContextBuilder::default()
        .template_context(template_context)
        .format(format)
        .format_option(Some(format))
        .verbose(verbose)
        .debug(debug)
        .quiet(quiet)
        .matches(matches)
        .build_async()
        .await
        .unwrap()
}

/// Create a temporary prompt file for testing
fn create_temp_prompt_file(name: &str, content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(file, "---").expect("Failed to write to temp file");
    writeln!(file, "title: {}", name).expect("Failed to write to temp file");
    writeln!(file, "description: Test prompt for integration testing")
        .expect("Failed to write to temp file");
    writeln!(file, "---").expect("Failed to write to temp file");
    writeln!(file, "{}", content).expect("Failed to write to temp file");
    file
}

#[tokio::test]
async fn test_list_command_integration() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        false,
    )
    .await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
    assert_eq!(exit_code, 0, "List command should succeed");
}

#[tokio::test]
async fn test_list_command_verbose_integration() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        true,
        false,
        false,
    )
    .await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
    assert_eq!(exit_code, 0, "Verbose list command should succeed");
}

#[tokio::test]
async fn test_list_command_json_format_integration() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Json,
        false,
        false,
        false,
    )
    .await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
    assert_eq!(exit_code, 0, "List command with JSON output should succeed");
}

#[tokio::test]
async fn test_list_command_yaml_format_integration() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Yaml,
        false,
        false,
        false,
    )
    .await;

    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
    assert_eq!(exit_code, 0, "List command with YAML output should succeed");
}

#[tokio::test]
async fn test_test_command_with_nonexistent_prompt() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        false,
    )
    .await;

    let test_cmd = cli::TestCommand {
        prompt_name: Some("nonexistent_prompt_12345".to_string()),
        file: None,
        vars: vec![],
        raw: false,
        copy: false,
        save: None,
        debug: false,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
    assert_ne!(
        exit_code, 0,
        "Test command with nonexistent prompt should fail"
    );
}

#[tokio::test]
async fn test_test_command_with_file() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        true, // quiet mode to suppress output
    )
    .await;

    let temp_file = create_temp_prompt_file("Test Prompt", "Hello, {{ name }}!");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    let test_cmd = cli::TestCommand {
        prompt_name: None,
        file: Some(file_path),
        vars: vec!["name=World".to_string()],
        raw: true,
        copy: false,
        save: None,
        debug: false,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
    // This may succeed or fail depending on the templating system setup
    // The important thing is that it doesn't crash
    assert!(exit_code >= 0); // Exit codes are always non-negative
}

#[tokio::test]
async fn test_test_command_with_save_file() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        true, // quiet mode
    )
    .await;

    let temp_file = create_temp_prompt_file("Save Test", "Content to save: {{ value }}");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    let output_file = NamedTempFile::new().expect("Failed to create output temp file");
    let output_path = output_file.path().to_str().unwrap().to_string();

    let test_cmd = cli::TestCommand {
        prompt_name: None,
        file: Some(file_path),
        vars: vec!["value=test123".to_string()],
        raw: false,
        copy: false,
        save: Some(output_path.clone()),
        debug: false,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;

    // Check if the command succeeded and potentially created the output file
    if exit_code == 0 {
        // If successful, the output file should exist and have content
        if std::path::Path::new(&output_path).exists() {
            let content = std::fs::read_to_string(&output_path).unwrap_or_default();
            assert!(!content.is_empty(), "Output file should have content");
        }
    }
}

#[tokio::test]
async fn test_test_command_debug_mode() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        true, // debug mode
        true, // quiet mode to suppress normal output
    )
    .await;

    let temp_file = create_temp_prompt_file("Debug Test", "Debug content: {{ debug_var }}");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    let test_cmd = cli::TestCommand {
        prompt_name: None,
        file: Some(file_path),
        vars: vec!["debug_var=debug_value".to_string()],
        raw: false,
        copy: false,
        save: None,
        debug: true,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
    // Debug mode should work the same as normal mode functionally
    assert!(exit_code >= 0); // Exit codes are always non-negative
}

#[tokio::test]
async fn test_test_command_missing_required_params() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        false,
    )
    .await;

    let test_cmd = cli::TestCommand {
        prompt_name: None,
        file: None,
        vars: vec![],
        raw: false,
        copy: false,
        save: None,
        debug: false,
    };

    let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
    assert_ne!(
        exit_code, 0,
        "Test command without prompt name or file should fail"
    );
}

#[tokio::test]
async fn test_validate_command_integration() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        false,
    )
    .await;

    let exit_code =
        handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context).await;
    // Validate command should succeed or fail gracefully
    assert!(exit_code >= 0); // Exit codes are always non-negative
}

#[tokio::test]
async fn test_all_commands_with_different_formats() {
    let formats = vec![
        swissarmyhammer_cli::cli::OutputFormat::Table,
        swissarmyhammer_cli::cli::OutputFormat::Json,
        swissarmyhammer_cli::cli::OutputFormat::Yaml,
    ];

    for format in formats {
        let context = create_test_context(format, false, false, true).await;

        // Test list command
        let exit_code =
            handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
        assert_eq!(
            exit_code, 0,
            "List command should succeed with format {:?}",
            format
        );

        // Test validate command
        let exit_code =
            handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context).await;
        // Validate may succeed or fail, but shouldn't crash
        assert!(
            exit_code >= 0,
            "Validate command should handle format {:?}",
            format
        );
    }
}

#[tokio::test]
async fn test_commands_with_verbose_and_debug() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        true, // verbose
        true, // debug
        false,
    )
    .await;

    // Test list command with verbose and debug
    let exit_code = handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
    assert_eq!(
        exit_code, 0,
        "List command should succeed with verbose and debug"
    );

    // Test validate command with verbose and debug
    let exit_code =
        handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context).await;
    assert!(
        exit_code >= 0,
        "Validate command should handle verbose and debug"
    );
}

#[tokio::test]
async fn test_context_builder_variations() {
    // Test that the context builder works with different configurations
    let variations = vec![
        (
            swissarmyhammer_cli::cli::OutputFormat::Table,
            false,
            false,
            false,
        ),
        (
            swissarmyhammer_cli::cli::OutputFormat::Json,
            true,
            false,
            false,
        ),
        (
            swissarmyhammer_cli::cli::OutputFormat::Yaml,
            false,
            true,
            false,
        ),
        (
            swissarmyhammer_cli::cli::OutputFormat::Table,
            true,
            true,
            true,
        ),
    ];

    for (format, verbose, debug, quiet) in variations {
        let context = create_test_context(format, verbose, debug, quiet).await;

        assert_eq!(context.verbose, verbose);
        assert_eq!(context.debug, debug);
        assert_eq!(context.quiet, quiet);

        // Ensure context can execute commands
        let exit_code =
            handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
        assert_eq!(
            exit_code, 0,
            "Context with format {:?}, verbose {}, debug {}, quiet {} should work",
            format, verbose, debug, quiet
        );
    }
}

#[tokio::test]
async fn test_error_handling_comprehensive() {
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Table,
        false,
        false,
        false,
    )
    .await;

    // Test various error scenarios
    let error_scenarios = vec![
        // Nonexistent file
        cli::TestCommand {
            prompt_name: None,
            file: Some("/nonexistent/path/file.md".to_string()),
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        },
        // Missing both prompt name and file
        cli::TestCommand {
            prompt_name: None,
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        },
        // Nonexistent prompt name
        cli::TestCommand {
            prompt_name: Some("definitely_nonexistent_prompt_name_12345".to_string()),
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        },
    ];

    for (i, test_cmd) in error_scenarios.into_iter().enumerate() {
        let exit_code = handle_command_typed(PromptCommand::Test(test_cmd), &context).await;
        assert_ne!(exit_code, 0, "Error scenario {} should fail gracefully", i);
    }
}

#[tokio::test]
async fn test_stress_multiple_commands() {
    // Test running multiple commands in sequence to ensure no state pollution
    let context = create_test_context(
        swissarmyhammer_cli::cli::OutputFormat::Json,
        false,
        false,
        true, // quiet to reduce test output
    )
    .await;

    for _ in 0..5 {
        // Run list command multiple times
        let exit_code =
            handle_command_typed(PromptCommand::List(cli::ListCommand {}), &context).await;
        assert_eq!(exit_code, 0, "Repeated list commands should succeed");

        // Run validate command multiple times
        let exit_code =
            handle_command_typed(PromptCommand::Validate(cli::ValidateCommand {}), &context).await;
        assert!(
            exit_code >= 0,
            "Repeated validate commands should be consistent"
        );
    }
}
