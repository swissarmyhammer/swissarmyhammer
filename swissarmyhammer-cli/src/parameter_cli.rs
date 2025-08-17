//! CLI parameter integration for workflow parameter resolution
//!
//! This module provides utilities for resolving workflow parameters from
//! CLI arguments and integrating with the existing flow command structure.

use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer::common::{
    discover_workflow_parameters, resolve_parameters_from_vars, DefaultParameterResolver,
    ParameterResolver,
};
use swissarmyhammer::workflow::WorkflowParameter;
use swissarmyhammer::Result;

/// Resolve workflow parameters with optional interactive prompting
pub fn resolve_workflow_parameters_interactive(
    workflow_name: &str,
    var_args: &[String],
    _set_args: &[String], // Reserved for future --set liquid template variable integration
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

    if interactive {
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

        // Convert WorkflowParameter to Parameter
        let parameters: Vec<_> = workflow_params
            .into_iter()
            .map(|wp| wp.to_parameter())
            .collect();

        resolver
            .resolve_parameters(&parameters, &cli_args, interactive)
            .map_err(|e| {
                swissarmyhammer::SwissArmyHammerError::Other(format!(
                    "Parameter resolution failed: {e}"
                ))
            })
    } else {
        // Phase 2: Use legacy parameter resolution (non-interactive)
        resolve_parameters_from_vars(&workflow_params, var_args, false)
    }
}

/// Get workflow parameters for help text generation (best effort)
/// Used for future dynamic help text generation implementation
#[allow(dead_code)]
pub fn get_workflow_parameters_for_help(workflow_name: &str) -> Vec<WorkflowParameter> {
    discover_workflow_parameters(workflow_name).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workflow_parameters_empty() {
        let result =
            resolve_workflow_parameters_interactive("nonexistent-workflow", &[], &[], false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_workflow_parameters_for_help_empty() {
        let result = get_workflow_parameters_for_help("nonexistent-workflow");
        assert!(result.is_empty());
    }
}
