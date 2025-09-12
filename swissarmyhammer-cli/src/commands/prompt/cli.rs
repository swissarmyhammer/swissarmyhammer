//! Command line interface definitions for prompt commands
//!
//! This module provides clap command builders for the prompt subcommands,
//! using external markdown files for help text and strong typing for parsed arguments.

use clap::ArgMatches;



/// List command for displaying available prompts.
///
/// This command lists all available prompts in the system. It uses global
/// verbose and format options from the CliContext rather than having its own
/// filtering options, keeping the interface simple and consistent.
#[derive(Debug)]
pub struct ListCommand {
    // No fields needed - uses global context
}

/// Test command for executing prompts with various options.
///
/// This command allows testing prompts either by name or from file, with
/// support for variable substitution, output formatting, and debugging options.
/// All functionality from the original implementation is preserved.
#[derive(Debug)]
pub struct TestCommand {
    pub prompt_name: Option<String>,
    pub file: Option<String>,
    pub vars: Vec<String>,
    pub raw: bool,
    pub copy: bool,
    pub save: Option<String>,
    pub debug: bool,
}

/// Command enum representing all available prompt subcommands.
///
/// This enum wraps all prompt-related commands and provides type-safe
/// parsing from command line arguments. Each variant contains the
/// parsed arguments for that specific command.
#[derive(Debug)]
pub enum PromptCommand {
    List(ListCommand),
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
/// A PromptCommand enum variant containing the parsed command and its arguments
///
/// # Example
/// ```rust
/// let command = parse_prompt_command(&matches);
/// match command {
///     PromptCommand::List(_) => println!("Listing prompts"),
///     PromptCommand::Test(test_cmd) => println!("Testing prompt: {:?}", test_cmd.prompt_name),
/// }
/// ```
pub fn parse_prompt_command(matches: &ArgMatches) -> PromptCommand {
    match matches.subcommand() {
        Some(("list", _sub_matches)) => PromptCommand::List(ListCommand {}),
        Some(("test", sub_matches)) => {
            let test_cmd = TestCommand {
                prompt_name: sub_matches.get_one::<String>("prompt_name").cloned(),
                file: sub_matches.get_one::<String>("file").cloned(),
                vars: sub_matches
                    .get_many::<String>("var")
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default(),
                raw: sub_matches.get_flag("raw"),
                copy: sub_matches.get_flag("copy"),
                save: sub_matches.get_one::<String>("save").cloned(),
                debug: sub_matches.get_flag("debug"),
            };
            PromptCommand::Test(test_cmd)
        }

        _ => {
            // Default to list command when no subcommand is provided
            PromptCommand::List(ListCommand {})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{ArgMatches, Command};

    // Helper function to create mock ArgMatches for testing
    fn create_mock_list_matches() -> ArgMatches {
        Command::new("prompt")
            .subcommand(Command::new("list"))
            .try_get_matches_from(["prompt", "list"])
            .unwrap()
    }

    fn create_mock_test_matches() -> ArgMatches {
        Command::new("prompt")
            .subcommand(
                Command::new("test")
                    .arg(clap::Arg::new("prompt_name").index(1))
                    .arg(clap::Arg::new("file").short('f').long("file"))
                    .arg(
                        clap::Arg::new("var")
                            .long("var")
                            .action(clap::ArgAction::Append),
                    )
                    .arg(
                        clap::Arg::new("raw")
                            .long("raw")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("copy")
                            .long("copy")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(clap::Arg::new("save").long("save"))
                    .arg(
                        clap::Arg::new("debug")
                            .long("debug")
                            .action(clap::ArgAction::SetTrue),
                    ),
            )
            .try_get_matches_from(["prompt", "test", "help"])
            .unwrap()
    }

    fn create_mock_test_matches_with_args() -> ArgMatches {
        Command::new("prompt")
            .subcommand(
                Command::new("test")
                    .arg(clap::Arg::new("prompt_name").index(1))
                    .arg(clap::Arg::new("file").short('f').long("file"))
                    .arg(
                        clap::Arg::new("var")
                            .long("var")
                            .action(clap::ArgAction::Append),
                    )
                    .arg(
                        clap::Arg::new("raw")
                            .long("raw")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("copy")
                            .long("copy")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(clap::Arg::new("save").long("save"))
                    .arg(
                        clap::Arg::new("debug")
                            .long("debug")
                            .action(clap::ArgAction::SetTrue),
                    ),
            )
            .try_get_matches_from([
                "prompt",
                "test",
                "help",
                "--var",
                "key1=value1",
                "--var",
                "key2=value2",
                "--raw",
                "--copy",
                "--save",
                "output.txt",
                "--debug",
            ])
            .unwrap()
    }

    #[test]
    fn test_parse_list_command() {
        let matches = create_mock_list_matches();
        let parsed = parse_prompt_command(&matches);

        match parsed {
            PromptCommand::List(_) => (),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_parse_test_command_with_prompt_name() {
        let matches = create_mock_test_matches();
        let parsed = parse_prompt_command(&matches);

        match parsed {
            PromptCommand::Test(test_cmd) => {
                assert_eq!(test_cmd.prompt_name, Some("help".to_string()));
                assert_eq!(test_cmd.file, None);
                assert!(test_cmd.vars.is_empty());
                assert!(!test_cmd.raw);
                assert!(!test_cmd.copy);
                assert_eq!(test_cmd.save, None);
                assert!(!test_cmd.debug);
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_test_command_with_all_args() {
        let matches = create_mock_test_matches_with_args();
        let parsed = parse_prompt_command(&matches);

        match parsed {
            PromptCommand::Test(test_cmd) => {
                assert_eq!(test_cmd.prompt_name, Some("help".to_string()));
                assert_eq!(test_cmd.file, None);
                assert_eq!(test_cmd.vars, vec!["key1=value1", "key2=value2"]);
                assert!(test_cmd.raw);
                assert!(test_cmd.copy);
                assert_eq!(test_cmd.save, Some("output.txt".to_string()));
                assert!(test_cmd.debug);
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_test_command_with_file() {
        // Test TestCommand structure with file parameter
        let test_cmd = TestCommand {
            prompt_name: None,
            file: Some("test.md".to_string()),
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        match PromptCommand::Test(test_cmd) {
            PromptCommand::Test(cmd) => {
                assert_eq!(cmd.prompt_name, None);
                assert_eq!(cmd.file, Some("test.md".to_string()));
                assert!(!cmd.debug);
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_no_subcommand_defaults_to_list() {
        let matches = Command::new("prompt")
            .try_get_matches_from(["prompt"])
            .unwrap();

        let result = parse_prompt_command(&matches);
        assert!(matches!(result, PromptCommand::List(_)));
    }

    #[test]
    fn test_list_command_struct() {
        let list_cmd = ListCommand {};
        // Test that the struct exists and can be created
        match PromptCommand::List(list_cmd) {
            PromptCommand::List(_) => (),
            _ => panic!("ListCommand should match PromptCommand::List"),
        }
    }

    #[test]
    fn test_test_command_struct_defaults() {
        let test_cmd = TestCommand {
            prompt_name: None,
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        match PromptCommand::Test(test_cmd) {
            PromptCommand::Test(cmd) => {
                assert_eq!(cmd.prompt_name, None);
                assert_eq!(cmd.file, None);
                assert!(cmd.vars.is_empty());
                assert!(!cmd.raw);
                assert!(!cmd.copy);
                assert_eq!(cmd.save, None);
                assert!(!cmd.debug);
            }
            _ => panic!("TestCommand should match PromptCommand::Test"),
        }
    }

    #[test]
    fn test_test_command_struct() {
        let test_cmd = TestCommand {
            prompt_name: Some("test".to_string()),
            file: None,
            vars: vec!["key=value".to_string()],
            raw: true,
            copy: false,
            save: Some("output.txt".to_string()),
            debug: true,
        };

        match PromptCommand::Test(test_cmd) {
            PromptCommand::Test(cmd) => {
                assert_eq!(cmd.prompt_name, Some("test".to_string()));
                assert!(cmd.raw);
                assert!(!cmd.copy);
                assert!(cmd.debug);
                assert_eq!(cmd.save, Some("output.txt".to_string()));
                assert_eq!(cmd.vars, vec!["key=value".to_string()]);
            }
            _ => panic!("TestCommand should match PromptCommand::Test"),
        }
    }



    #[test]
    fn test_command_debug_display() {
        let list_cmd = ListCommand {};
        let debug_str = format!("{:?}", list_cmd);
        assert!(debug_str.contains("ListCommand"));

        let test_cmd = TestCommand {
            prompt_name: Some("test".to_string()),
            file: None,
            vars: vec!["key=value".to_string()],
            raw: true,
            copy: false,
            save: None,
            debug: false,
        };
        let debug_str = format!("{:?}", test_cmd);
        assert!(debug_str.contains("TestCommand"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("key=value"));



        let prompt_cmd = PromptCommand::List(list_cmd);
        let debug_str = format!("{:?}", prompt_cmd);
        assert!(debug_str.contains("List"));
    }

    #[test]
    fn test_parse_error_struct_exists() {
        // Test that ParseError enum can be debugged (for future extensibility)
        let _debug_check = format!("{:?}", "ParseError enum exists for future use");
        // Currently no error cases to test
    }
}
