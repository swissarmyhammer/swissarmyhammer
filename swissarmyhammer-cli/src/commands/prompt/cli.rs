//! Command line interface definitions for prompt commands
//!
//! This module provides clap command builders for the prompt subcommands,
//! using external markdown files for help text and strong typing for parsed arguments.

use clap::ArgMatches;

/// Error type for command parsing
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Unknown subcommand")]
    UnknownSubcommand,
}



/// Simplified list command structure - no filtering options
/// Uses global verbose/format from CliContext
#[derive(Debug)]
pub struct ListCommand {
    // No fields needed - uses global context
}

/// Test command structure with all current functionality preserved
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

/// Validate command structure
#[derive(Debug)]
pub struct ValidateCommand {
    // No fields needed for now - uses global context
}

/// Command enum wrapping all prompt subcommands
#[derive(Debug)]
pub enum PromptCommand {
    List(ListCommand),
    Test(TestCommand),
    Validate(ValidateCommand),
}

/// Parse clap matches into command structs
pub fn parse_prompt_command(matches: &ArgMatches) -> Result<PromptCommand, ParseError> {
    match matches.subcommand() {
        Some(("list", _sub_matches)) => Ok(PromptCommand::List(ListCommand {})),
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
            Ok(PromptCommand::Test(test_cmd))
        }
        Some(("validate", _sub_matches)) => Ok(PromptCommand::Validate(ValidateCommand {})),
        _ => Err(ParseError::UnknownSubcommand),
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
                    .arg(clap::Arg::new("var").long("var").action(clap::ArgAction::Append))
                    .arg(clap::Arg::new("raw").long("raw").action(clap::ArgAction::SetTrue))
                    .arg(clap::Arg::new("copy").long("copy").action(clap::ArgAction::SetTrue))
                    .arg(clap::Arg::new("save").long("save"))
                    .arg(clap::Arg::new("debug").long("debug").action(clap::ArgAction::SetTrue))
            )
            .try_get_matches_from(["prompt", "test", "help"])
            .unwrap()
    }

    #[test]
    fn test_parse_list_command() {
        let matches = create_mock_list_matches();
        let parsed = parse_prompt_command(&matches).unwrap();
        
        match parsed {
            PromptCommand::List(_) => (),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_parse_test_command_with_prompt_name() {
        let matches = create_mock_test_matches();
        let parsed = parse_prompt_command(&matches).unwrap();
        
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
    fn test_parse_unknown_subcommand() {
        let matches = Command::new("prompt")
            .try_get_matches_from(["prompt"])
            .unwrap();
        
        let result = parse_prompt_command(&matches);
        assert!(matches!(result, Err(ParseError::UnknownSubcommand)));
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
    fn test_validate_command_struct() {
        let validate_cmd = ValidateCommand {};
        
        match PromptCommand::Validate(validate_cmd) {
            PromptCommand::Validate(_) => (),
            _ => panic!("ValidateCommand should match PromptCommand::Validate"),
        }
    }

    #[test]
    fn test_parse_validate_command() {
        let matches = Command::new("prompt")
            .subcommand(Command::new("validate"))
            .try_get_matches_from(["prompt", "validate"])
            .unwrap();
        
        let parsed = parse_prompt_command(&matches).unwrap();
        
        match parsed {
            PromptCommand::Validate(_) => (),
            _ => panic!("Expected Validate command"),
        }
    }
}