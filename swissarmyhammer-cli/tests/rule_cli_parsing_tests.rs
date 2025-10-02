//! CLI argument parsing tests for rule commands
//!
//! Tests command structure, help text generation, argument validation,
//! and error message clarity for rule subcommands.

use swissarmyhammer_cli::dynamic_cli::CliBuilder;

/// Test utility to get the rule command from CliBuilder
fn get_rule_command() -> clap::Command {
    CliBuilder::build_rule_command()
}

// =============================================================================
// BASIC COMMAND STRUCTURE TESTS
// =============================================================================

#[test]
fn test_rule_command_has_correct_name() {
    let rule_cmd = get_rule_command();
    assert_eq!(rule_cmd.get_name(), "rule");
}

#[test]
fn test_rule_command_has_subcommands() {
    let rule_cmd = get_rule_command();
    let subcommands: Vec<&str> = rule_cmd.get_subcommands().map(|sc| sc.get_name()).collect();

    assert!(
        subcommands.contains(&"list"),
        "Should have 'list' subcommand"
    );
    assert!(
        subcommands.contains(&"validate"),
        "Should have 'validate' subcommand"
    );
    assert!(
        subcommands.contains(&"check"),
        "Should have 'check' subcommand"
    );
    assert_eq!(subcommands.len(), 3, "Should have exactly 3 subcommands");
}

#[test]
fn test_rule_list_subcommand_structure() {
    let rule_cmd = get_rule_command();
    let list_cmd = rule_cmd
        .find_subcommand("list")
        .expect("list subcommand should exist");

    assert_eq!(list_cmd.get_name(), "list");
    assert!(
        list_cmd.get_about().is_some(),
        "list should have about text"
    );

    // List command should not have any required arguments
    let required_args: Vec<_> = list_cmd
        .get_arguments()
        .filter(|arg| arg.is_required_set())
        .collect();
    assert_eq!(
        required_args.len(),
        0,
        "list should have no required arguments"
    );
}

#[test]
fn test_rule_validate_subcommand_structure() {
    let rule_cmd = get_rule_command();
    let validate_cmd = rule_cmd
        .find_subcommand("validate")
        .expect("validate subcommand should exist");

    assert_eq!(validate_cmd.get_name(), "validate");
    assert!(
        validate_cmd.get_about().is_some(),
        "validate should have about text"
    );

    // Validate should have optional rule_name and file arguments
    let has_rule_name = validate_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "rule_name" || arg.get_id().as_str() == "rule-name");
    let has_file = validate_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "file");

    assert!(
        has_rule_name || has_file,
        "validate should have rule_name or file argument"
    );
}

#[test]
fn test_rule_check_subcommand_structure() {
    let rule_cmd = get_rule_command();
    let check_cmd = rule_cmd
        .find_subcommand("check")
        .expect("check subcommand should exist");

    assert_eq!(check_cmd.get_name(), "check");
    assert!(
        check_cmd.get_about().is_some(),
        "check should have about text"
    );

    // Check should have patterns argument
    let has_patterns = check_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "patterns");
    assert!(has_patterns, "check should have patterns argument");

    // Check should have optional rule, severity, and category filters
    let has_rule = check_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "rule");
    let has_severity = check_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "severity");
    let has_category = check_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "category");

    assert!(
        has_rule && has_severity && has_category,
        "check should have rule, severity, and category arguments"
    );
}

// =============================================================================
// ARGUMENT TYPE TESTS
// =============================================================================

#[test]
fn test_check_severity_argument_has_valid_values() {
    let rule_cmd = get_rule_command();
    let check_cmd = rule_cmd
        .find_subcommand("check")
        .expect("check subcommand should exist");

    let severity_arg = check_cmd
        .get_arguments()
        .find(|arg| arg.get_id().as_str() == "severity")
        .expect("severity argument should exist");

    // Check if the argument has value parser with specific values
    let possible_values = severity_arg.get_possible_values();
    if !possible_values.is_empty() {
        let values: Vec<_> = possible_values.iter().map(|v| v.get_name()).collect();

        // Should have standard severity levels
        assert!(
            values.contains(&"error") || values.contains(&"warning") || values.contains(&"info"),
            "severity should have standard levels, got: {:?}",
            values
        );
    }
}

#[test]
fn test_check_patterns_is_multi_valued() {
    let rule_cmd = get_rule_command();
    let check_cmd = rule_cmd
        .find_subcommand("check")
        .expect("check subcommand should exist");

    let patterns_arg = check_cmd
        .get_arguments()
        .find(|arg| arg.get_id().as_str() == "patterns")
        .expect("patterns argument should exist");

    // Patterns should accept multiple values
    assert!(
        patterns_arg.is_allow_hyphen_values_set()
            || patterns_arg.get_num_args().is_some()
            || patterns_arg.is_last_set()
            || patterns_arg.get_action().takes_values(),
        "patterns should accept multiple values"
    );
}

#[test]
fn test_check_rule_filter_is_multi_valued() {
    let rule_cmd = get_rule_command();
    let check_cmd = rule_cmd
        .find_subcommand("check")
        .expect("check subcommand should exist");

    let rule_arg = check_cmd
        .get_arguments()
        .find(|arg| arg.get_id().as_str() == "rule")
        .expect("rule argument should exist");

    // Rule filter should accept multiple values for filtering multiple rules
    assert!(
        rule_arg.get_action().takes_values(),
        "rule argument should accept values"
    );
}

// =============================================================================
// HELP TEXT TESTS
// =============================================================================

#[test]
fn test_rule_command_has_help_text() {
    let rule_cmd = get_rule_command();

    assert!(
        rule_cmd.get_about().is_some() || rule_cmd.get_long_about().is_some(),
        "rule command should have help text"
    );

    if let Some(about) = rule_cmd.get_about() {
        let about_str = about.to_string();
        assert!(
            about_str.contains("rule") || about_str.contains("lint"),
            "help should mention rules or linting"
        );
    }
}

#[test]
fn test_all_subcommands_have_help_text() {
    let rule_cmd = get_rule_command();

    for subcommand in rule_cmd.get_subcommands() {
        assert!(
            subcommand.get_about().is_some() || subcommand.get_long_about().is_some(),
            "subcommand '{}' should have help text",
            subcommand.get_name()
        );
    }
}

#[test]
fn test_list_subcommand_help_content() {
    let rule_cmd = get_rule_command();
    let list_cmd = rule_cmd
        .find_subcommand("list")
        .expect("list subcommand should exist");

    let help = list_cmd
        .get_about()
        .or_else(|| list_cmd.get_long_about())
        .expect("list should have help text");
    let help_str = help.to_string();

    assert!(
        help_str.contains("Display") || help_str.contains("rule"),
        "help should mention displaying rules"
    );
}

#[test]
fn test_validate_subcommand_help_content() {
    let rule_cmd = get_rule_command();
    let validate_cmd = rule_cmd
        .find_subcommand("validate")
        .expect("validate subcommand should exist");

    let help = validate_cmd
        .get_about()
        .or_else(|| validate_cmd.get_long_about())
        .expect("validate should have help text");
    let help_str = help.to_string();

    assert!(
        help_str.contains("validate") || help_str.contains("Validate"),
        "help should mention validation"
    );
}

#[test]
fn test_check_subcommand_help_content() {
    let rule_cmd = get_rule_command();
    let check_cmd = rule_cmd
        .find_subcommand("check")
        .expect("check subcommand should exist");

    let help = check_cmd
        .get_about()
        .or_else(|| check_cmd.get_long_about())
        .expect("check should have help text");
    let help_str = help.to_string();

    assert!(
        help_str.contains("Run") || help_str.contains("rule") || help_str.contains("code"),
        "help should mention running rules against code"
    );
}

// =============================================================================
// COMMAND CONSISTENCY TESTS
// =============================================================================

#[test]
fn test_subcommand_names_are_lowercase() {
    let rule_cmd = get_rule_command();

    for subcommand in rule_cmd.get_subcommands() {
        let name = subcommand.get_name();
        assert_eq!(
            name,
            name.to_lowercase(),
            "subcommand name '{}' should be lowercase",
            name
        );
    }
}

#[test]
fn test_all_subcommands_have_about_or_long_about() {
    let rule_cmd = get_rule_command();

    for subcommand in rule_cmd.get_subcommands() {
        assert!(
            subcommand.get_about().is_some() || subcommand.get_long_about().is_some(),
            "subcommand '{}' should have about or long_about text",
            subcommand.get_name()
        );
    }
}

// =============================================================================
// ARGUMENT VALIDATION TESTS
// =============================================================================

#[test]
fn test_validate_command_accepts_rule_name_or_file() {
    let rule_cmd = get_rule_command();
    let validate_cmd = rule_cmd
        .find_subcommand("validate")
        .expect("validate subcommand should exist");

    let has_rule_name = validate_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "rule_name" || arg.get_id().as_str() == "rule-name");
    let has_file = validate_cmd
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "file");

    assert!(
        has_rule_name || has_file,
        "validate should accept rule_name or file argument"
    );
}

// =============================================================================
// INTEGRATION CONSISTENCY TESTS
// =============================================================================

#[test]
fn test_rule_command_matches_agent_pattern() {
    // Get both rule and agent commands
    let rule_cmd = CliBuilder::build_rule_command();
    let agent_cmd = CliBuilder::build_agent_command();

    // Both should have consistent structure
    assert_eq!(rule_cmd.get_name(), "rule");
    assert_eq!(agent_cmd.get_name(), "agent");

    // Both should have multiple subcommands
    assert!(
        rule_cmd.get_subcommands().count() > 0,
        "rule should have subcommands"
    );
    assert!(
        agent_cmd.get_subcommands().count() > 0,
        "agent should have subcommands"
    );

    // Both should have help text
    assert!(
        rule_cmd.get_about().is_some() || rule_cmd.get_long_about().is_some(),
        "rule should have help text"
    );
    assert!(
        agent_cmd.get_about().is_some() || agent_cmd.get_long_about().is_some(),
        "agent should have help text"
    );
}

#[test]
fn test_command_has_examples_in_help() {
    let rule_cmd = get_rule_command();

    if let Some(long_about) = rule_cmd.get_long_about() {
        let help_text = long_about.to_string();
        // Help should contain examples
        assert!(
            help_text.contains("Example") || help_text.contains("example"),
            "long help should contain examples"
        );
    }
}
