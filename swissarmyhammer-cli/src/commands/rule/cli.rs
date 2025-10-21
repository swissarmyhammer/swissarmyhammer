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
    pub create_issues: bool,
    pub no_fail_fast: bool,
    pub force: bool,
    /// Maximum number of ERROR violations to return. Defaults to 1 for fast feedback
    /// and incremental error fixing. Use higher values to see more errors at once.
    pub max_errors: Option<usize>,
}

/// Cache command for managing the rule evaluation cache.
///
/// This command provides operations for managing the cache of rule evaluation results.
#[derive(Debug)]
pub struct CacheCommand {
    pub action: CacheAction,
}

/// Actions available for cache management
#[derive(Debug)]
pub enum CacheAction {
    /// Clear all cache entries
    Clear,
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
    Cache(CacheCommand),
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
                create_issues: sub_matches.get_flag("create-issues"),
                no_fail_fast: sub_matches.get_flag("no-fail-fast"),
                force: sub_matches.get_flag("force"),
                max_errors: sub_matches.get_one::<usize>("max-errors").copied(),
            };
            RuleCommand::Check(check_cmd)
        }
        Some(("cache", sub_matches)) => {
            match sub_matches.subcommand() {
                Some(("clear", _)) => RuleCommand::Cache(CacheCommand {
                    action: CacheAction::Clear,
                }),
                _ => {
                    // Default to list when no valid cache subcommand
                    RuleCommand::List(ListCommand {})
                }
            }
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize)),
                    ),
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
                assert!(!check_cmd.create_issues);
                assert!(!check_cmd.no_fail_fast);
                assert!(!check_cmd.force);
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize)),
                    ),
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
                assert!(!check_cmd.create_issues);
                assert!(!check_cmd.no_fail_fast);
                assert!(!check_cmd.force);
            }
            _ => panic!("Expected Check command"),
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
            create_issues: false,
            no_fail_fast: false,
            force: false,
            max_errors: None,
        };

        match RuleCommand::Check(check_cmd) {
            RuleCommand::Check(cmd) => {
                assert_eq!(cmd.patterns, vec!["file1.rs", "file2.rs"]);
                assert_eq!(cmd.rule, Some(vec!["test-rule".to_string()]));
                assert_eq!(cmd.severity, Some("error".to_string()));
                assert_eq!(cmd.category, Some("security".to_string()));
                assert!(!cmd.create_issues);
                assert!(!cmd.no_fail_fast);
                assert!(!cmd.force);
                assert_eq!(cmd.max_errors, None);
            }
            _ => panic!("CheckCommand should match RuleCommand::Check"),
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

    #[test]
    fn test_parse_check_command_with_create_issues() {
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize)),
                    ),
            )
            .try_get_matches_from(["rule", "check", "--create-issues", "file.rs"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert!(check_cmd.create_issues);
                assert!(!check_cmd.no_fail_fast);
                assert!(!check_cmd.force);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_check_command_with_no_fail_fast() {
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize)),
                    ),
            )
            .try_get_matches_from(["rule", "check", "--no-fail-fast", "file.rs"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert!(!check_cmd.create_issues);
                assert!(check_cmd.no_fail_fast);
                assert!(!check_cmd.force);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_check_command_with_both_flags() {
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize)),
                    ),
            )
            .try_get_matches_from([
                "rule",
                "check",
                "--create-issues",
                "--no-fail-fast",
                "file.rs",
            ])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert!(check_cmd.create_issues);
                assert!(check_cmd.no_fail_fast);
                assert!(!check_cmd.force);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_check_command_max_errors_defaults_to_one() {
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize))
                            .default_value("1"),
                    ),
            )
            .try_get_matches_from(["rule", "check", "file.rs"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert_eq!(
                    check_cmd.max_errors,
                    Some(1),
                    "max_errors should default to 1 when not specified"
                );
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_parse_check_command_max_errors_explicit_value() {
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
                    .arg(Arg::new("category").short('c').long("category"))
                    .arg(
                        Arg::new("create-issues")
                            .long("create-issues")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("no-fail-fast")
                            .long("no-fail-fast")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(
                        Arg::new("max-errors")
                            .long("max-errors")
                            .value_parser(clap::value_parser!(usize))
                            .default_value("1"),
                    ),
            )
            .try_get_matches_from(["rule", "check", "--max-errors", "10", "file.rs"])
            .unwrap();

        let parsed = parse_rule_command(&matches);
        match parsed {
            RuleCommand::Check(check_cmd) => {
                assert_eq!(check_cmd.patterns, vec!["file.rs"]);
                assert_eq!(
                    check_cmd.max_errors,
                    Some(10),
                    "max_errors should use the explicitly provided value"
                );
            }
            _ => panic!("Expected Check command"),
        }
    }
}
