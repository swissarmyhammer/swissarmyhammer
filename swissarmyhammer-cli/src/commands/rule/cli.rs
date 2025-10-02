//! Command line interface definitions for rule commands
//!
//! This module provides clap command builders for the rule subcommands,
//! using external markdown files for help text and strong typing for parsed arguments.

use clap::ArgMatches;

/// List command for displaying available rules.
///
/// This command lists all available rules in the system. It uses global
/// verbose and format options from the CliContext rather than having its own
/// filtering options, keeping the interface simple and consistent.
#[derive(Debug)]
pub struct ListCommand {
    // No fields needed - uses global context
}

/// Validate command for checking rule syntax and structure.
///
/// This command validates rule files to ensure they have correct syntax,
/// valid frontmatter, and proper template structure.
#[derive(Debug)]
pub struct ValidateCommand {
    pub rule_name: Option<String>,
    pub file: Option<String>,
}

/// Check command for running rules against code.
///
/// This command checks code files against specified rules or all applicable rules,
/// reporting violations and errors.
#[derive(Debug)]
pub struct CheckCommand {
    pub patterns: Vec<String>,
    pub rule: Option<Vec<String>>,
    pub severity: Option<String>,
    pub category: Option<String>,
}

/// Test command for testing rules with sample code.
///
/// This command allows testing rules with sample code snippets to verify
/// rule behavior and expected violations.
#[derive(Debug)]
pub struct TestCommand {
    pub rule_name: String,
    pub file: Option<String>,
    pub code: Option<String>,
}

/// Command enum representing all available rule subcommands.
///
/// This enum wraps all rule-related commands and provides type-safe
/// parsing from command line arguments. Each variant contains the
/// parsed arguments for that specific command.
#[derive(Debug)]
pub enum RuleCommand {
    List(ListCommand),
    Validate(ValidateCommand),
    Check(CheckCommand),
    Test(TestCommand),
}

/// Parse clap matches into strongly-typed command structs.
///
/// This is the single parsing function used by both production and test code.
/// It parses command line arguments from clap's ArgMatches into type-safe
/// command structures. Defaults to the list command when no subcommand is provided.
///
/// # Arguments
/// * `matches` - The ArgMatches from clap containing parsed command line arguments
///
/// # Returns
/// A RuleCommand enum variant containing the parsed command and its arguments
///
/// # Example
/// ```rust
/// let command = parse_rule_command(&matches);
/// match command {
///     RuleCommand::List(_) => println!("Listing rules"),
///     RuleCommand::Check(check_cmd) => println!("Checking files: {:?}", check_cmd.files),
/// }
/// ```
pub fn parse_rule_command(matches: &ArgMatches) -> RuleCommand {
    match matches.subcommand() {
        Some(("list", _sub_matches)) => RuleCommand::List(ListCommand {}),
        Some(("validate", sub_matches)) => {
            let validate_cmd = ValidateCommand {
                rule_name: sub_matches.get_one::<String>("rule").cloned(),
                file: sub_matches.get_one::<String>("file").cloned(),
            };
            RuleCommand::Validate(validate_cmd)
        }
        Some(("check", sub_matches)) => {
            let check_cmd = CheckCommand {
                patterns: sub_matches
                    .get_many::<String>("patterns")
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default(),
                rule: sub_matches
                    .get_many::<String>("rule")
                    .map(|vals| vals.cloned().collect()),
                severity: sub_matches.get_one::<String>("severity").cloned(),
                category: sub_matches.get_one::<String>("category").cloned(),
            };
            RuleCommand::Check(check_cmd)
        }
        Some(("test", sub_matches)) => {
            let test_cmd = TestCommand {
                rule_name: sub_matches
                    .get_one::<String>("rule_name")
                    .cloned()
                    .expect("rule_name is required"),
                file: sub_matches.get_one::<String>("file").cloned(),
                code: sub_matches.get_one::<String>("code").cloned(),
            };
            RuleCommand::Test(test_cmd)
        }
        _ => {
            // Default to list command when no subcommand is provided
            RuleCommand::List(ListCommand {})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};

    #[test]
    fn test_parse_list_command() {
        let matches = Command::new("rule")
            .subcommand(Command::new("list"))
            .try_get_matches_from(["rule", "list"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::List(_) => (),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_parse_validate_command_with_rule_name() {
        let matches = Command::new("rule")
            .subcommand(
                Command::new("validate")
                    .arg(Arg::new("rule").long("rule").value_name("NAME"))
                    .arg(Arg::new("file").long("file").value_name("FILE")),
            )
            .try_get_matches_from(["rule", "validate", "--rule", "my-rule"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Validate(validate_cmd) => {
                assert_eq!(validate_cmd.rule_name, Some("my-rule".to_string()));
                assert_eq!(validate_cmd.file, None);
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_parse_validate_command_with_file() {
        let matches = Command::new("rule")
            .subcommand(
                Command::new("validate")
                    .arg(Arg::new("rule").long("rule").value_name("NAME"))
                    .arg(Arg::new("file").long("file").value_name("FILE")),
            )
            .try_get_matches_from(["rule", "validate", "--file", "rule.md"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Validate(validate_cmd) => {
                assert_eq!(validate_cmd.rule_name, None);
                assert_eq!(validate_cmd.file, Some("rule.md".to_string()));
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_parse_check_command() {
        let matches = Command::new("rule")
            .subcommand(
                Command::new("check")
                    .arg(
                        Arg::new("rule")
                            .short('r')
                            .long("rule")
                            .action(ArgAction::Append),
                    )
                    .arg(Arg::new("patterns").action(ArgAction::Append))
                    .arg(Arg::new("severity").short('s').long("severity"))
                    .arg(Arg::new("category").short('c').long("category")),
            )
            .try_get_matches_from(["rule", "check", "file1.rs", "file2.rs"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.rule, None);
                assert_eq!(check_cmd.patterns, vec!["file1.rs", "file2.rs"]);
                assert_eq!(check_cmd.severity, None);
                assert_eq!(check_cmd.category, None);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_check_command_with_filters() {
        let matches = Command::new("rule")
            .subcommand(
                Command::new("check")
                    .arg(
                        Arg::new("rule")
                            .short('r')
                            .long("rule")
                            .action(ArgAction::Append),
                    )
                    .arg(Arg::new("patterns").action(ArgAction::Append))
                    .arg(Arg::new("severity").short('s').long("severity"))
                    .arg(Arg::new("category").short('c').long("category")),
            )
            .try_get_matches_from([
                "rule",
                "check",
                "--severity",
                "error",
                "--category",
                "security",
                "file.rs",
            ])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert_eq!(check_cmd.severity, Some("error".to_string()));
                assert_eq!(check_cmd.category, Some("security".to_string()));
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_test_command() {
        let matches = Command::new("rule")
            .subcommand(
                Command::new("test")
                    .arg(Arg::new("rule_name").index(1).required(true))
                    .arg(Arg::new("file").short('f').long("file"))
                    .arg(Arg::new("code").short('c').long("code")),
            )
            .try_get_matches_from(["rule", "test", "my-rule"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Test(test_cmd) => {
                assert_eq!(test_cmd.rule_name, "my-rule");
                assert_eq!(test_cmd.file, None);
                assert_eq!(test_cmd.code, None);
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_no_subcommand_defaults_to_list() {
        let matches = Command::new("rule").try_get_matches_from(["rule"]).unwrap();

        let result = parse_rule_command(&matches);
        assert!(matches!(result, RuleCommand::List(_)));
    }

    #[test]
    fn test_list_command_struct() {
        let list_cmd = ListCommand {};
        match RuleCommand::List(list_cmd) {
            RuleCommand::List(_) => (),
            _ => panic!("ListCommand should match RuleCommand::List"),
        }
    }

    #[test]
    fn test_validate_command_struct() {
        let validate_cmd = ValidateCommand {
            rule_name: Some("test-rule".to_string()),
            file: None,
        };

        match RuleCommand::Validate(validate_cmd) {
            RuleCommand::Validate(cmd) => {
                assert_eq!(cmd.rule_name, Some("test-rule".to_string()));
                assert_eq!(cmd.file, None);
            }
            _ => panic!("ValidateCommand should match RuleCommand::Validate"),
        }
    }

    #[test]
    fn test_check_command_struct() {
        let check_cmd = CheckCommand {
            patterns: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            rule: Some(vec!["test-rule".to_string()]),
            severity: Some("error".to_string()),
            category: Some("security".to_string()),
        };

        match RuleCommand::Check(check_cmd) {
            RuleCommand::Check(cmd) => {
                assert_eq!(cmd.patterns, vec!["file1.rs", "file2.rs"]);
                assert_eq!(cmd.rule, Some(vec!["test-rule".to_string()]));
                assert_eq!(cmd.severity, Some("error".to_string()));
                assert_eq!(cmd.category, Some("security".to_string()));
            }
            _ => panic!("CheckCommand should match RuleCommand::Check"),
        }
    }

    #[test]
    fn test_test_command_struct() {
        let test_cmd = TestCommand {
            rule_name: "test-rule".to_string(),
            file: Some("test.rs".to_string()),
            code: None,
        };

        match RuleCommand::Test(test_cmd) {
            RuleCommand::Test(cmd) => {
                assert_eq!(cmd.rule_name, "test-rule");
                assert_eq!(cmd.file, Some("test.rs".to_string()));
                assert_eq!(cmd.code, None);
            }
            _ => panic!("TestCommand should match RuleCommand::Test"),
        }
    }

    #[test]
    fn test_command_debug_display() {
        let list_cmd = ListCommand {};
        let debug_str = format!("{:?}", list_cmd);
        assert!(debug_str.contains("ListCommand"));

        let validate_cmd = ValidateCommand {
            rule_name: Some("test".to_string()),
            file: None,
        };
        let debug_str = format!("{:?}", validate_cmd);
        assert!(debug_str.contains("ValidateCommand"));
        assert!(debug_str.contains("test"));

        let rule_cmd = RuleCommand::List(list_cmd);
        let debug_str = format!("{:?}", rule_cmd);
        assert!(debug_str.contains("List"));
    }
}
