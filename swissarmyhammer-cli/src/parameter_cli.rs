//! CLI parameter integration for workflow parameter resolution
//!
//! This module provides utilities for resolving workflow parameters from
//! CLI arguments and integrating with the existing flow command structure.

use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer::common::{
    discover_workflow_parameters, resolve_parameters_from_vars,
};
use swissarmyhammer::workflow::WorkflowParameter;
use swissarmyhammer::Result;

/// Resolve workflow parameters from CLI arguments and discover workflow-specific parameters
pub fn resolve_workflow_parameters(
    workflow_name: &str,
    var_args: &[String],
    _set_args: &[String], // Reserved for future --set liquid template variable integration
) -> Result<HashMap<String, Value>> {
    // Phase 1: Discover workflow parameters
    let workflow_params = match discover_workflow_parameters(workflow_name) {
        Ok(params) => params,
        Err(_) => {
            // If we can't load the workflow, just return empty - workflow will be validated later
            return Ok(HashMap::new());
        }
    };
    
    // Phase 2: Resolve parameters from --var arguments and defaults
    resolve_parameters_from_vars(&workflow_params, var_args, false)
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
        let result = resolve_workflow_parameters("nonexistent-workflow", &[], &[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_workflow_parameters_for_help_empty() {
        let result = get_workflow_parameters_for_help("nonexistent-workflow");
        assert!(result.is_empty());
    }
}