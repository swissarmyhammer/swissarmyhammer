# Comprehensive Testing Suite for Workflow Parameters

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Create a comprehensive testing suite that validates all aspects of the workflow parameter system, ensuring reliability, backward compatibility, and correct behavior across all parameter features and edge cases.

## Current State

- Individual components have been implemented with basic tests
- No comprehensive integration testing across the full parameter system
- Missing edge case testing and error condition validation
- Need end-to-end testing of complete user workflows

## Implementation Tasks

### 1. Unit Test Coverage

Ensure comprehensive unit test coverage for all parameter components:

```rust
// swissarmyhammer/src/common/parameters/tests.rs
#[cfg(test)]
mod parameter_tests {
    use super::*;

    #[test]
    fn test_parameter_type_validation() {
        // String validation
        let string_param = Parameter {
            name: "test".to_string(),
            parameter_type: ParameterType::String,
            validation: Some(ValidationRules {
                min_length: Some(5),
                max_length: Some(20),
                pattern: Some(r"^[a-zA-Z]+$".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Valid cases
        assert!(validate_parameter(&string_param, "hello").is_ok());
        assert!(validate_parameter(&string_param, "ValidString").is_ok());
        
        // Invalid cases
        assert!(validate_parameter(&string_param, "hi").is_err()); // Too short
        assert!(validate_parameter(&string_param, "a".repeat(25)).is_err()); // Too long
        assert!(validate_parameter(&string_param, "hello123").is_err()); // Pattern mismatch
    }
    
    #[test]
    fn test_number_validation() {
        let number_param = Parameter {
            name: "count".to_string(),
            parameter_type: ParameterType::Number,
            validation: Some(ValidationRules {
                min: Some(1.0),
                max: Some(100.0),
                step: Some(5.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        assert!(validate_parameter(&number_param, 10.0).is_ok());
        assert!(validate_parameter(&number_param, 0.0).is_err()); // Below min
        assert!(validate_parameter(&number_param, 150.0).is_err()); // Above max  
        assert!(validate_parameter(&number_param, 7.0).is_err()); // Invalid step
    }
    
    #[test]
    fn test_choice_validation() {
        let choice_param = Parameter {
            name: "env".to_string(),
            parameter_type: ParameterType::Choice,
            choices: Some(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()]),
            ..Default::default()
        };
        
        assert!(validate_parameter(&choice_param, "dev").is_ok());
        assert!(validate_parameter(&choice_param, "invalid").is_err());
    }
}
```

### 2. Integration Tests

Create comprehensive integration tests covering complete workflows:

```rust
// tests/workflow_parameters_integration_tests.rs
use swissarmyhammer::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_complete_parameter_workflow() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test workflow with parameters
    let workflow_content = r#"
---
title: Test Workflow
description: Integration test workflow
parameters:
  - name: username
    description: User name
    required: true
    type: string
    pattern: '^[a-zA-Z]+$'
    
  - name: age
    description: User age
    required: false
    type: number
    min: 18
    max: 120
    default: 25
    
  - name: environment
    description: Target environment
    required: true
    type: choice
    choices: [dev, test, prod]
---

# Test Workflow
{{ username }} is {{ age }} years old, deploying to {{ environment }}.
"#;

    // Write workflow file
    let workflow_path = temp_dir.path().join("test-workflow.md");
    std::fs::write(&workflow_path, workflow_content).unwrap();
    
    // Test CLI parameter parsing
    let args = vec![
        "--username", "alice",
        "--age", "30", 
        "--environment", "prod"
    ];
    
    let result = execute_workflow_with_cli_args("test-workflow", args).await;
    assert!(result.is_ok());
    
    let context = result.unwrap();
    assert_eq!(context.get("username").unwrap(), "alice");
    assert_eq!(context.get("age").unwrap().as_f64().unwrap(), 30.0);
    assert_eq!(context.get("environment").unwrap(), "prod");
}

#[tokio::test] 
async fn test_interactive_prompting_integration() {
    // Mock interactive input
    let mock_input = MockInteractiveInput::new(vec![
        "john".to_string(),      // username
        "35".to_string(),        // age
        "2".to_string(),         // environment choice (prod)
    ]);
    
    let result = execute_workflow_interactively("test-workflow", mock_input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_conditional_parameters_integration() {
    let workflow_content = r#"
---
title: Conditional Test
parameters:
  - name: enable_ssl
    type: boolean
    default: false
    
  - name: cert_path
    type: string
    required: true
    condition: "enable_ssl == true"
    pattern: '^.*\.(pem|crt)$'
---
# Conditional Test
"#;

    // Test with SSL disabled - cert_path should not be required
    let result = execute_workflow_with_args("conditional-test", vec![
        "--enable-ssl=false"
    ]).await;
    assert!(result.is_ok());
    
    // Test with SSL enabled - cert_path should be required
    let result = execute_workflow_with_args("conditional-test", vec![
        "--enable-ssl=true"
        // Missing cert_path should cause error
    ]).await;
    assert!(result.is_err());
    
    // Test with SSL enabled and cert_path provided
    let result = execute_workflow_with_args("conditional-test", vec![
        "--enable-ssl=true",
        "--cert-path=/path/to/cert.pem"
    ]).await;
    assert!(result.is_ok());
}
```

### 3. Error Condition Testing

Test all error conditions and edge cases:

```rust
#[tokio::test]
async fn test_parameter_validation_errors() {
    // Test required parameter missing
    let result = execute_workflow_with_args("test-workflow", vec![
        "--age=30" // Missing required username
    ]).await;
    
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("required parameter 'username' is missing"));
    
    // Test pattern validation failure  
    let result = execute_workflow_with_args("test-workflow", vec![
        "--username=user123", // Contains numbers, violates pattern
        "--environment=dev"
    ]).await;
    
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("must match pattern"));
    
    // Test choice validation failure
    let result = execute_workflow_with_args("test-workflow", vec![
        "--username=alice",
        "--environment=invalid" // Not in choices list
    ]).await;
    
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("must be one of"));
}

#[tokio::test]
async fn test_circular_dependency_detection() {
    let workflow_content = r#"
---
title: Circular Dependency Test
parameters:
  - name: param_a
    type: string
    condition: "param_b == 'value'"
    
  - name: param_b  
    type: string
    condition: "param_a == 'other'"
---
# Circular Test
"#;

    let result = load_workflow_from_content(workflow_content);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("circular dependency"));
}
```

### 4. Backward Compatibility Tests

Ensure existing usage patterns continue to work:

```rust
#[tokio::test]
async fn test_legacy_var_support() {
    // Test that --var arguments still work alongside new parameter switches
    let result = execute_workflow_with_args("greeting", vec![
        "--var", "person_name=John",
        "--language=Spanish", // New-style switch
        "--var", "enthusiastic=true"
    ]).await;
    
    assert!(result.is_ok());
    
    let context = result.unwrap();
    assert_eq!(context.get("person_name").unwrap(), "John");
    assert_eq!(context.get("language").unwrap(), "Spanish");
    assert_eq!(context.get("enthusiastic").unwrap(), &serde_json::json!(true));
}

#[tokio::test]
async fn test_legacy_set_support() {
    // Test that --set arguments for liquid templates still work
    let result = execute_workflow_with_args("greeting", vec![
        "--person-name", "Alice",
        "--set", "custom_template_var=value"
    ]).await;
    
    assert!(result.is_ok());
    
    let context = result.unwrap();
    assert!(context.contains_key("_template_vars"));
}
```

### 5. Performance Tests

Test performance with large numbers of parameters:

```rust
#[tokio::test]
async fn test_large_parameter_set_performance() {
    use std::time::Instant;
    
    // Create workflow with 100 parameters
    let mut parameters = Vec::new();
    for i in 0..100 {
        parameters.push(format!(
            "- name: param_{}\n  type: string\n  required: false\n  default: 'value_{}'",
            i, i
        ));
    }
    
    let workflow_content = format!(
        "---\ntitle: Large Parameter Test\nparameters:\n{}\n---\n# Test",
        parameters.join("\n")
    );
    
    let start = Instant::now();
    let result = load_workflow_from_content(&workflow_content);
    let load_time = start.elapsed();
    
    assert!(result.is_ok());
    assert!(load_time.as_millis() < 100); // Should load in under 100ms
    
    // Test parameter resolution performance
    let start = Instant::now();
    let context = resolve_parameters_with_defaults(&result.unwrap()).await.unwrap();
    let resolve_time = start.elapsed();
    
    assert_eq!(context.len(), 100);
    assert!(resolve_time.as_millis() < 50); // Should resolve in under 50ms
}
```

### 6. CLI Help Text Tests

Validate CLI help text generation:

```rust
#[tokio::test]
async fn test_cli_help_generation() {
    let help_text = generate_workflow_help("greeting").await.unwrap();
    
    // Should include parameter descriptions
    assert!(help_text.contains("--person-name"));
    assert!(help_text.contains("The name of the person to greet"));
    
    // Should include choices for choice parameters  
    assert!(help_text.contains("[possible values: English, Spanish, French"));
    
    // Should indicate required vs optional
    assert!(help_text.contains("(required)"));
    
    // Should show default values
    assert!(help_text.contains("[default: English]"));
}

#[tokio::test]
async fn test_grouped_help_generation() {
    let help_text = generate_workflow_help("deploy").await.unwrap();
    
    // Should organize parameters by groups
    assert!(help_text.contains("Deployment Configuration:"));
    assert!(help_text.contains("Security Settings:"));
    assert!(help_text.contains("Monitoring and Logging:"));
    
    // Parameters should appear under correct groups
    let deployment_section = extract_section(&help_text, "Deployment Configuration:");
    assert!(deployment_section.contains("--deploy-env"));
    assert!(deployment_section.contains("--region"));
}
```

## Technical Details

### Test Organization

```
tests/
├── workflow_parameters/
│   ├── unit_tests/
│   │   ├── parameter_validation_tests.rs
│   │   ├── condition_evaluation_tests.rs
│   │   └── parameter_group_tests.rs
│   ├── integration_tests/
│   │   ├── cli_parameter_tests.rs
│   │   ├── interactive_prompting_tests.rs
│   │   └── workflow_execution_tests.rs
│   ├── compatibility_tests/
│   │   ├── backward_compatibility_tests.rs
│   │   └── migration_tests.rs
│   └── performance_tests/
│       ├── parameter_resolution_benchmarks.rs
│       └── large_workflow_tests.rs
```

### Test Utilities

Create helper functions for test setup:

```rust
pub struct WorkflowTestHelpers;

impl WorkflowTestHelpers {
    pub fn create_test_workflow(parameters: Vec<Parameter>) -> Workflow {
        // Helper to create workflows for testing
    }
    
    pub async fn execute_workflow_with_mock_input(
        workflow_name: &str,
        inputs: Vec<String>
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Helper for testing interactive prompting
    }
    
    pub fn assert_parameter_error(
        result: &Result<(), ParameterError>,
        expected_error_type: &str
    ) {
        // Helper for validating specific error types
    }
}
```

### Testing Requirements

- [ ] 100% code coverage for parameter-related code
- [ ] All parameter types tested (string, boolean, number, choice, multi_choice)
- [ ] All validation rules tested (pattern, range, length, etc.)
- [ ] Conditional parameter logic tested
- [ ] Parameter group functionality tested
- [ ] CLI switch generation tested
- [ ] Interactive prompting tested
- [ ] Error conditions and edge cases tested
- [ ] Backward compatibility tested
- [ ] Performance benchmarks established

## Success Criteria

- [ ] Comprehensive test suite covers all parameter features
- [ ] All tests pass consistently in CI/CD environment  
- [ ] Performance meets established benchmarks
- [ ] Error messages are clear and actionable
- [ ] Backward compatibility maintained
- [ ] Edge cases handled gracefully
- [ ] Test coverage reports show complete coverage

## Dependencies

- Requires completion of all previous workflow parameter steps
- Foundation for final integration and documentation

## Example Test Execution

```bash
# Run all parameter tests
cargo test workflow_parameters

# Run specific test categories
cargo test parameter_validation
cargo test interactive_prompting
cargo test backward_compatibility

# Run performance benchmarks  
cargo bench parameter_resolution
```

## Next Steps

After completion, enables:
- Confident deployment of parameter system
- Clear documentation of tested functionality
- Maintenance and enhancement of parameter features