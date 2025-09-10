//! CLI parameter integration for workflow parameter resolution
//!
//! This module provides utilities for resolving workflow parameters from
//! CLI arguments and integrating with the existing flow command structure.

use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer::{ParameterType, Result};
use swissarmyhammer_common::Parameter;
use swissarmyhammer_common::{DefaultParameterResolver, ParameterResolver};

/// Resolve parameter values from --var arguments and defaults
///
/// Precedence (highest to lowest):
/// 1. Variable switches (--var param_name=value)
/// 2. Default values from parameter definitions
/// 3. Interactive prompting (if enabled and required)
pub fn resolve_parameters_from_vars(
    workflow_params: &[Parameter],
    var_args: &[String],
    _interactive: bool, // Reserved for future interactive prompting when parameters are missing
) -> swissarmyhammer_common::Result<HashMap<String, Value>> {
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
            return Err(swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!(
                    "Required parameter '{}' is missing. Provide it via --var {}=<value>",
                    param.name, param.name
                ),
            });
        }

        // Store resolved value
        if let Some(v) = value {
            resolved.insert(param.name.clone(), v);
        }
    }

    Ok(resolved)
}

/// Parse a string value according to the parameter type
fn parse_parameter_value(
    value: &str,
    param_type: &ParameterType,
) -> swissarmyhammer_common::Result<Value> {
    match param_type {
        ParameterType::String => Ok(Value::String(value.to_string())),
        ParameterType::Boolean => {
            let parsed = value.parse::<bool>().map_err(|_| {
                swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: format!("Invalid boolean value '{value}'. Expected true/false"),
                }
            })?;
            Ok(Value::Bool(parsed))
        }
        ParameterType::Number => {
            // Try parsing as integer first, then float
            if let Ok(int_val) = value.parse::<i64>() {
                Ok(Value::Number(int_val.into()))
            } else if let Ok(float_val) = value.parse::<f64>() {
                Ok(Value::Number(
                    serde_json::Number::from_f64(float_val).ok_or_else(|| {
                        swissarmyhammer_common::SwissArmyHammerError::Other {
                            message: format!(
                                "Invalid number value '{value}'. Number too large or invalid"
                            ),
                        }
                    })?,
                ))
            } else {
                Err(swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: format!("Invalid number value '{value}'. Expected a number"),
                })
            }
        }
        ParameterType::Choice => Ok(Value::String(value.to_string())),
        ParameterType::MultiChoice => {
            // For multi-choice, split by comma and create array
            let choices: Vec<Value> = value
                .split(',')
                .map(|s| Value::String(s.trim().to_string()))
                .collect();
            Ok(Value::Array(choices))
        }
    }
}

/// Parse --var arguments into a key-value map
fn parse_var_arguments(
    var_args: &[String],
) -> swissarmyhammer_common::Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    for var in var_args {
        if let Some((key, value)) = var.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        } else {
            return Err(swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Invalid --var format '{var}'. Expected KEY=VALUE"),
            });
        }
    }

    Ok(map)
}

/// Resolve workflow parameters with optional interactive prompting
pub fn resolve_workflow_parameters_interactive(
    workflow: &swissarmyhammer_workflow::Workflow,
    var_args: &[String],
    interactive: bool,
) -> Result<HashMap<String, Value>> {
    // Phase 1: Get workflow parameters from the workflow object
    let workflow_params = workflow.parameters.clone();

    // Auto-detect interactive mode: if no var args are provided and we have parameters,
    // and we're in a terminal, enable interactive prompting (matches prompt test behavior)
    let should_be_interactive = interactive
        || (var_args.is_empty() && !workflow_params.is_empty() && atty::is(atty::Stream::Stdin));

    if should_be_interactive {
        // Phase 2: Use interactive parameter resolver
        let resolver = DefaultParameterResolver::new();

        // Convert var_args to HashMap format expected by resolver
        let mut cli_args = HashMap::new();
        for var in var_args {
            let parts: Vec<&str> = var.splitn(2, '=').collect();
            if parts.len() == 2 {
                cli_args.insert(parts[0].to_string(), parts[1].to_string());
            }
        }

        // Parameters are already the correct type
        let parameters = workflow_params;

        resolver
            .resolve_parameters(&parameters, &cli_args, should_be_interactive)
            .map_err(|e| swissarmyhammer::SwissArmyHammerError::Other {
                message: format!("Parameter resolution failed: {e}"),
            })
    } else {
        // Phase 2: Use legacy parameter resolution (non-interactive)
        resolve_parameters_from_vars(&workflow_params, var_args, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workflow_parameters_empty() {
        let result = resolve_workflow_parameters_interactive("nonexistent-workflow", &[], false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_auto_detection_logic() {
        // Test that the auto-detection logic works as expected
        // When no var_args are provided and workflow has parameters, it should enable interactive mode
        // Note: This test runs without a terminal, so interactive detection will be false

        // Use a non-existent workflow to avoid slow file system operations
        // This tests the logic paths without expensive I/O
        let workflow_name = "nonexistent-workflow";
        let empty_vars: &[String] = &[];

        // Test with explicit interactive = false and no vars - should work with empty workflow
        let result = resolve_workflow_parameters_interactive(workflow_name, empty_vars, false);
        // Should succeed with empty parameters for non-existent workflow
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        // Test with explicit interactive = true - should succeed (no parameters to prompt for)
        let result_interactive =
            resolve_workflow_parameters_interactive(workflow_name, empty_vars, true);
        assert!(result_interactive.is_ok());

        // Test with provided vars - should succeed and ignore extra vars
        let vars_with_values = vec!["person_name=TestUser".to_string()];
        let result_with_vars =
            resolve_workflow_parameters_interactive(workflow_name, &vars_with_values, false);
        assert!(result_with_vars.is_ok());
    }
}
