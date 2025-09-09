//! Action parsing utilities for workflow state descriptions
//!
//! TEMPORARY IMPLEMENTATION: This is a simplified stub that provides the same
//! interface as the original ActionParser but uses basic string parsing instead
//! of chumsky combinators to resolve compilation issues.

use crate::actions::{
    AbortAction, ActionError, ActionResult, LogAction, LogLevel, PromptAction, SetVariableAction,
    ShellAction, SubWorkflowAction, WaitAction,
};

use std::collections::HashMap;
use std::time::Duration;

/// Simplified action parser using regex instead of chumsky
/// TODO: Restore chumsky-based implementation once API compatibility is resolved
pub struct ActionParser;

impl ActionParser {
    /// Create a new action parser
    pub fn new() -> ActionResult<Self> {
        Ok(Self)
    }

    /// Parse action from description
    #[allow(dead_code)] // Used in tests
    pub fn parse(&self, description: &str) -> ActionResult<Box<dyn crate::actions::Action>> {
        let description = description.trim();

        // Simple pattern matching for basic actions
        if description.starts_with("shell:") || description.starts_with("SHELL:") {
            let command = description[6..].trim();
            Ok(Box::new(ShellAction::new(command.to_string())))
        } else if description.starts_with("log:") || description.starts_with("LOG:") {
            let message = description[4..].trim();
            Ok(Box::new(LogAction::new(
                message.to_string(),
                LogLevel::Info,
            )))
        } else if description.starts_with("prompt:") || description.starts_with("PROMPT:") {
            let prompt = description[7..].trim();
            Ok(Box::new(PromptAction::new(prompt.to_string())))
        } else if description.starts_with("wait:") || description.starts_with("WAIT:") {
            let duration_str = description[5..].trim();
            if let Ok(seconds) = duration_str.parse::<u64>() {
                Ok(Box::new(WaitAction::new_duration(Duration::from_secs(
                    seconds,
                ))))
            } else {
                Ok(Box::new(WaitAction::new_user_input()))
            }
        } else if description.starts_with("set:") || description.starts_with("SET:") {
            let rest = description[4..].trim();
            if let Some((key, value)) = rest.split_once('=') {
                Ok(Box::new(SetVariableAction::new(
                    key.trim().to_string(),
                    value.trim().to_string(),
                )))
            } else {
                Err(ActionError::ParseError(format!(
                    "Invalid set action format: {}",
                    description
                )))
            }
        } else if description.starts_with("subworkflow:") || description.starts_with("SUBWORKFLOW:")
        {
            let workflow_name = description[12..].trim();
            Ok(Box::new(SubWorkflowAction::new(workflow_name.to_string())))
        } else if description.starts_with("abort:") || description.starts_with("ABORT:") {
            let message = description[6..].trim();
            Ok(Box::new(AbortAction::new(message.to_string())))
        } else {
            // Default to shell action for backward compatibility
            Ok(Box::new(ShellAction::new(description.to_string())))
        }
    }

    /// Parse action from description with context
    #[allow(dead_code)] // Used in tests
    pub fn parse_with_context(
        &self,
        description: &str,
        _context: &crate::WorkflowTemplateContext,
    ) -> ActionResult<Box<dyn crate::actions::Action>> {
        // For now, just delegate to basic parse
        // TODO: Implement proper context handling
        self.parse(description)
    }

    /// Get action type from description
    #[allow(dead_code)] // Used in tests
    pub fn get_action_type(&self, description: &str) -> ActionResult<String> {
        let description = description.trim().to_lowercase();

        if description.starts_with("shell:") {
            Ok("shell".to_string())
        } else if description.starts_with("log:") {
            Ok("log".to_string())
        } else if description.starts_with("prompt:") {
            Ok("prompt".to_string())
        } else if description.starts_with("wait:") {
            Ok("wait".to_string())
        } else if description.starts_with("set:") {
            Ok("set".to_string())
        } else if description.starts_with("subworkflow:") {
            Ok("subworkflow".to_string())
        } else if description.starts_with("abort:") {
            Ok("abort".to_string())
        } else {
            Ok("shell".to_string()) // Default
        }
    }

    /// Parse shell action specifically (backward compatibility method)
    pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>> {
        let description = description.trim();

        // Parse shell actions with optional parameters
        // Use regex to handle variable whitespace between "shell" and the command
        let shell_regex = regex::Regex::new(r#"(?i)^shell\s+"([^"]*)"(.*)"#)
            .map_err(|e| ActionError::ParseError(format!("Failed to create shell regex: {}", e)))?;

        if let Some(captures) = shell_regex.captures(description) {
            let command = captures.get(1).unwrap().as_str();
            let params_str = captures.get(2).map(|m| m.as_str()).unwrap_or("");

            let mut action = ShellAction::new(command.to_string());

            // Parse parameters if present
            if params_str.trim_start().starts_with("with ") {
                let params_part = params_str.trim_start();
                let params_part = &params_part[5..]; // Remove "with "

                // Check for malformed parameters (key= with no value) - these make the command unparseable
                let malformed_param_regex = regex::Regex::new(r"(\w+)=$").map_err(|e| {
                    ActionError::ParseError(format!(
                        "Failed to create malformed param regex: {}",
                        e
                    ))
                })?;

                if malformed_param_regex.is_match(params_part) {
                    return Ok(None); // Malformed parameters make the command unparseable
                }

                self.parse_shell_parameters(&mut action, params_part)?;
            }

            Ok(Some(action))
        } else {
            // Invalid syntax or not a shell command
            Ok(None)
        }
    }

    fn parse_shell_parameters(
        &self,
        action: &mut ShellAction,
        params_str: &str,
    ) -> ActionResult<()> {
        // Parse timeout=30 result="files" working_dir="/tmp" env_var="value" etc.
        let timeout_regex = regex::Regex::new(r"timeout=(-?\d+)").map_err(|e| {
            ActionError::ParseError(format!("Failed to create timeout regex: {}", e))
        })?;

        let result_regex = regex::Regex::new(r#"result="([^"]*)"#).map_err(|e| {
            ActionError::ParseError(format!("Failed to create result regex: {}", e))
        })?;

        let working_dir_regex = regex::Regex::new(r#"working_dir="([^"]*)"#).map_err(|e| {
            ActionError::ParseError(format!("Failed to create working_dir regex: {}", e))
        })?;

        let env_regex = regex::Regex::new(r#"(\w+)="([^"]*)"#)
            .map_err(|e| ActionError::ParseError(format!("Failed to create env regex: {}", e)))?;

        let unquoted_param_regex = regex::Regex::new(r"(\w+)=([^\s]+)").map_err(|e| {
            ActionError::ParseError(format!("Failed to create unquoted param regex: {}", e))
        })?;

        let env_json_regex = regex::Regex::new(r#"env=(\{[^}]+\})"#).map_err(|e| {
            ActionError::ParseError(format!("Failed to create env json regex: {}", e))
        })?;

        // Check for malformed timeout parameters (timeout=non-numeric)
        let malformed_timeout_regex = regex::Regex::new(r"timeout=([^\s]+)").map_err(|e| {
            ActionError::ParseError(format!("Failed to create malformed timeout regex: {}", e))
        })?;

        if let Some(malformed_match) = malformed_timeout_regex.captures(params_str) {
            let timeout_value = malformed_match.get(1).unwrap().as_str();
            // Check if it's not a valid number
            if timeout_value.parse::<i64>().is_err() {
                return Err(ActionError::ParseError(format!(
                    "Invalid timeout value: {}",
                    timeout_value
                )));
            }
        }

        // Parse timeout parameters
        if let Some(timeout_match) = timeout_regex.captures(params_str) {
            let timeout_str = timeout_match.get(1).unwrap().as_str();

            match timeout_str.parse::<i64>() {
                Ok(val) if val <= 0 => {
                    return Err(ActionError::ParseError(
                        "Timeout must be greater than 0".to_string(),
                    ));
                }
                Ok(timeout_secs) => {
                    action.timeout = Some(std::time::Duration::from_secs(timeout_secs as u64));
                }
                Err(_) => {
                    return Err(ActionError::ParseError(format!(
                        "Invalid timeout value: {}",
                        timeout_str
                    )));
                }
            }
        }

        // Validate and parse result variable
        if let Some(result_match) = result_regex.captures(params_str) {
            let result_var = result_match.get(1).unwrap().as_str();

            // Validate variable name
            if result_var.is_empty() {
                return Err(ActionError::ParseError(
                    "Result variable name cannot be empty".to_string(),
                ));
            }

            // Check if variable name is valid (starts with letter or underscore, contains only alphanumeric and underscore)
            if !result_var.chars().next().unwrap_or('0').is_alphabetic()
                && !result_var.starts_with('_')
            {
                return Err(ActionError::ParseError(format!(
                    "Invalid result variable name: {}",
                    result_var
                )));
            }

            if !result_var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(ActionError::ParseError(format!(
                    "Invalid result variable name: {}",
                    result_var
                )));
            }

            action.result_variable = Some(result_var.to_string());
        }

        // Validate and parse working directory
        if let Some(working_dir_match) = working_dir_regex.captures(params_str) {
            let working_dir = working_dir_match.get(1).unwrap().as_str();

            if working_dir.is_empty() {
                return Err(ActionError::ParseError(
                    "Working directory cannot be empty".to_string(),
                ));
            }

            action.working_dir = Some(working_dir.to_string());
        }

        // Parse JSON-formatted environment variables first (env={"key": "value"})
        if let Some(env_json_match) = env_json_regex.captures(params_str) {
            let json_str = env_json_match.get(1).unwrap().as_str();
            match serde_json::from_str::<std::collections::HashMap<String, String>>(json_str) {
                Ok(env_map) => {
                    for (key, value) in env_map {
                        action.environment.insert(key, value);
                    }
                }
                Err(e) => {
                    return Err(ActionError::ParseError(format!(
                        "Invalid JSON in environment variables: {}",
                        e
                    )));
                }
            }
        }

        // Parse and validate all parameter names (both quoted and unquoted)
        let known_params = ["timeout", "result", "working_dir", "env"];

        // Check quoted parameters
        for cap in env_regex.captures_iter(params_str) {
            let key = cap.get(1).unwrap().as_str();

            if !known_params.contains(&key) {
                return Err(ActionError::ParseError(format!(
                    "Unknown parameter: {}",
                    key
                )));
            }
        }

        // Check unquoted parameters
        for cap in unquoted_param_regex.captures_iter(params_str) {
            let key = cap.get(1).unwrap().as_str();

            if !known_params.contains(&key) {
                return Err(ActionError::ParseError(format!(
                    "Unknown parameter: {}",
                    key
                )));
            }
        }

        Ok(())
    }

    /// Parse prompt action (backward compatibility method)
    pub fn parse_prompt_action(&self, description: &str) -> ActionResult<Option<PromptAction>> {
        let description = description.trim();
        let lower = description.to_lowercase();

        if lower.starts_with("execute prompt ") {
            // Parse "Execute prompt" pattern - more sophisticated parsing needed
            if let Some(action) = self.parse_execute_prompt_pattern(description)? {
                return Ok(Some(action));
            }
        } else if lower.starts_with("prompt ") {
            let prompt = &description[7..];
            return Ok(Some(PromptAction::new(prompt.to_string())));
        }

        Ok(None)
    }

    fn parse_execute_prompt_pattern(
        &self,
        description: &str,
    ) -> ActionResult<Option<PromptAction>> {
        // Parse "Execute prompt "name" with arg1="value1" arg2="value2""
        let regex = regex::Regex::new(r#"^Execute prompt "([^"]+)"(.*)"#)
            .map_err(|e| ActionError::ParseError(format!("Failed to create regex: {}", e)))?;

        if let Some(captures) = regex.captures(description) {
            let prompt_name = captures.get(1).unwrap().as_str();
            let args_str = captures.get(2).map(|m| m.as_str()).unwrap_or("");

            let mut action = PromptAction::new(prompt_name.to_string());

            // Parse arguments if present
            if let Some(args_part) = args_str.strip_prefix(" with ") {
                action.arguments = self.parse_arguments(args_part)?;
            }

            Ok(Some(action))
        } else {
            Ok(None)
        }
    }

    fn parse_arguments(
        &self,
        args_str: &str,
    ) -> ActionResult<std::collections::HashMap<String, String>> {
        let mut args = std::collections::HashMap::new();

        // Simple parsing of key="value" pairs
        let regex = regex::Regex::new(r#"(\w+)="([^"]*)"#)
            .map_err(|e| ActionError::ParseError(format!("Failed to create args regex: {}", e)))?;

        for capture in regex.captures_iter(args_str) {
            let key = capture.get(1).unwrap().as_str().to_string();
            let value = capture.get(2).unwrap().as_str().to_string();
            args.insert(key, value);
        }

        Ok(args)
    }

    /// Strip surrounding quotes from a string
    fn strip_quotes<'a>(&self, s: &'a str) -> &'a str {
        if s.len() >= 2
            && ((s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('\'') && s.ends_with('\'')))
        {
            &s[1..s.len() - 1]
        } else {
            s
        }
    }

    /// Parse wait action (backward compatibility method)
    pub fn parse_wait_action(&self, description: &str) -> ActionResult<Option<WaitAction>> {
        let description = description.trim();
        let lower = description.to_lowercase();

        if lower.starts_with("wait ") {
            let rest = &description[5..].trim();

            // Handle "Wait 30 seconds" pattern
            if rest.ends_with(" seconds") || rest.ends_with(" second") {
                let number_part = if let Some(stripped) = rest.strip_suffix(" seconds") {
                    stripped
                } else if let Some(stripped) = rest.strip_suffix(" second") {
                    stripped
                } else {
                    unreachable!()
                };

                if let Ok(seconds) = number_part.trim().parse::<u64>() {
                    return Ok(Some(WaitAction::new_duration(Duration::from_secs(seconds))));
                }
            }

            // Handle "Wait for user confirmation" pattern
            if rest.starts_with("for ") {
                return Ok(Some(WaitAction::new_user_input()));
            }

            // Handle simple number format "Wait 30"
            if let Ok(seconds) = rest.parse::<u64>() {
                Ok(Some(WaitAction::new_duration(Duration::from_secs(seconds))))
            } else {
                Ok(Some(WaitAction::new_user_input()))
            }
        } else {
            Ok(None)
        }
    }

    /// Parse log action (backward compatibility method)
    pub fn parse_log_action(&self, description: &str) -> ActionResult<Option<LogAction>> {
        let description = description.trim();
        if description.to_lowercase().starts_with("log ") {
            let rest = description[4..].trim();

            // Handle "Log error "message"" or "Log "message""
            let (level, message_part) = if rest.to_lowercase().starts_with("error ") {
                (LogLevel::Error, rest[6..].trim())
            } else if rest.to_lowercase().starts_with("warn ")
                || rest.to_lowercase().starts_with("warning ")
            {
                let offset = if rest.to_lowercase().starts_with("warn ") {
                    5
                } else {
                    8
                };
                (LogLevel::Warning, rest[offset..].trim())
            } else if rest.to_lowercase().starts_with("info ") {
                (LogLevel::Info, rest[5..].trim())
            } else {
                // Default to info level if no level specified
                (LogLevel::Info, rest)
            };

            let message = self.strip_quotes(message_part);
            Ok(Some(LogAction::new(message.to_string(), level)))
        } else {
            Ok(None)
        }
    }

    /// Parse set variable action (backward compatibility method)
    pub fn parse_set_variable_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<SetVariableAction>> {
        let description = description.trim();
        let lower = description.to_lowercase();

        let rest = if lower.starts_with("set ") {
            &description[4..]
        } else if lower.starts_with("set_variable ") {
            &description[13..]
        } else {
            return Ok(None);
        };

        if let Some((key, value)) = rest.split_once('=') {
            let clean_value = self.strip_quotes(value.trim());
            Ok(Some(SetVariableAction::new(
                key.trim().to_string(),
                clean_value.to_string(),
            )))
        } else {
            Ok(None)
        }
    }

    /// Parse sub workflow action (backward compatibility method)
    pub fn parse_sub_workflow_action(
        &self,
        description: &str,
    ) -> ActionResult<Option<SubWorkflowAction>> {
        let description = description.trim();
        let lower = description.to_lowercase();

        eprintln!(
            "DEBUG: parse_sub_workflow_action called with: '{}'",
            description
        );

        if lower.starts_with("run workflow ") {
            // Parse "Run workflow "name" with input="value""
            if let Some(action) = self.parse_run_workflow_pattern(description)? {
                return Ok(Some(action));
            }
        } else if lower.starts_with("subworkflow ") {
            let workflow_name = &description[12..];
            return Ok(Some(SubWorkflowAction::new(workflow_name.to_string())));
        }

        Ok(None)
    }

    fn parse_run_workflow_pattern(
        &self,
        description: &str,
    ) -> ActionResult<Option<SubWorkflowAction>> {
        // Parse "Run workflow "name" with input="value""
        let regex = regex::Regex::new(r#"^Run workflow "([^"]+)"(.*)"#)
            .map_err(|e| ActionError::ParseError(format!("Failed to create regex: {}", e)))?;

        if let Some(captures) = regex.captures(description) {
            let workflow_name = captures.get(1).unwrap().as_str();
            let args_str = captures.get(2).map(|m| m.as_str()).unwrap_or("");

            let mut action = SubWorkflowAction::new(workflow_name.to_string());

            // Parse arguments if present
            if let Some(args_part) = args_str.strip_prefix(" with ") {
                let parsed_args = self.parse_arguments(args_part)?;

                // Convert parsed arguments to the format SubWorkflowAction expects
                for (key, value) in parsed_args {
                    if key == "result" {
                        // Special handling for the result parameter
                        eprintln!("DEBUG: Setting result_variable to '{}'", value);
                        action.result_variable = Some(value);
                    } else {
                        // Regular input variables
                        eprintln!("DEBUG: Adding input variable '{}' = '{}'", key, value);
                        action.input_variables.insert(key, value);
                    }
                }
            }

            Ok(Some(action))
        } else {
            Ok(None)
        }
    }

    /// Parse abort action (backward compatibility method)
    pub fn parse_abort_action(&self, description: &str) -> ActionResult<Option<AbortAction>> {
        let description = description.trim();
        if description.to_lowercase().starts_with("abort ") {
            let message_part = &description[6..].trim();
            let message = self.strip_quotes(message_part);
            Ok(Some(AbortAction::new(message.to_string())))
        } else {
            Ok(None)
        }
    }

    /// Substitute variables safely (backward compatibility method)
    pub fn substitute_variables_safe(
        &self,
        template: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> ActionResult<String> {
        use regex::Regex;

        eprintln!(
            "DEBUG substitute_variables_safe: template='{}', context={:?}",
            template, context
        );

        // Create regex to match ${variable_name} patterns
        let re = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").map_err(|e| {
            ActionError::ExecutionError(format!(
                "Failed to create variable substitution regex: {}",
                e
            ))
        })?;

        // Replace all ${variable} with their values from context
        let result = re.replace_all(template, |caps: &regex::Captures| {
            let var_name = &caps[1];

            // Look up the variable in the context
            if let Some(value) = context.get(var_name) {
                // Convert JSON value to string representation
                match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    _ => value.to_string(), // For arrays and objects, use JSON representation
                }
            } else {
                // If variable not found, leave the original ${variable} syntax
                caps[0].to_string()
            }
        });

        Ok(result.to_string())
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
    fn test_basic_shell_parsing() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("shell: echo hello").unwrap();
        // Basic smoke test - just ensure it doesn't crash
        assert!(action.as_any().downcast_ref::<ShellAction>().is_some());
    }

    #[test]
    fn test_basic_log_parsing() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("log: test message").unwrap();
        assert!(action.as_any().downcast_ref::<LogAction>().is_some());
    }

    #[test]
    fn test_basic_prompt_parsing() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("prompt: Enter value").unwrap();
        assert!(action.as_any().downcast_ref::<PromptAction>().is_some());
    }

    #[test]
    fn test_basic_wait_parsing() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("wait: 5").unwrap();
        assert!(action.as_any().downcast_ref::<WaitAction>().is_some());
    }

    #[test]
    fn test_basic_set_parsing() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("set: key=value").unwrap();
        assert!(action
            .as_any()
            .downcast_ref::<SetVariableAction>()
            .is_some());
    }

    #[test]
    fn test_get_action_type() {
        let parser = ActionParser::new().unwrap();
        assert_eq!(parser.get_action_type("shell: echo").unwrap(), "shell");
        assert_eq!(parser.get_action_type("log: message").unwrap(), "log");
        assert_eq!(
            parser.get_action_type("prompt: question").unwrap(),
            "prompt"
        );
        assert_eq!(parser.get_action_type("wait: 5").unwrap(), "wait");
        assert_eq!(parser.get_action_type("set: a=b").unwrap(), "set");
    }

    #[test]
    fn test_default_to_shell() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse("just some command").unwrap();
        assert!(action.as_any().downcast_ref::<ShellAction>().is_some());
    }
}
