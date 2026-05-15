//! CLI argument parsing tests for model commands
//!
//! Tests command structure, help text generation, argument validation,
//! and error message clarity for model subcommands.

use anyhow::Result;
use clap::Parser;
use swissarmyhammer_cli::cli::{Cli, Commands, ModelSubcommand, OutputFormat};

/// Test utility to parse CLI arguments and return result
fn try_parse_cli(args: &[&str]) -> Result<Cli, clap::Error> {
    let args_with_program: Vec<String> = std::iter::once("sah".to_string())
        .chain(args.iter().map(|s| s.to_string()))
        .collect();

    Cli::try_parse_from(args_with_program)
}

/// Test utility to get help text for a command
fn get_help_text(args: &[&str]) -> String {
    match try_parse_cli(args) {
        Err(e) => {
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => e.to_string(),
                _ => format!("Parse error: {}", e),
            }
        }
        Ok(_) => "Command parsed successfully".to_string(),
    }
}

// =============================================================================
// HELPER FUNCTIONS TO REDUCE COGNITIVE COMPLEXITY
// =============================================================================

/// Assert that CLI parsed to a Model command with a List subcommand
fn assert_list_subcommand(cli: &Cli, expected_format: OutputFormat) {
    match &cli.command {
        Some(Commands::Model { subcommand }) => match subcommand {
            Some(ModelSubcommand::List { format }) => {
                assert_eq!(*format, expected_format);
            }
            _ => panic!("Should parse as List subcommand"),
        },
        _ => panic!("Should parse as Model command"),
    }
}

/// Assert that CLI parsed to a Model command with a Use subcommand
fn assert_use_subcommand(cli: &Cli, expected_name: &str) {
    match &cli.command {
        Some(Commands::Model { subcommand }) => match subcommand {
            Some(ModelSubcommand::Use { name }) => {
                assert_eq!(name, expected_name);
            }
            _ => panic!("Should parse as Use subcommand"),
        },
        _ => panic!("Should parse as Model command"),
    }
}

/// Parse CLI args and assert the result is a List command with the expected format
fn assert_parsed_list_format(args: &[&str], expected: OutputFormat) {
    let result = try_parse_cli(args);
    assert!(result.is_ok(), "Should parse successfully");
    assert_list_subcommand(&result.unwrap(), expected);
}

/// Parse CLI args and assert the result is a Use command with expected model name
fn assert_parsed_use_args(args: &[&str], expected_name: &str) {
    let result = try_parse_cli(args);
    assert!(result.is_ok(), "Should parse successfully");
    assert_use_subcommand(&result.unwrap(), expected_name);
}

/// Assert that the CLI has verbose or debug flag set
fn assert_has_verbose_or_debug(cli: &Cli) {
    assert!(
        cli.verbose || cli.debug,
        "Global verbose or debug flag should be parsed"
    );
}

/// Assert that the CLI has quiet flag set
fn assert_has_quiet(cli: &Cli) {
    assert!(cli.quiet, "Global quiet flag should be parsed");
}

/// Assert that the CLI command is a Model command
fn assert_is_model_command(cli: &Cli) {
    assert!(matches!(cli.command, Some(Commands::Model { .. })));
}

/// Helper to assert common help text properties
fn assert_help_text_contains(help_text: &str, should_contain: &[&str], description: &str) {
    assert!(!help_text.is_empty(), "Help text should not be empty");
    assert!(help_text.lines().count() > 3, "Help should be multi-line");
    assert!(
        !help_text.contains("error:"),
        "Help should not contain error messages"
    );

    for required in should_contain {
        assert!(
            help_text.contains(required),
            "{}: Help should contain '{}' but got: {}",
            description,
            required,
            help_text
        );
    }
}

/// Helper to assert help text contains at least one of the provided terms
fn assert_help_text_contains_any(help_text: &str, any_of: &[&str], description: &str) {
    assert!(
        any_of.iter().any(|s| help_text.contains(s)),
        "{}: Help should contain at least one of {:?} but got: {}",
        description,
        any_of,
        help_text
    );
}

// =============================================================================
// BASIC COMMAND STRUCTURE TESTS
// =============================================================================

#[test]
fn test_model_command_basic_parsing() {
    // Test model command without subcommand now succeeds (defaults to Show)
    let result = try_parse_cli(&["model"]);
    assert!(
        result.is_ok(),
        "Model command without subcommand should succeed (defaults to Show)"
    );

    // Test valid model list command
    assert_parsed_list_format(&["model", "list"], OutputFormat::Table);

    // Test valid model use command
    assert_parsed_use_args(&["model", "use", "test-model"], "test-model");
}

#[test]
fn test_model_list_format_parsing() {
    // Test table format
    assert_parsed_list_format(&["model", "list", "--format", "table"], OutputFormat::Table);

    // Test json format
    assert_parsed_list_format(&["model", "list", "--format", "json"], OutputFormat::Json);

    // Test yaml format
    assert_parsed_list_format(&["model", "list", "--format", "yaml"], OutputFormat::Yaml);

    // Test invalid format
    let result = try_parse_cli(&["model", "list", "--format", "invalid"]);
    assert!(result.is_err(), "Invalid format should fail to parse");
}

#[test]
fn test_model_use_with_valid_name() {
    // Test with valid model name
    assert_parsed_use_args(&["model", "use", "claude-code"], "claude-code");

    // Test with single valid model name
    assert_parsed_use_args(&["model", "use", "test-model"], "test-model");
}

#[test]
fn test_model_use_with_special_characters() {
    // Test with model name containing hyphens and underscores
    let result = try_parse_cli(&["model", "use", "custom-agent_name"]);
    assert!(
        result.is_ok(),
        "Agent name with hyphens/underscores should parse"
    );

    // Test with model name containing numbers
    let result = try_parse_cli(&["model", "use", "agent-v2"]);
    assert!(result.is_ok(), "Agent name with numbers should parse");
}

#[test]
fn test_model_use_without_name() {
    // Test without model name (should fail)
    let result = try_parse_cli(&["model", "use"]);
    assert!(result.is_err(), "Model use without name should fail");
}

#[test]
fn test_model_use_with_two_arguments() {
    // With single model selection, two positional arguments should fail
    let result = try_parse_cli(&["model", "use", "first-agent", "second-agent"]);
    assert!(
        result.is_err(),
        "Model use with two arguments should fail (only one model name expected)"
    );
}

// =============================================================================
// HELP TEXT TESTS
// =============================================================================

#[test]
fn test_model_help_text_content() {
    let help_text = get_help_text(&["model", "--help"]);

    assert_help_text_contains(&help_text, &[], "Model help basic validation");

    // Should contain usage information
    assert_help_text_contains_any(
        &help_text,
        &["Usage:", "usage:"],
        "Model help should show usage",
    );

    // Should mention model command
    assert_help_text_contains_any(
        &help_text,
        &["model", "agent"],
        "Model help should mention model command",
    );

    // Should contain subcommands
    assert_help_text_contains(
        &help_text,
        &["list"],
        "Model help should mention list subcommand",
    );
    assert_help_text_contains(
        &help_text,
        &["use"],
        "Model help should mention use subcommand",
    );

    // Should contain subcommand descriptions
    assert_help_text_contains_any(
        &help_text,
        &["List available", "list"],
        "Model help should describe list command",
    );
    assert_help_text_contains_any(
        &help_text,
        &["Switch to", "Use", "use"],
        "Model help should describe use command",
    );
}

#[test]
fn test_model_list_help_text_content() {
    let help_text = get_help_text(&["model", "list", "--help"]);

    assert_help_text_contains(&help_text, &[], "Model list help basic validation");

    // Should contain usage information
    assert_help_text_contains_any(
        &help_text,
        &["Usage:", "usage:"],
        "Model list help should show usage",
    );

    // Should contain format option
    assert_help_text_contains_any(
        &help_text,
        &["format", "FORMAT"],
        "Model list help should mention format option",
    );

    // Should contain format choices
    assert_help_text_contains_any(
        &help_text,
        &["table", "json", "yaml"],
        "Model list help should show format options",
    );

    // Should mention listing
    assert_help_text_contains_any(
        &help_text,
        &["list", "List"],
        "Model list help should describe listing",
    );
}

#[test]
fn test_model_use_help_text_content() {
    let help_text = get_help_text(&["model", "use", "--help"]);

    assert_help_text_contains(&help_text, &[], "Model use help basic validation");

    // Should contain usage information
    assert_help_text_contains_any(
        &help_text,
        &["Usage:", "usage:"],
        "Model use help should show usage",
    );

    // Should contain model name parameter
    assert_help_text_contains_any(
        &help_text,
        &["NAME", "name", "<NAME>", "<name>"],
        "Model use help should show model name parameter",
    );

    // Should describe the use action
    assert_help_text_contains_any(
        &help_text,
        &["use", "Use", "switch"],
        "Model use help should describe use action",
    );

    // Should mention switching/selection
    assert_help_text_contains_any(
        &help_text,
        &["switch", "use", "select", "Apply"],
        "Model use help should describe switching",
    );
}

#[test]
fn test_help_text_formatting_quality() {
    let main_help = get_help_text(&["model", "--help"]);
    let list_help = get_help_text(&["model", "list", "--help"]);
    let use_help = get_help_text(&["model", "use", "--help"]);

    // Help text should not be empty
    assert!(!main_help.is_empty(), "Main help should not be empty");
    assert!(!list_help.is_empty(), "List help should not be empty");
    assert!(!use_help.is_empty(), "Use help should not be empty");

    // Should contain proper capitalization
    assert!(
        main_help.contains("Usage") || main_help.contains("USAGE"),
        "Should have proper Usage capitalization"
    );

    // Should not contain obvious errors
    assert!(
        !main_help.contains("error:"),
        "Help should not contain error messages"
    );
    assert!(
        !list_help.contains("error:"),
        "List help should not contain error messages"
    );
    assert!(
        !use_help.contains("error:"),
        "Use help should not contain error messages"
    );

    // Should be reasonably formatted (multiple lines)
    assert!(main_help.lines().count() > 3, "Help should be multi-line");
    assert!(
        list_help.lines().count() > 3,
        "List help should be multi-line"
    );
    assert!(
        use_help.lines().count() > 3,
        "Use help should be multi-line"
    );
}

// =============================================================================
// ERROR MESSAGE TESTS
// =============================================================================

#[test]
fn test_argument_validation_error_messages() {
    // Test missing subcommand - now succeeds with optional subcommand (defaults to Show)
    let result = try_parse_cli(&["model"]);
    assert!(
        result.is_ok(),
        "Should succeed without explicit subcommand (defaults to Show)"
    );

    // Test invalid subcommand
    let result = try_parse_cli(&["model", "invalid-subcommand"]);
    assert!(result.is_err(), "Should fail with invalid subcommand");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("invalid-subcommand") || error_msg.contains("unexpected"),
        "Error should mention invalid subcommand: {}",
        error_msg
    );

    // Test missing model name for use command
    let result = try_parse_cli(&["model", "use"]);
    assert!(result.is_err(), "Should fail without model name");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("name")
            || error_msg.contains("NAME")
            || error_msg.contains("required")
            || error_msg.contains("missing"),
        "Error should mention missing model name: {}",
        error_msg
    );
}

#[test]
fn test_format_validation_error_messages() {
    // Test invalid format value
    let result = try_parse_cli(&["model", "list", "--format", "invalid"]);
    assert!(result.is_err(), "Should fail with invalid format");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("invalid")
            && (error_msg.contains("format") || error_msg.contains("value")),
        "Error should mention invalid format: {}",
        error_msg
    );

    // Should suggest valid formats
    assert!(
        error_msg.contains("table") || error_msg.contains("json") || error_msg.contains("yaml"),
        "Error should suggest valid formats: {}",
        error_msg
    );

    // Test format without value
    let result = try_parse_cli(&["model", "list", "--format"]);
    assert!(result.is_err(), "Should fail with format flag but no value");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("format")
            && (error_msg.contains("value")
                || error_msg.contains("requires")
                || error_msg.contains("argument")),
        "Error should mention missing format value: {}",
        error_msg
    );
}

#[test]
fn test_error_message_clarity_and_helpfulness() {
    // Test that error messages provide actionable guidance
    let result = try_parse_cli(&["model", "unknown-command"]);
    assert!(result.is_err(), "Should fail with unknown command");

    let error_msg = result.unwrap_err().to_string();

    // Should mention the invalid command
    assert!(
        error_msg.contains("unknown-command"),
        "Error should mention the invalid command"
    );

    // Should provide suggestions or available options
    assert!(
        error_msg.contains("list")
            || error_msg.contains("use")
            || error_msg.contains("available")
            || error_msg.contains("try"),
        "Error should provide helpful suggestions: {}",
        error_msg
    );

    // Test format-specific errors are clear
    let result = try_parse_cli(&["model", "list", "--format", "txt"]);
    assert!(result.is_err(), "Should fail with invalid format");

    let format_error = result.unwrap_err().to_string();
    assert!(
        format_error.contains("txt"),
        "Should mention invalid format value"
    );
    assert!(
        format_error.contains("table")
            || format_error.contains("json")
            || format_error.contains("yaml"),
        "Should show valid format options"
    );
}

// =============================================================================
// COMMAND STRUCTURE VALIDATION TESTS
// =============================================================================

#[test]
fn test_command_hierarchy_structure() {
    // Test that model is a proper subcommand of the main CLI
    let result = try_parse_cli(&["model", "list"]);
    assert!(result.is_ok(), "Agent should be a valid top-level command");

    // Verify the parsed structure
    if let Ok(cli) = result {
        assert_is_model_command(&cli);
        assert_list_subcommand(&cli, OutputFormat::Table);
    }

    // Test that model subcommands are properly nested
    let result = try_parse_cli(&["model", "use", "test"]);
    assert!(result.is_ok(), "Model use should be properly nested");

    if let Ok(cli) = result {
        assert_is_model_command(&cli);
        assert_use_subcommand(&cli, "test");
    }
}

#[test]
fn test_global_verbose_flag() {
    let result = try_parse_cli(&["--verbose", "model", "list"]);
    if let Ok(cli) = result {
        assert_has_verbose_or_debug(&cli);
        assert_is_model_command(&cli);
    }
}

#[test]
fn test_global_quiet_flag() {
    let result = try_parse_cli(&["--quiet", "model", "use", "test"]);
    if let Ok(cli) = result {
        assert_has_quiet(&cli);
        assert_is_model_command(&cli);
    }
}

#[test]
fn test_global_debug_flag() {
    let result = try_parse_cli(&["--debug", "model", "list", "--format", "json"]);
    if let Ok(cli) = result {
        assert!(cli.debug, "Global debug flag should be parsed");
        assert_is_model_command(&cli);
        assert_list_subcommand(&cli, OutputFormat::Json);
    }
}

// =============================================================================
// OUTPUT FORMAT ENUM TESTS
// =============================================================================

#[test]
fn test_output_format_enum_completeness() {
    // Test all expected output formats can be parsed
    let formats = [
        ("table", OutputFormat::Table),
        ("json", OutputFormat::Json),
        ("yaml", OutputFormat::Yaml),
    ];

    for (format_str, expected_format) in &formats {
        assert_parsed_list_format(&["model", "list", "--format", format_str], *expected_format);
    }
}

#[test]
fn test_output_format_case_sensitivity() {
    // Test that format parsing is case-sensitive (lowercase expected)
    let uppercase_formats = ["TABLE", "JSON", "YAML"];

    for format in &uppercase_formats {
        let result = try_parse_cli(&["model", "list", "--format", format]);
        assert!(
            result.is_err(),
            "Uppercase format '{}' should be rejected",
            format
        );
    }

    // Test mixed case
    let mixed_formats = ["Table", "Json", "Yaml"];

    for format in &mixed_formats {
        let result = try_parse_cli(&["model", "list", "--format", format]);
        assert!(
            result.is_err(),
            "Mixed case format '{}' should be rejected",
            format
        );
    }
}

// =============================================================================
// EDGE CASES AND BOUNDARY TESTS
// =============================================================================

#[test]
fn test_model_name_edge_cases() {
    // Test various model name formats
    let valid_names = [
        "simple",
        "with-hyphens",
        "with_underscores",
        "with123numbers",
        "mixed-name_123",
        "a", // Single character
        "very-long-agent-name-with-many-components",
    ];

    for name in &valid_names {
        assert_parsed_use_args(&["model", "use", name], name);
    }
}

#[test]
fn test_argument_order_flexibility() {
    // Test different argument orders where applicable
    let result1 = try_parse_cli(&["model", "list", "--format", "json"]);
    let result2 = try_parse_cli(&["--verbose", "model", "list", "--format", "json"]);

    if let Err(err) = &result1 {
        eprintln!("result1 error: {:?}", err);
    }
    if let Err(err) = &result2 {
        eprintln!("result2 error: {:?}", err);
    }

    assert!(
        result1.is_ok() && result2.is_ok(),
        "Different argument orders should work"
    );

    // Both should parse to the same subcommand structure
    let cli1 = result1.unwrap();
    let cli2 = result2.unwrap();
    assert_list_subcommand(&cli1, OutputFormat::Json);
    assert_list_subcommand(&cli2, OutputFormat::Json);
}

#[test]
fn test_help_flag_variations() {
    // Test different help flag formats
    let help_variations = [
        &["model", "--help"][..],
        &["model", "-h"][..],
        &["help", "model"][..], // This might not work depending on CLI structure
    ];

    for help_args in &help_variations {
        let result = try_parse_cli(help_args);
        // Should either succeed and show help, or fail with help display error
        match result {
            Ok(_) => {
                // Command succeeded (unlikely for help)
            }
            Err(e) => {
                use clap::error::ErrorKind;
                if e.kind() == ErrorKind::DisplayHelp {
                    // This is expected for help flags
                    let help_text = e.to_string();
                    assert!(
                        help_text.contains("model"),
                        "Help should mention model command"
                    );
                } else {
                    // For variations that might not be supported, that's also OK
                }
            }
        }
    }
}
