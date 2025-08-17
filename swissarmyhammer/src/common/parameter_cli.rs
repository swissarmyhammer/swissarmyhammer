//! CLI integration utilities for parameter system
//!
//! This module provides utilities for parameter resolution from multiple sources
//! and CLI argument name conversion.

use crate::workflow::{WorkflowName, WorkflowParameter, WorkflowStorage};
use serde_json::Value;
use std::collections::HashMap;

/// Convert a parameter name to CLI argument format
///
/// Examples:
/// - "person_name" -> "--person-name"
/// - "language" -> "--language"  
/// - "is_enabled" -> "--is-enabled"
pub fn parameter_name_to_cli_switch(name: &str) -> String {
    format!("--{}", name.replace('_', "-"))
}

/// Generate help text for a workflow parameter
pub fn generate_parameter_help_text(param: &WorkflowParameter) -> String {
    let mut help = param.description.clone();

    // Add default value if present
    if let Some(default) = &param.default {
        let default_str = match default {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => default.to_string(),
        };
        help = format!("{help} [default: {default_str}]");
    }

    // Add choices if present
    if let Some(choices) = &param.choices {
        let choices_str = choices.join(", ");
        help = format!("{help} [possible values: {choices_str}]");
    }

    help
}

/// Discover workflow parameters for CLI argument generation
///
/// This function loads a workflow by name and returns its parameter definitions
/// for use in dynamic CLI argument generation.
pub fn discover_workflow_parameters(workflow_name: &str) -> crate::Result<Vec<WorkflowParameter>> {
    let storage = WorkflowStorage::file_system()?;
    let name = WorkflowName::new(workflow_name.to_string());
    let workflow = storage.get_workflow(&name)?;
    Ok(workflow.parameters)
}

/// Resolve parameter values from --var arguments and defaults
///
/// Precedence (highest to lowest):
/// 1. Variable switches (--var param_name=value)
/// 2. Default values from parameter definitions
/// 3. Interactive prompting (if enabled and required)
pub fn resolve_parameters_from_vars(
    workflow_params: &[WorkflowParameter],
    var_args: &[String],
    _interactive: bool, // Reserved for future interactive prompting when parameters are missing
) -> crate::Result<HashMap<String, Value>> {
    let mut resolved = HashMap::new();

    // Parse var arguments into a map
    let var_map = parse_var_arguments(var_args)?;

    for param in workflow_params {
        let mut value: Option<Value> = None;

        // 1. Check --var arguments (highest precedence)
        if let Some(var_value) = var_map.get(&param.name) {
            value = Some(parse_parameter_value(var_value, &param.parameter_type)?);
        }

        // 2. Use default value (low precedence)
        if value.is_none() && param.default.is_some() {
            value = param.default.clone();
        }

        // 3. Check if required parameter is missing
        if value.is_none() && param.required {
            return Err(crate::SwissArmyHammerError::Config(format!(
                "Required parameter '{}' is missing. Provide it via --var {}=<value>",
                param.name, param.name
            )));
        }

        // Store resolved value
        if let Some(v) = value {
            resolved.insert(param.name.clone(), v);
        }
    }

    Ok(resolved)
}

/// Parse --var arguments into a key-value map
fn parse_var_arguments(var_args: &[String]) -> crate::Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    for var in var_args {
        if let Some((key, value)) = var.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        } else {
            return Err(crate::SwissArmyHammerError::Config(format!(
                "Invalid --var format '{var}'. Expected KEY=VALUE"
            )));
        }
    }

    Ok(map)
}

/// Parse a string value according to the parameter type
fn parse_parameter_value(
    value: &str,
    param_type: &crate::workflow::ParameterType,
) -> crate::Result<Value> {
    match param_type {
        crate::workflow::ParameterType::String => Ok(Value::String(value.to_string())),
        crate::workflow::ParameterType::Boolean => {
            let parsed = value.parse::<bool>().map_err(|_| {
                crate::SwissArmyHammerError::Config(format!(
                    "Invalid boolean value '{value}'. Expected true/false"
                ))
            })?;
            Ok(Value::Bool(parsed))
        }
        crate::workflow::ParameterType::Number => {
            // Try parsing as integer first, then float
            if let Ok(int_val) = value.parse::<i64>() {
                Ok(Value::Number(int_val.into()))
            } else if let Ok(float_val) = value.parse::<f64>() {
                Ok(Value::Number(
                    serde_json::Number::from_f64(float_val).ok_or_else(|| {
                        crate::SwissArmyHammerError::Config(format!(
                            "Invalid number value '{value}'. Number too large or invalid"
                        ))
                    })?,
                ))
            } else {
                Err(crate::SwissArmyHammerError::Config(format!(
                    "Invalid number value '{value}'. Expected a number"
                )))
            }
        }
        crate::workflow::ParameterType::Choice => Ok(Value::String(value.to_string())),
        crate::workflow::ParameterType::MultiChoice => {
            // For multi-choice, split by comma and create array
            let choices: Vec<Value> = value
                .split(',')
                .map(|s| Value::String(s.trim().to_string()))
                .collect();
            Ok(Value::Array(choices))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::ParameterType;

    #[test]
    fn test_parameter_name_to_cli_switch() {
        assert_eq!(parameter_name_to_cli_switch("person_name"), "--person-name");
        assert_eq!(parameter_name_to_cli_switch("language"), "--language");
        assert_eq!(parameter_name_to_cli_switch("is_enabled"), "--is-enabled");
        assert_eq!(
            parameter_name_to_cli_switch("multi_word_param"),
            "--multi-word-param"
        );
    }

    #[test]
    fn test_parse_var_arguments() {
        let vars = vec![
            "name=John".to_string(),
            "age=30".to_string(),
            "enabled=true".to_string(),
        ];

        let result = parse_var_arguments(&vars).unwrap();
        assert_eq!(result.get("name"), Some(&"John".to_string()));
        assert_eq!(result.get("age"), Some(&"30".to_string()));
        assert_eq!(result.get("enabled"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_var_arguments_invalid() {
        let vars = vec!["invalid_format".to_string()];
        let result = parse_var_arguments(&vars);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_parameter_value_string() {
        let result = parse_parameter_value("test", &ParameterType::String).unwrap();
        assert_eq!(result, Value::String("test".to_string()));
    }

    #[test]
    fn test_parse_parameter_value_boolean() {
        let result = parse_parameter_value("true", &ParameterType::Boolean).unwrap();
        assert_eq!(result, Value::Bool(true));

        let result = parse_parameter_value("false", &ParameterType::Boolean).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_parse_parameter_value_number() {
        let result = parse_parameter_value("42", &ParameterType::Number).unwrap();
        assert_eq!(result, Value::Number(42.into()));

        let result = parse_parameter_value("3.14", &ParameterType::Number).unwrap();
        assert!(result.as_f64().is_some());
    }

    #[test]
    fn test_parse_parameter_value_multichoice() {
        let result = parse_parameter_value("red,green,blue", &ParameterType::MultiChoice).unwrap();
        let expected = Value::Array(vec![
            Value::String("red".to_string()),
            Value::String("green".to_string()),
            Value::String("blue".to_string()),
        ]);
        assert_eq!(result, expected);
    }
}
