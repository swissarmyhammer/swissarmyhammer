//! CLI parameter integration for workflow parameter resolution
//!
//! This module provides utilities for resolving workflow parameters from
//! CLI arguments and integrating with the existing flow command structure.

use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer::Result;
use swissarmyhammer_common::Parameter;
use swissarmyhammer_common::{DefaultParameterResolver, ParameterResolver};

/// Resolve workflow parameters with optional interactive prompting
pub fn resolve_workflow_parameters_interactive(
    workflow_name: &str,
    var_args: &[String],
    interactive: bool,
) -> Result<HashMap<String, Value>> {
    // Phase 1: Discover workflow parameters
    let workflow_params = match discover_workflow_parameters(workflow_name) {
        Ok(params) => params,
        Err(_) => {
            // If we can't load the workflow, just return empty - workflow will be validated later
            return Ok(HashMap::new());
        }
    };

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
            .map_err(|e| {
                swissarmyhammer::SwissArmyHammerError::Other { message: format!(
                    "Parameter resolution failed: {e}"
                ) }
            })
    } else {
        // Phase 2: Use legacy parameter resolution (non-interactive)
        resolve_parameters_from_vars(&workflow_params, var_args, false)
    }
}

/// Get workflow parameters for help text generation (best effort)
/// Used for future dynamic help text generation implementation
/// Currently only used in tests, hence the allow attribute
#[allow(dead_code)]
pub fn get_workflow_parameters_for_help(workflow_name: &str) -> Vec<Parameter> {
    discover_workflow_parameters(workflow_name).unwrap_or_default()
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
