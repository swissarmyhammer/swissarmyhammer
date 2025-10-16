//! Parameter mapping and parsing for workflow execution
//!
//! Maps positional arguments to required workflow parameters and parses optional parameters

use std::collections::HashMap;
use swissarmyhammer::{Parameter, Result, SwissArmyHammerError, Workflow};

/// Map positional arguments to required workflow parameters
///
/// Takes positional arguments and maps them in order to the required parameters
/// defined in the workflow. If no positional arguments are provided, returns an empty map
/// (parameters can be provided via --param or --var instead). Returns an error if more
/// positional arguments are provided than there are required parameters.
pub fn map_positional_to_params(
    workflow: &Workflow,
    positional: Vec<String>,
) -> Result<HashMap<String, serde_json::Value>> {
    let required_params: Vec<&Parameter> =
        workflow.parameters.iter().filter(|p| p.required).collect();

    // If no positional args, that's fine - params can come from --param or --var
    if positional.is_empty() {
        return Ok(HashMap::new());
    }

    // If too many positional args, that's an error
    if positional.len() > required_params.len() {
        let param_names: Vec<String> = required_params.iter().map(|p| p.name.clone()).collect();
        return Err(SwissArmyHammerError::Other {
            message: format!(
                "Expected {} positional argument(s) for required parameters [{}], got {}",
                required_params.len(),
                param_names.join(", "),
                positional.len()
            ),
        });
    }

    let mut params = HashMap::new();
    for (arg, param) in positional.iter().zip(required_params.iter()) {
        params.insert(param.name.clone(), serde_json::Value::String(arg.clone()));
    }

    Ok(params)
}

/// Parse key=value pairs from parameter strings
///
/// Accepts a vector of "key=value" strings and returns a HashMap.
/// Returns an error if any string is not in the correct format.
pub fn parse_param_pairs(pairs: &[String]) -> Result<HashMap<String, serde_json::Value>> {
    let mut params = HashMap::new();

    for pair in pairs {
        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(SwissArmyHammerError::Other {
                message: format!(
                    "Invalid parameter format: '{}'. Expected 'key=value' format. Example: --param input=test",
                    pair
                ),
            });
        }

        let key = parts[0].to_string();
        let value = parts[1].to_string();
        params.insert(key, serde_json::Value::String(value));
    }

    Ok(params)
}

/// Merge parameters from multiple sources with proper precedence
///
/// Precedence (highest to lowest):
/// 1. --param (explicit optional parameters)
/// 2. --var (deprecated, for backward compatibility)
/// 3. Positional arguments (mapped to required parameters)
pub fn merge_params(
    positional_params: HashMap<String, serde_json::Value>,
    param_pairs: HashMap<String, serde_json::Value>,
    var_pairs: HashMap<String, serde_json::Value>,
) -> HashMap<String, serde_json::Value> {
    let mut merged = positional_params;

    // Add --var values (lower precedence)
    for (key, value) in var_pairs {
        merged.insert(key, value);
    }

    // Add --param values (higher precedence, can override --var)
    for (key, value) in param_pairs {
        merged.insert(key, value);
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer::{ParameterType, WorkflowName};

    fn create_test_workflow_with_params(params: Vec<Parameter>) -> Workflow {
        use swissarmyhammer_workflow::StateId;
        Workflow {
            name: WorkflowName::new("test"),
            description: "Test workflow".to_string(),
            initial_state: StateId::new("start"),
            states: HashMap::new(),
            transitions: Vec::new(),
            parameters: params,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_map_positional_to_params_success() {
        let params = vec![
            Parameter::new("input", "Input file", ParameterType::String).required(true),
            Parameter::new("output", "Output file", ParameterType::String).required(true),
        ];

        let workflow = create_test_workflow_with_params(params);
        let positional = vec!["input.txt".to_string(), "output.txt".to_string()];

        let result = map_positional_to_params(&workflow, positional).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("input").unwrap(),
            &serde_json::Value::String("input.txt".to_string())
        );
        assert_eq!(
            result.get("output").unwrap(),
            &serde_json::Value::String("output.txt".to_string())
        );
    }

    #[test]
    fn test_map_positional_to_params_partial_args_allowed() {
        // Providing fewer positional args than required params is now allowed
        // The remaining required params can be provided via --param or --var
        let params = vec![
            Parameter::new("input", "Input file", ParameterType::String).required(true),
            Parameter::new("output", "Output file", ParameterType::String).required(true),
        ];

        let workflow = create_test_workflow_with_params(params);
        let positional = vec!["input.txt".to_string()]; // Only one arg, but that's OK

        let result = map_positional_to_params(&workflow, positional).unwrap();

        // Should only map the first parameter from positional args
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("input").unwrap(),
            &serde_json::Value::String("input.txt".to_string())
        );
    }

    #[test]
    fn test_map_positional_to_params_too_many_args() {
        let params =
            vec![Parameter::new("input", "Input file", ParameterType::String).required(true)];

        let workflow = create_test_workflow_with_params(params);
        let positional = vec!["input.txt".to_string(), "extra.txt".to_string()]; // Too many

        let result = map_positional_to_params(&workflow, positional);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Expected 1 positional argument(s)"));
        assert!(err_msg.contains("got 2"));
    }

    #[test]
    fn test_map_positional_to_params_no_required_params() {
        let params = vec![
            Parameter::new("optional", "Optional param", ParameterType::String).required(false),
        ];

        let workflow = create_test_workflow_with_params(params);
        let positional = vec![];

        let result = map_positional_to_params(&workflow, positional).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_param_pairs_success() {
        let pairs = vec![
            "key1=value1".to_string(),
            "key2=value2".to_string(),
            "key3=value with spaces".to_string(),
        ];

        let result = parse_param_pairs(&pairs).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(
            result.get("key1").unwrap(),
            &serde_json::Value::String("value1".to_string())
        );
        assert_eq!(
            result.get("key2").unwrap(),
            &serde_json::Value::String("value2".to_string())
        );
        assert_eq!(
            result.get("key3").unwrap(),
            &serde_json::Value::String("value with spaces".to_string())
        );
    }

    #[test]
    fn test_parse_param_pairs_with_equals_in_value() {
        let pairs = vec!["key=value=with=equals".to_string()];

        let result = parse_param_pairs(&pairs).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("key").unwrap(),
            &serde_json::Value::String("value=with=equals".to_string())
        );
    }

    #[test]
    fn test_parse_param_pairs_invalid_format() {
        let pairs = vec!["invalid_no_equals".to_string()];

        let result = parse_param_pairs(&pairs);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid parameter format"));
        assert!(err_msg.contains("invalid_no_equals"));
    }

    #[test]
    fn test_parse_param_pairs_empty() {
        let pairs: Vec<String> = vec![];

        let result = parse_param_pairs(&pairs).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_merge_params_precedence() {
        let mut positional = HashMap::new();
        positional.insert(
            "pos1".to_string(),
            serde_json::Value::String("positional_value".to_string()),
        );

        let mut params = HashMap::new();
        params.insert(
            "param1".to_string(),
            serde_json::Value::String("param_value".to_string()),
        );
        params.insert(
            "shared".to_string(),
            serde_json::Value::String("from_param".to_string()),
        );

        let mut vars = HashMap::new();
        vars.insert(
            "var1".to_string(),
            serde_json::Value::String("var_value".to_string()),
        );
        vars.insert(
            "shared".to_string(),
            serde_json::Value::String("from_var".to_string()),
        );

        let merged = merge_params(positional, params, vars);

        assert_eq!(merged.len(), 4);
        assert_eq!(
            merged.get("pos1").unwrap(),
            &serde_json::Value::String("positional_value".to_string())
        );
        assert_eq!(
            merged.get("param1").unwrap(),
            &serde_json::Value::String("param_value".to_string())
        );
        assert_eq!(
            merged.get("var1").unwrap(),
            &serde_json::Value::String("var_value".to_string())
        );
        // --param should win over --var for shared key
        assert_eq!(
            merged.get("shared").unwrap(),
            &serde_json::Value::String("from_param".to_string())
        );
    }

    #[test]
    fn test_merge_params_empty() {
        let merged = merge_params(HashMap::new(), HashMap::new(), HashMap::new());

        assert_eq!(merged.len(), 0);
    }
}
