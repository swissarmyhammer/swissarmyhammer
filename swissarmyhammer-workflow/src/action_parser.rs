//! Action parsing utilities for workflow state descriptions

use crate::actions::{
    AbortAction, ActionError, ActionResult, CelSetAction, LogAction, LogLevel, PromptAction,
    SetVariableAction, ShellAction, SubWorkflowAction, WaitAction,
};
use chumsky::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// Type alias for parser error type
type ParserError<'a> = extra::Err<Rich<'a, char>>;

/// Robust action parser using chumsky parser combinators
pub struct ActionParser;

impl ActionParser {
    /// Create a new action parser
    pub fn new() -> ActionResult<Self> {
        Ok(Self)
    }

    // Helper parsers

    /// Parse whitespace (spaces and tabs)
    fn whitespace<'a>() -> impl Parser<'a, &'a str, (), ParserError<'a>> {
        one_of(" \t").repeated().at_least(1).ignored()
    }

    /// Parse optional whitespace
    fn opt_whitespace<'a>() -> impl Parser<'a, &'a str, (), ParserError<'a>> {
        one_of(" \t").repeated().ignored()
    }

    /// Parse a case-insensitive word
    fn case_insensitive<'a>(word: &'static str) -> impl Parser<'a, &'a str, (), ParserError<'a>> {
        any()
            .filter(move |c: &char| c.is_alphabetic())
            .repeated()
            .exactly(word.len())
            .collect::<String>()
            .try_map(move |s, span| {
                if s.to_lowercase() == word.to_lowercase() {
                    Ok(())
                } else {
                    Err(Rich::custom(
                        span,
                        format!("expected '{word}', found '{s}'"),
                    ))
                }
            })
            .ignored()
    }

    /// Parse a quoted string
    fn quoted_string<'a>() -> impl Parser<'a, &'a str, String, ParserError<'a>> {
        just('"')
            .ignore_then(none_of('"').repeated().collect::<String>())
            .then_ignore(just('"'))
    }

    /// Parse an identifier (variable/argument name)
    fn identifier<'a>() -> impl Parser<'a, &'a str, String, ParserError<'a>> {
        any()
            .filter(|c: &char| c.is_alphabetic() || *c == '_')
            .then(
                any()
                    .filter(|c: &char| c.is_alphanumeric() || *c == '_')
                    .repeated()
                    .collect::<String>(),
            )
            .map(|(first, rest)| format!("{first}{rest}"))
    }

    /// Parse an argument key (allows hyphens)
    fn argument_key<'a>() -> impl Parser<'a, &'a str, String, ParserError<'a>> {
        any()
            .filter(|c: &char| c.is_alphabetic() || *c == '_')
            .then(
                any()
                    .filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .repeated()
                    .collect::<String>(),
            )
            .map(|(first, rest)| format!("{first}{rest}"))
    }

    /// Parse a prompt action from description
    /// Format: Execute prompt "prompt-name" with arg1="value1" arg2="value2"
    pub fn parse_prompt_action(&self, description: &str) -> ActionResult<Option<PromptAction>> {
        let parser = Self::case_insensitive("execute")
            .then_ignore(Self::whitespace())
            .then_ignore(Self::case_insensitive("prompt"))
            .then_ignore(Self::whitespace())
            .ignore_then(Self::quoted_string())
            .then(
                Self::whitespace()
                    .ignore_then(Self::case_insensitive("with"))
                    .ignore_then(Self::whitespace())
                    .then(
                        Self::argument_key()
                            .then_ignore(just('='))
                            .then(Self::quoted_string())
                            .separated_by(Self::whitespace())
                            .collect::<Vec<(String, String)>>(),
                    )
                    .map(|(_, args)| args)
                    .or_not(),
            );

        match parser.parse(description.trim()).into_result() {
            Ok((prompt_name, args)) => {
                let mut action = PromptAction::new(prompt_name);

                if let Some(arguments) = args {
                    for (key, value) in arguments {
                        if key == "result" {
                            action = action.with_result_variable(value);
                        } else {
                            if !self.is_valid_argument_key(&key) {
                                return Err(ActionError::ParseError(
                                    format!("Invalid argument key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                                ));
                            }
                            action.arguments.insert(key, value);
                        }
                    }
                }

                Ok(Some(action))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a wait action from description
    /// Format: Wait for user confirmation OR Wait 30 seconds
    pub fn parse_wait_action(&self, description: &str) -> ActionResult<Option<WaitAction>> {
        // Parser for "Wait for user" is now handled by string contains check below

        // Parser for duration units
        let duration_unit = choice((
            Self::case_insensitive("seconds")
                .to("seconds".to_string())
                .or(Self::case_insensitive("second").to("second".to_string())),
            Self::case_insensitive("minutes")
                .to("minutes".to_string())
                .or(Self::case_insensitive("minute").to("minute".to_string())),
            Self::case_insensitive("hours")
                .to("hours".to_string())
                .or(Self::case_insensitive("hour").to("hour".to_string())),
            Self::case_insensitive("sec").to("sec".to_string()),
            Self::case_insensitive("min").to("min".to_string()),
            just("h").to("h".to_string()),
            just("s").to("s".to_string()),
            just("m").to("m".to_string()),
        ));

        // Parser for "Wait N units"
        let wait_duration = Self::case_insensitive("wait")
            .then_ignore(Self::whitespace())
            .ignore_then(text::int(10).from_str::<u64>().unwrapped())
            .then_ignore(Self::whitespace())
            .then(duration_unit)
            .then(
                Self::whitespace()
                    .then(any().repeated().collect::<String>())
                    .map(|(_, msg)| msg)
                    .or_not(),
            );

        // Try to parse as user wait first
        let lower_desc = description.to_lowercase();
        if lower_desc.contains("wait for user") {
            return Ok(Some(
                WaitAction::new_user_input().with_message(description.to_string()),
            ));
        }

        // Try to parse as duration wait
        match wait_duration.parse(description.trim()).into_result() {
            Ok(((duration_value, unit), message)) => {
                let duration = self.parse_duration_unit(duration_value, &unit)?;
                let mut action = WaitAction::new_duration(duration);

                if let Some(msg) = message {
                    action = action.with_message(msg);
                }

                Ok(Some(action))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a log action from description
    /// Format: Log "message" OR Log error "message"
    pub fn parse_log_action(&self, description: &str) -> ActionResult<Option<LogAction>> {
        let log_level = choice((
            Self::case_insensitive("error").to(LogLevel::Error),
            Self::case_insensitive("warning").to(LogLevel::Warning),
        ));

        let parser = Self::case_insensitive("log")
            .then_ignore(Self::whitespace())
            .ignore_then(log_level.then_ignore(Self::whitespace()).or_not())
            .then(Self::quoted_string());

        match parser.parse(description.trim()).into_result() {
            Ok((level, message)) => {
                let level = level.unwrap_or(LogLevel::Info);
                Ok(Some(LogAction::new(message, level)))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a set variable action from description
    /// Supports both formats:
    /// - New format: set_variable variable_name="value"
    /// - Legacy format: Set variable_name="value"
    pub fn parse_set_variable_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<SetVariableAction>> {
        // Try new format first
        if let Some(action) = self.parse_set_variable_format_action(description)? {
            return Ok(Some(action));
        }

        // Fall back to legacy format
        let value_parser = choice((
            Self::quoted_string(),
            none_of('"').repeated().at_least(1).collect::<String>(),
        ));

        let parser = Self::case_insensitive("set")
            .then_ignore(Self::whitespace())
            .ignore_then(Self::identifier())
            .then_ignore(Self::opt_whitespace())
            .then_ignore(just('='))
            .then_ignore(Self::opt_whitespace())
            .then(value_parser);

        match parser.parse(description.trim()).into_result() {
            Ok((var_name, value)) => {
                // Validate variable name
                if !self.is_valid_variable_name(&var_name) {
                    return Err(ActionError::ParseError(
                        format!("Invalid variable name '{var_name}': must start with letter or underscore and contain only alphanumeric characters and underscores")
                    ));
                }

                Ok(Some(SetVariableAction::new(var_name, value)))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a set variable action with new format from description
    /// Format: set_variable variable_name="value"
    pub fn parse_set_variable_format_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<SetVariableAction>> {
        let value_parser = choice((
            Self::quoted_string(),
            none_of('"').repeated().at_least(1).collect::<String>(),
        ));

        let parser = just("set_variable")
            .ignored()
            .then_ignore(Self::whitespace())
            .ignore_then(Self::identifier())
            .then_ignore(Self::opt_whitespace())
            .then_ignore(just('='))
            .then_ignore(Self::opt_whitespace())
            .then(value_parser);

        match parser.parse(description.trim()).into_result() {
            Ok((var_name, value)) => {
                // Validate variable name
                if !self.is_valid_variable_name(&var_name) {
                    return Err(ActionError::ParseError(
                        format!("Invalid variable name '{var_name}': must start with letter or underscore and contain only alphanumeric characters and underscores")
                    ));
                }

                Ok(Some(SetVariableAction::new(var_name, value)))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a cel_set action from description
    /// Format: cel_set variable_name value
    pub fn parse_cel_set_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<CelSetAction>> {
        let value_parser = choice((
            Self::quoted_string(),
            none_of('"').repeated().at_least(1).collect::<String>(),
        ));

        let parser = Self::case_insensitive("cel_set")
            .then_ignore(Self::whitespace())
            .ignore_then(Self::identifier())
            .then_ignore(Self::whitespace())
            .then(value_parser);

        match parser.parse(description.trim()).into_result() {
            Ok((var_name, value)) => {
                // Validate variable name
                if !self.is_valid_variable_name(&var_name) {
                    return Err(ActionError::ParseError(
                        format!("Invalid variable name '{var_name}': must start with letter or underscore and contain only alphanumeric characters and underscores")
                    ));
                }

                Ok(Some(CelSetAction::new(var_name, value)))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse an abort action from description
    /// Format: Abort "error message" OR Abort with message "error message"
    pub fn parse_abort_action(&self, description: &str) -> ActionResult<Option<AbortAction>> {
        // Parser for simple "Abort" followed by quoted message
        let simple_abort = Self::case_insensitive("abort")
            .then_ignore(Self::whitespace())
            .ignore_then(Self::quoted_string());

        // Parser for "Abort with message" format
        let abort_with_message = Self::case_insensitive("abort")
            .then_ignore(Self::whitespace())
            .then_ignore(Self::case_insensitive("with"))
            .then_ignore(Self::whitespace())
            .then_ignore(Self::case_insensitive("message"))
            .then_ignore(Self::whitespace())
            .ignore_then(Self::quoted_string());

        // Try both formats
        let parser = choice((abort_with_message, simple_abort));

        match parser.parse(description.trim()).into_result() {
            Ok(message) => Ok(Some(AbortAction::new(message))),
            Err(_) => Ok(None),
        }
    }

    /// Parse a sub-workflow action from description
    /// Format: Run workflow "workflow-name" with input1="value1" input2="value2"
    /// Format: Delegate to "workflow-name" with input="${data}"
    pub fn parse_sub_workflow_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<SubWorkflowAction>> {
        // Parser for "Run workflow" or "Delegate to"
        let run_workflow = Self::case_insensitive("run")
            .then_ignore(Self::whitespace())
            .then_ignore(Self::case_insensitive("workflow"));

        let delegate_to = Self::case_insensitive("delegate")
            .then_ignore(Self::whitespace())
            .then_ignore(Self::case_insensitive("to").or_not());

        let workflow_prefix = choice((run_workflow.to(()), delegate_to.to(())));

        // Parser for single input format
        let single_input = Self::case_insensitive("input")
            .then_ignore(just('='))
            .then(choice((
                Self::quoted_string(),
                none_of(' ').repeated().at_least(1).collect::<String>(),
            )));

        // Parser for arguments
        let argument_parser = Self::argument_key()
            .then_ignore(just('='))
            .then(Self::quoted_string())
            .separated_by(Self::whitespace())
            .collect::<Vec<(String, String)>>();

        let parser = workflow_prefix
            .then_ignore(Self::whitespace())
            .ignore_then(Self::quoted_string())
            .then(
                Self::whitespace()
                    .then_ignore(Self::case_insensitive("with"))
                    .then_ignore(Self::whitespace())
                    .then(choice((
                        single_input.map(|(_, value)| vec![("input".to_string(), value)]),
                        argument_parser,
                    )))
                    .map(|(_, args)| args)
                    .or_not(),
            );

        match parser.parse(description.trim()).into_result() {
            Ok((workflow_name, args)) => {
                let mut action = SubWorkflowAction::new(workflow_name);

                if let Some(arguments) = args {
                    for (key, value) in arguments {
                        if key == "result" {
                            action = action.with_result_variable(value);
                        } else if key == "timeout" {
                            // Timeout is no longer supported - ignore
                        } else {
                            if !self.is_valid_argument_key(&key) {
                                return Err(ActionError::ParseError(
                                    format!("Invalid input variable key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                                ));
                            }
                            action.input_variables.insert(key, value);
                        }
                    }
                }

                Ok(Some(action))
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse a shell action from description
    /// Format: Shell "command" [with timeout=N] [result="variable"] [working_dir="path"] [env={"KEY": "value"}]
    pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>> {
        // Parse individual parameters
        let timeout_parser = Self::argument_key()
            .filter(|key| key == "timeout")
            .then_ignore(just('='))
            .ignore_then(
                any()
                    .filter(|c: &char| c.is_alphanumeric())
                    .repeated()
                    .at_least(1)
                    .collect::<String>(),
            )
            .map(|v| ("timeout".to_string(), v));

        let result_parser = Self::argument_key()
            .filter(|key| key == "result")
            .then_ignore(just('='))
            .ignore_then(Self::quoted_string())
            .map(|v| ("result".to_string(), v));

        let working_dir_parser = Self::argument_key()
            .filter(|key| key == "working_dir" || key == "working-dir")
            .then_ignore(just('='))
            .ignore_then(Self::quoted_string())
            .map(|v| ("working_dir".to_string(), v));

        // Parse environment as a special case - capture everything between braces
        let env_as_param = just("env")
            .then_ignore(just('='))
            .ignore_then(
                just('{')
                    .ignore_then(none_of('}').repeated().collect::<String>())
                    .then_ignore(just('}')),
            )
            .map(|content| ("env".to_string(), format!("{{{content}}}")));

        // Generic parameter parser for unknown parameters
        let generic_param_parser = Self::argument_key().then_ignore(just('=')).then(choice((
            Self::quoted_string(),
            // For env parameters or other complex values, capture everything until whitespace or end
            none_of(' ').repeated().at_least(1).collect::<String>(),
        )));

        // Combine all parameter parsers
        let param_parser = choice((
            timeout_parser,
            result_parser,
            working_dir_parser,
            env_as_param,
            generic_param_parser,
        ));

        // Main parser for shell action
        let parser = Self::case_insensitive("shell")
            .then_ignore(Self::whitespace())
            .ignore_then(Self::quoted_string())
            .then(
                Self::whitespace()
                    .ignore_then(Self::case_insensitive("with"))
                    .ignore_then(Self::whitespace())
                    .ignore_then(
                        param_parser
                            .separated_by(Self::whitespace())
                            .collect::<Vec<(String, String)>>(),
                    )
                    .or_not(),
            );

        match parser.parse(description.trim()).into_result() {
            Ok((command, params)) => {
                // Validate command is not empty
                if command.is_empty() {
                    return Err(ActionError::ParseError(
                        "Shell command cannot be empty".to_string(),
                    ));
                }

                let mut action = ShellAction::new(command);
                let mut env_vars = HashMap::new();

                if let Some(parameters) = params {
                    for (key, value) in parameters {
                        match key.as_str() {
                            "timeout" => {
                                let timeout_value = value.parse::<u64>().map_err(|_| {
                                    ActionError::ParseError(format!(
                                        "Invalid timeout value: {value}"
                                    ))
                                })?;

                                if timeout_value == 0 {
                                    return Err(ActionError::ParseError(
                                        "Timeout must be greater than 0".to_string(),
                                    ));
                                }

                                action = action.with_timeout(Duration::from_secs(timeout_value));
                            }
                            "result" => {
                                if !self.is_valid_variable_name(&value) {
                                    return Err(ActionError::ParseError(
                                        format!("Invalid result variable name '{value}': must start with letter or underscore and contain only alphanumeric characters and underscores")
                                    ));
                                }
                                action = action.with_result_variable(value);
                            }
                            "working_dir" => {
                                if value.is_empty() {
                                    return Err(ActionError::ParseError(
                                        "Working directory cannot be empty".to_string(),
                                    ));
                                }
                                action = action.with_working_dir(value);
                            }
                            "env" => {
                                // Parse the JSON environment variables
                                let json_value: serde_json::Value = serde_json::from_str(&value)
                                    .map_err(|e| {
                                        ActionError::ParseError(format!(
                                            "Invalid environment variables JSON: {e}"
                                        ))
                                    })?;

                                if let serde_json::Value::Object(obj) = json_value {
                                    for (key, val) in obj {
                                        if let serde_json::Value::String(string_val) = val {
                                            env_vars.insert(key, string_val);
                                        } else {
                                            return Err(ActionError::ParseError(
                                                format!("Environment variable values must be strings, found: {val:?}")
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(ActionError::ParseError(
                                        "Environment variables must be specified as a JSON object"
                                            .to_string(),
                                    ));
                                }
                            }
                            _ => {
                                return Err(ActionError::ParseError(format!(
                                    "Unknown shell action parameter: {key}"
                                )));
                            }
                        }
                    }
                }

                if !env_vars.is_empty() {
                    action = action.with_environment(env_vars);
                }

                Ok(Some(action))
            }
            Err(_) => Ok(None),
        }
    }

    /// Safely substitute variables in a string using regex
    pub fn substitute_variables_safe(
        &self,
        input: &str,
        context: &HashMap<String, Value>,
    ) -> ActionResult<String> {
        let var_regex = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_.-]*)\}").map_err(|e| {
            ActionError::ParseError(format!("Failed to compile variable regex: {e}"))
        })?;

        let result = var_regex.replace_all(input, |caps: &regex::Captures| {
            let var_name = &caps[1];
            context
                .get(var_name)
                .map(|v| self.value_to_string(v))
                .unwrap_or_else(|| format!("${{{var_name}}}"))
        });

        Ok(result.into_owned())
    }

    /// Parse duration unit string into Duration
    fn parse_duration_unit(&self, value: u64, unit: &str) -> ActionResult<Duration> {
        match unit {
            "second" | "seconds" | "sec" | "s" => Ok(Duration::from_secs(value)),
            "minute" | "minutes" | "min" | "m" => Ok(Duration::from_secs(value * 60)),
            "hour" | "hours" | "h" => Ok(Duration::from_secs(value * 3600)),
            _ => Err(ActionError::ParseError(format!(
                "Invalid duration unit: {unit}"
            ))),
        }
    }

    /// Parse timeout value from string (supports formats like "100ms", "5s", "2m", etc.)
    /// Validate that an argument key is safe for command-line use
    fn is_valid_argument_key(&self, key: &str) -> bool {
        !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    }

    /// Validate that a variable name is valid
    fn is_valid_variable_name(&self, name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
    }

    /// Convert a JSON Value to a string representation
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(_) | Value::Object(_) => value.to_string(),
        }
    }
}

impl Default for ActionParser {
    fn default() -> Self {
        Self::new().expect("Failed to create default ActionParser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt_action() {
        let parser = ActionParser::new().unwrap();

        // Test basic prompt
        let action = parser
            .parse_prompt_action("Execute prompt \"analyze-code\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.prompt_name, "analyze-code");
        assert!(action.arguments.is_empty());

        // Test prompt with arguments
        let action = parser
            .parse_prompt_action(
                "Execute prompt \"analyze-code\" with file=\"test.rs\" verbose=\"true\"",
            )
            .unwrap()
            .unwrap();
        assert_eq!(action.prompt_name, "analyze-code");
        assert_eq!(action.arguments.get("file"), Some(&"test.rs".to_string()));
        assert_eq!(action.arguments.get("verbose"), Some(&"true".to_string()));

        // Test invalid format
        let result = parser.parse_prompt_action("Execute prompt analyze-code");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_wait_action() {
        let parser = ActionParser::new().unwrap();

        // Test user input wait
        let result = parser
            .parse_wait_action("Wait for user confirmation")
            .unwrap();
        assert!(
            result.is_some(),
            "Failed to parse 'Wait for user confirmation'"
        );
        let action = result.unwrap();
        assert!(action.duration.is_none());

        // Test duration wait
        let action = parser
            .parse_wait_action("Wait 30 seconds")
            .unwrap()
            .unwrap();
        assert_eq!(action.duration, Some(Duration::from_secs(30)));

        // Test duration with different units
        let action = parser.parse_wait_action("Wait 5 minutes").unwrap().unwrap();
        assert_eq!(action.duration, Some(Duration::from_secs(300)));

        // Test invalid format
        let result = parser.parse_wait_action("Wait invalid");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_log_action() {
        let parser = ActionParser::new().unwrap();

        // Test info log
        let action = parser
            .parse_log_action("Log \"Hello world\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Hello world");
        assert!(matches!(action.level, LogLevel::Info));

        // Test error log
        let action = parser
            .parse_log_action("Log error \"Something failed\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Something failed");
        assert!(matches!(action.level, LogLevel::Error));

        // Test warning log
        let action = parser
            .parse_log_action("Log warning \"Be careful\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Be careful");
        assert!(matches!(action.level, LogLevel::Warning));
    }

    #[test]
    fn test_parse_set_variable_action() {
        let parser = ActionParser::new().unwrap();

        // Test basic set
        let action = parser
            .parse_set_variable_action("Set result=\"success\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "result");
        assert_eq!(action.value, "success");

        // Test with variable substitution
        let action = parser
            .parse_set_variable_action("Set output=\"${claude_response}\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "output");
        assert_eq!(action.value, "${claude_response}");

        // Test new set_variable format
        let action = parser
            .parse_set_variable_action("set_variable result=\"success\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "result");
        assert_eq!(action.value, "success");

        // Test new format with variable substitution
        let action = parser
            .parse_set_variable_action("set_variable output=\"${claude_response}\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "output");
        assert_eq!(action.value, "${claude_response}");

        // Test new format with underscore in variable name
        let action = parser
            .parse_set_variable_action("set_variable user_name=\"John Doe\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "user_name");
        assert_eq!(action.value, "John Doe");

        // Test new format invalid variable name
        let result = parser.parse_set_variable_action("set_variable 123invalid=\"value\"");
        assert!(result.unwrap().is_none());

        // Test invalid variable name
        let result = parser.parse_set_variable_action("Set 123invalid=\"value\"");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_set_variable_format_action() {
        let parser = ActionParser::new().unwrap();

        // Test basic set_variable format
        let action = parser
            .parse_set_variable_format_action("set_variable result=\"success\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "result");
        assert_eq!(action.value, "success");

        // Test with complex value containing spaces
        let action = parser
            .parse_set_variable_format_action("set_variable message=\"Hello World from Claude\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "message");
        assert_eq!(action.value, "Hello World from Claude");

        // Test with variable substitution
        let action = parser
            .parse_set_variable_format_action("set_variable output=\"${previous_result}\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "output");
        assert_eq!(action.value, "${previous_result}");

        // Test with underscore in variable name
        let action = parser
            .parse_set_variable_format_action("set_variable user_data=\"John Doe\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "user_data");
        assert_eq!(action.value, "John Doe");

        // Test invalid variable name starting with number
        let result = parser.parse_set_variable_format_action("set_variable 123invalid=\"value\"");
        assert!(result.unwrap().is_none());

        // Test wrong command
        let result = parser.parse_set_variable_format_action("set result=\"value\"");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_variable_substitution() {
        let parser = ActionParser::new().unwrap();
        let mut context = HashMap::new();
        context.insert("file".to_string(), Value::String("test.rs".to_string()));
        context.insert("count".to_string(), Value::Number(42.into()));

        let result = parser
            .substitute_variables_safe("Process ${file} with ${count} items", &context)
            .unwrap();
        assert_eq!(result, "Process test.rs with 42 items");

        // Test with missing variable
        let result = parser
            .substitute_variables_safe("Process ${missing} file", &context)
            .unwrap();
        assert_eq!(result, "Process ${missing} file");
    }

    #[test]
    fn test_parse_sub_workflow_action() {
        let parser = ActionParser::new().unwrap();

        // Test "Run workflow" format
        let action = parser
            .parse_sub_workflow_action("Run workflow \"validation-workflow\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.workflow_name, "validation-workflow");
        assert!(action.input_variables.is_empty());

        // Test "Run workflow" with arguments
        let action = parser
            .parse_sub_workflow_action(
                "Run workflow \"analyze-code\" with file=\"test.rs\" mode=\"strict\"",
            )
            .unwrap()
            .unwrap();
        assert_eq!(action.workflow_name, "analyze-code");
        assert_eq!(
            action.input_variables.get("file"),
            Some(&"test.rs".to_string())
        );
        assert_eq!(
            action.input_variables.get("mode"),
            Some(&"strict".to_string())
        );

        // Test "Delegate to" format
        let action = parser
            .parse_sub_workflow_action("Delegate to \"validation-workflow\" with input=\"${data}\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.workflow_name, "validation-workflow");
        assert_eq!(
            action.input_variables.get("input"),
            Some(&"${data}".to_string())
        );

        // Test case insensitive
        let action = parser
            .parse_sub_workflow_action("run workflow \"test-workflow\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.workflow_name, "test-workflow");

        // Test invalid format
        let result = parser.parse_sub_workflow_action("Run workflow test-workflow");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_abort_action() {
        let parser = ActionParser::new().unwrap();

        // Test simple abort format
        let action = parser
            .parse_abort_action("Abort \"Test error message\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Test error message");

        // Test abort with message format
        let action = parser
            .parse_abort_action("Abort with message \"Critical error occurred\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Critical error occurred");

        // Test case insensitive
        let action = parser
            .parse_abort_action("abort \"test error\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "test error");

        // Test invalid format
        let result = parser.parse_abort_action("Abort test error");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_shell_action_basic() {
        let parser = ActionParser::new().unwrap();

        // Test basic shell command
        let action = parser
            .parse_shell_action("Shell \"echo hello\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "echo hello");
        assert!(action.timeout.is_none());
        assert!(action.result_variable.is_none());
        assert!(action.working_dir.is_none());
        assert!(action.environment.is_empty());

        // Test case insensitive
        let action = parser.parse_shell_action("shell \"pwd\"").unwrap().unwrap();
        assert_eq!(action.command, "pwd");
    }

    #[test]
    fn test_parse_shell_action_with_timeout() {
        let parser = ActionParser::new().unwrap();

        // Test with timeout
        let action = parser
            .parse_shell_action("Shell \"ls -la\" with timeout=30")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "ls -la");
        assert_eq!(action.timeout, Some(Duration::from_secs(30)));

        // Test invalid timeout (zero)
        let result = parser.parse_shell_action("Shell \"echo test\" with timeout=0");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Timeout must be greater than 0"));

        // Test invalid timeout (non-numeric)
        let result = parser.parse_shell_action("Shell \"echo test\" with timeout=abc");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid timeout value"));
    }

    #[test]
    fn test_parse_shell_action_with_result() {
        let parser = ActionParser::new().unwrap();

        // Test with result variable
        let action = parser
            .parse_shell_action("Shell \"git status\" with result=\"status_output\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "git status");
        assert_eq!(action.result_variable, Some("status_output".to_string()));

        // Test invalid result variable name (starts with number)
        let result = parser.parse_shell_action("Shell \"echo test\" with result=\"123invalid\"");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid result variable name"));

        // Test invalid result variable name (contains special chars)
        let result = parser.parse_shell_action("Shell \"echo test\" with result=\"invalid-name\"");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid result variable name"));
    }

    #[test]
    fn test_parse_shell_action_with_working_dir() {
        let parser = ActionParser::new().unwrap();

        // Test with working directory
        let action = parser
            .parse_shell_action("Shell \"ls\" with working_dir=\"/tmp\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "ls");
        assert_eq!(action.working_dir, Some("/tmp".to_string()));

        // Test empty working directory
        let result = parser.parse_shell_action("Shell \"ls\" with working_dir=\"\"");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Working directory cannot be empty"));
    }

    #[test]
    fn test_parse_shell_action_with_environment() {
        let parser = ActionParser::new().unwrap();

        // Test with environment variables
        let action = parser
            .parse_shell_action(
                "Shell \"env\" with env={\"PATH\":\"/usr/bin\",\"USER\":\"testuser\"}",
            )
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "env");
        assert_eq!(action.environment.len(), 2);
        assert_eq!(
            action.environment.get("PATH"),
            Some(&"/usr/bin".to_string())
        );
        assert_eq!(
            action.environment.get("USER"),
            Some(&"testuser".to_string())
        );

        // Test invalid JSON environment
        let result = parser.parse_shell_action("Shell \"env\" with env={invalid json}");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid environment variables JSON"));

        // Test non-string values in environment JSON
        let result = parser.parse_shell_action("Shell \"env\" with env={\"NUM_VAR\":123}");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Environment variable values must be strings"));

        // Test non-object JSON for environment
        let result = parser.parse_shell_action("Shell \"env\" with env=[\"array\"]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Environment variables must be specified as a JSON object"));
    }

    #[test]
    fn test_parse_shell_action_combined_parameters() {
        let parser = ActionParser::new().unwrap();

        // Test multiple parameters combined
        let action = parser
            .parse_shell_action("Shell \"cargo build\" with timeout=120 result=\"build_output\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "cargo build");
        assert_eq!(action.timeout, Some(Duration::from_secs(120)));
        assert_eq!(action.result_variable, Some("build_output".to_string()));

        // Test all parameters combined
        let action = parser
            .parse_shell_action("Shell \"make\" with timeout=300 result=\"make_output\" working_dir=\"./project\" env={\"CC\":\"gcc\"}")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "make");
        assert_eq!(action.timeout, Some(Duration::from_secs(300)));
        assert_eq!(action.result_variable, Some("make_output".to_string()));
        assert_eq!(action.working_dir, Some("./project".to_string()));
        assert_eq!(action.environment.get("CC"), Some(&"gcc".to_string()));
    }

    #[test]
    fn test_parse_shell_action_validation() {
        let parser = ActionParser::new().unwrap();

        // Test empty command
        let result = parser.parse_shell_action("Shell \"\"");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Shell command cannot be empty"));

        // Test unknown parameter
        let result = parser.parse_shell_action("Shell \"echo test\" with unknown_param=\"value\"");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown shell action parameter"));
    }

    #[test]
    fn test_parse_shell_action_invalid_formats() {
        let parser = ActionParser::new().unwrap();

        // Test invalid format (no quotes)
        let result = parser.parse_shell_action("Shell echo hello");
        assert!(result.unwrap().is_none());

        // Test invalid format (not shell command)
        let result = parser.parse_shell_action("Execute \"echo hello\"");
        assert!(result.unwrap().is_none());

        // Test invalid format (missing with keyword)
        let result = parser.parse_shell_action("Shell \"echo hello\" timeout=30");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_shell_action_edge_cases() {
        let parser = ActionParser::new().unwrap();

        // Test command with spaces and special characters
        let action = parser
            .parse_shell_action("Shell \"echo 'hello world' && ls -la\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.command, "echo 'hello world' && ls -la");

        // Test valid variable names
        let action = parser
            .parse_shell_action("Shell \"echo test\" with result=\"_valid_var_123\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.result_variable, Some("_valid_var_123".to_string()));

        // Test working directory with path
        let action = parser
            .parse_shell_action("Shell \"ls\" with working_dir=\"/home/user/projects/app\"")
            .unwrap()
            .unwrap();
        assert_eq!(
            action.working_dir,
            Some("/home/user/projects/app".to_string())
        );
    }
}
