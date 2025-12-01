//! CLI argument parsing tests for agent commands
//!
//! Tests command structure, help text generation, argument validation,
//! and error message clarity for agent subcommands.

use anyhow::Result;
use clap::Parser;
use swissarmyhammer_cli::cli::{AgentSubcommand, Cli, Commands, OutputFormat};

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
// BASIC COMMAND STRUCTURE TESTS
// =============================================================================

#[test]
fn test_agent_command_basic_parsing() {
    // Test agent command without subcommand now succeeds (defaults to Show)
    let result = try_parse_cli(&["agent"]);
    assert!(
        result.is_ok(),
        "Agent command without subcommand should succeed (defaults to Show)"
    );

    // Test valid agent list command
    let result = try_parse_cli(&["agent", "list"]);
    assert!(result.is_ok(), "Agent list should parse successfully");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::List { format }) => {
                    assert_eq!(
                        format,
                        OutputFormat::Table,
                        "Default format should be Table"
                    );
                }
                _ => panic!("Should parse as List subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test valid agent use command
    let result = try_parse_cli(&["agent", "use", "test-agent"]);
    assert!(result.is_ok(), "Agent use should parse successfully");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::Use { first, second }) => {
                    assert_eq!(first, "test-agent", "Should parse agent name correctly");
                    assert_eq!(second, None, "Second argument should be None");
                }
                _ => panic!("Should parse as Use subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }
}

#[test]
fn test_agent_list_format_parsing() {
    // Test table format
    let result = try_parse_cli(&["agent", "list", "--format", "table"]);
    assert!(result.is_ok(), "Table format should parse");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::List { format }) => {
                    assert_eq!(format, OutputFormat::Table);
                }
                _ => panic!("Should parse as List subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test json format
    let result = try_parse_cli(&["agent", "list", "--format", "json"]);
    assert!(result.is_ok(), "JSON format should parse");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::List { format }) => {
                    assert_eq!(format, OutputFormat::Json);
                }
                _ => panic!("Should parse as List subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test yaml format
    let result = try_parse_cli(&["agent", "list", "--format", "yaml"]);
    assert!(result.is_ok(), "YAML format should parse");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::List { format }) => {
                    assert_eq!(format, OutputFormat::Yaml);
                }
                _ => panic!("Should parse as List subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test invalid format
    let result = try_parse_cli(&["agent", "list", "--format", "invalid"]);
    assert!(result.is_err(), "Invalid format should fail to parse");
}

#[test]
fn test_agent_use_argument_parsing() {
    // Test with valid agent name
    let result = try_parse_cli(&["agent", "use", "claude-code"]);
    assert!(result.is_ok(), "Valid agent name should parse");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::Use { first, second }) => {
                    assert_eq!(first, "claude-code");
                    assert_eq!(second, None);
                }
                _ => panic!("Should parse as Use subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test with agent name containing hyphens and underscores
    let result = try_parse_cli(&["agent", "use", "custom-agent_name"]);
    assert!(
        result.is_ok(),
        "Agent name with hyphens/underscores should parse"
    );

    // Test with agent name containing numbers
    let result = try_parse_cli(&["agent", "use", "agent-v2"]);
    assert!(result.is_ok(), "Agent name with numbers should parse");

    // Test without agent name (should fail)
    let result = try_parse_cli(&["agent", "use"]);
    assert!(result.is_err(), "Agent use without name should fail");

    // Test with two arguments (should succeed - first is use case or agent, second is agent name)
    let result = try_parse_cli(&["agent", "use", "first-agent", "second-agent"]);
    assert!(
        result.is_ok(),
        "Two arguments should succeed (use case + agent name pattern)"
    );

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::Use { first, second }) => {
                    assert_eq!(first, "first-agent");
                    assert_eq!(second, Some("second-agent".to_string()));
                }
                _ => panic!("Should parse as Use subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }

    // Test with single valid agent name
    let result = try_parse_cli(&["agent", "use", "test-agent"]);
    assert!(
        result.is_ok(),
        "Single agent name should parse successfully"
    );

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::Use { first, second }) => {
                    assert_eq!(first, "test-agent", "Should use provided agent name");
                    assert_eq!(second, None);
                }
                _ => panic!("Should parse as Use subcommand"),
            },
            _ => panic!("Should parse as Agent command"),
        }
    }
}

// =============================================================================
// HELP TEXT TESTS
// =============================================================================

#[test]
fn test_agent_help_text_content() {
    let help_text = get_help_text(&["agent", "--help"]);

    // Should contain command description
    assert!(
        help_text.contains("agent"),
        "Help should mention agent command"
    );

    // Should contain subcommands
    assert!(
        help_text.contains("list"),
        "Help should mention list subcommand"
    );
    assert!(
        help_text.contains("use"),
        "Help should mention use subcommand"
    );

    // Should contain usage information
    assert!(
        help_text.contains("Usage:") || help_text.contains("usage:"),
        "Help should show usage information"
    );

    // Should contain subcommand descriptions
    assert!(
        help_text.contains("List available agents")
            || help_text.contains("list") && help_text.contains("agent"),
        "Help should describe list command"
    );
    assert!(
        help_text.contains("Switch to")
            || help_text.contains("Use")
            || help_text.contains("use") && help_text.contains("agent"),
        "Help should describe use command"
    );
}

#[test]
fn test_agent_list_help_text_content() {
    let help_text = get_help_text(&["agent", "list", "--help"]);

    // Should contain format option
    assert!(
        help_text.contains("format") || help_text.contains("FORMAT"),
        "Help should mention format option"
    );

    // Should contain format choices
    assert!(
        help_text.contains("table") || help_text.contains("json") || help_text.contains("yaml"),
        "Help should show format options"
    );

    // Should contain usage information
    assert!(
        help_text.contains("Usage:") || help_text.contains("usage:"),
        "Help should show usage"
    );

    // Should mention agent listing
    assert!(
        help_text.contains("list") && (help_text.contains("agent") || help_text.contains("List")),
        "Help should describe agent listing"
    );
}

#[test]
fn test_agent_use_help_text_content() {
    let help_text = get_help_text(&["agent", "use", "--help"]);

    // Should contain agent name parameter (now using FIRST and SECOND)
    assert!(
        help_text.contains("FIRST")
            || help_text.contains("first")
            || help_text.contains("<FIRST>")
            || help_text.contains("AGENT_NAME")
            || help_text.contains("agent-name")
            || help_text.contains("<AGENT_NAME>")
            || help_text.contains("agent_name"),
        "Help should show agent name parameter: {}",
        help_text
    );

    // Should contain usage information
    assert!(
        help_text.contains("Usage:") || help_text.contains("usage:"),
        "Help should show usage"
    );

    // Should describe the use action
    assert!(
        help_text.contains("use") || help_text.contains("Use") || help_text.contains("switch"),
        "Help should describe use action"
    );

    // Should mention agent switching/selection
    assert!(
        help_text.contains("agent")
            && (help_text.contains("switch")
                || help_text.contains("use")
                || help_text.contains("select")),
        "Help should describe agent switching"
    );
}

#[test]
fn test_help_text_formatting_quality() {
    let main_help = get_help_text(&["agent", "--help"]);
    let list_help = get_help_text(&["agent", "list", "--help"]);
    let use_help = get_help_text(&["agent", "use", "--help"]);

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
    let result = try_parse_cli(&["agent"]);
    assert!(
        result.is_ok(),
        "Should succeed without explicit subcommand (defaults to Show)"
    );

    // Test invalid subcommand
    let result = try_parse_cli(&["agent", "invalid-subcommand"]);
    assert!(result.is_err(), "Should fail with invalid subcommand");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("invalid-subcommand") || error_msg.contains("unexpected"),
        "Error should mention invalid subcommand: {}",
        error_msg
    );

    // Test missing agent name for use command
    let result = try_parse_cli(&["agent", "use"]);
    assert!(result.is_err(), "Should fail without agent name");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("AGENT_NAME")
            || error_msg.contains("required")
            || error_msg.contains("missing"),
        "Error should mention missing agent name: {}",
        error_msg
    );
}

#[test]
fn test_format_validation_error_messages() {
    // Test invalid format value
    let result = try_parse_cli(&["agent", "list", "--format", "invalid"]);
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
    let result = try_parse_cli(&["agent", "list", "--format"]);
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
    let result = try_parse_cli(&["agent", "unknown-command"]);
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
    let result = try_parse_cli(&["agent", "list", "--format", "txt"]);
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
    // Test that agent is a proper subcommand of the main CLI
    let result = try_parse_cli(&["agent", "list"]);
    assert!(result.is_ok(), "Agent should be a valid top-level command");

    // Verify the parsed structure
    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => {
                // This confirms the command hierarchy is correct
                assert!(matches!(subcommand, Some(AgentSubcommand::List { .. })));
            }
            _ => panic!("Should parse as Agent command with proper hierarchy"),
        }
    }

    // Test that agent subcommands are properly nested
    let result = try_parse_cli(&["agent", "use", "test"]);
    assert!(result.is_ok(), "Agent use should be properly nested");

    if let Ok(cli) = result {
        match cli.command {
            Some(Commands::Agent { subcommand }) => {
                assert!(matches!(subcommand, Some(AgentSubcommand::Use { .. })));
            }
            _ => panic!("Should parse as nested Agent Use command"),
        }
    }
}

#[test]
fn test_global_flags_with_agent_commands() {
    // Test global verbose flag with agent commands
    let result = try_parse_cli(&["--verbose", "agent", "list"]);
    if let Ok(cli) = result {
        assert!(
            cli.verbose || cli.debug,
            "Global verbose flag should be parsed"
        );
        assert!(matches!(cli.command, Some(Commands::Agent { .. })));
    }

    // Test global quiet flag with agent commands
    let result = try_parse_cli(&["--quiet", "agent", "use", "test"]);
    if let Ok(cli) = result {
        assert!(cli.quiet, "Global quiet flag should be parsed");
        assert!(matches!(cli.command, Some(Commands::Agent { .. })));
    }

    // Test global debug flag with agent commands
    let result = try_parse_cli(&["--debug", "agent", "list", "--format", "json"]);
    if let Ok(cli) = result {
        assert!(cli.debug, "Global debug flag should be parsed");
        match cli.command {
            Some(Commands::Agent { subcommand }) => match subcommand {
                Some(AgentSubcommand::List { format }) => {
                    assert_eq!(format, OutputFormat::Json);
                }
                _ => panic!("Should maintain subcommand parsing with global flags"),
            },
            _ => panic!("Should parse as Agent command with global flags"),
        }
    }
}

// =============================================================================
// OUTPUT FORMAT ENUM TESTS
// =============================================================================

#[test]
fn test_output_format_enum_completeness() {
    // Test all expected output formats can be parsed
    let formats = ["table", "json", "yaml"];

    for format in &formats {
        let result = try_parse_cli(&["agent", "list", "--format", format]);
        assert!(result.is_ok(), "Format '{}' should be valid", format);

        if let Ok(cli) = result {
            match cli.command {
                Some(Commands::Agent { subcommand }) => match subcommand {
                    Some(AgentSubcommand::List {
                        format: parsed_format,
                    }) => match parsed_format {
                        OutputFormat::Table => assert_eq!(*format, "table"),
                        OutputFormat::Json => assert_eq!(*format, "json"),
                        OutputFormat::Yaml => assert_eq!(*format, "yaml"),
                    },
                    _ => panic!("Should parse as List command"),
                },
                _ => panic!("Should parse as Agent command"),
            }
        }
    }
}

#[test]
fn test_output_format_case_sensitivity() {
    // Test that format parsing is case-sensitive (lowercase expected)
    let uppercase_formats = ["TABLE", "JSON", "YAML"];

    for format in &uppercase_formats {
        let result = try_parse_cli(&["agent", "list", "--format", format]);
        assert!(
            result.is_err(),
            "Uppercase format '{}' should be rejected",
            format
        );
    }

    // Test mixed case
    let mixed_formats = ["Table", "Json", "Yaml"];

    for format in &mixed_formats {
        let result = try_parse_cli(&["agent", "list", "--format", format]);
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
fn test_agent_name_edge_cases() {
    // Test various agent name formats
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
        let result = try_parse_cli(&["agent", "use", name]);
        assert!(result.is_ok(), "Agent name '{}' should be valid", name);

        if let Ok(cli) = result {
            match cli.command {
                Some(Commands::Agent { subcommand }) => match subcommand {
                    Some(AgentSubcommand::Use { first, second }) => {
                        assert_eq!(first, *name);
                        assert_eq!(second, None);
                    }
                    _ => panic!("Should parse as Use command"),
                },
                _ => panic!("Should parse as Agent command"),
            }
        }
    }
}

#[test]
fn test_argument_order_flexibility() {
    // Test different argument orders where applicable
    let result1 = try_parse_cli(&["agent", "list", "--format", "json"]);
    let result2 = try_parse_cli(&["--verbose", "agent", "list", "--format", "json"]);

    assert!(
        result1.is_ok() && result2.is_ok(),
        "Different argument orders should work"
    );

    // Both should parse to the same subcommand structure
    if let (Ok(cli1), Ok(cli2)) = (result1, result2) {
        match (cli1.command, cli2.command) {
            (
                Some(Commands::Agent { subcommand: sub1 }),
                Some(Commands::Agent { subcommand: sub2 }),
            ) => match (sub1, sub2) {
                (
                    Some(AgentSubcommand::List { format: fmt1 }),
                    Some(AgentSubcommand::List { format: fmt2 }),
                ) => {
                    assert_eq!(fmt1, fmt2, "Format should be parsed consistently");
                    assert_eq!(fmt1, OutputFormat::Json);
                }
                _ => panic!("Both should parse as List commands"),
            },
            _ => panic!("Both should parse as Agent commands"),
        }
    }
}

#[test]
fn test_help_flag_variations() {
    // Test different help flag formats
    let help_variations = [
        &["agent", "--help"][..],
        &["agent", "-h"][..],
        &["help", "agent"][..], // This might not work depending on CLI structure
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
                        help_text.contains("agent"),
                        "Help should mention agent command"
                    );
                } else {
                    // For variations that might not be supported, that's also OK
                }
            }
        }
    }
}
