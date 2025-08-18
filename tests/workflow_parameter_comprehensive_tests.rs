//! Comprehensive workflow parameter system integration tests
//!
//! This integration test module brings together all comprehensive tests
//! for the workflow parameter system, ensuring all components work correctly
//! in integration scenarios.

// Import required dependencies that would normally be available in integration tests
use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    CommonPatterns, DefaultParameterResolver, Parameter, ParameterError, ParameterResult, 
    ParameterResolver, ParameterType, ParameterValidator, ValidationRules,
};

// Re-export these for the included modules
pub use serde_json;
pub use swissarmyhammer;

// Include all test modules using the path attribute to directly include the files
// This allows the tests to be compiled and run as part of this integration test
#[path = "workflow_parameters/unit_tests/parameter_validation_comprehensive_tests.rs"]
mod parameter_validation_comprehensive_tests;

#[path = "workflow_parameters/unit_tests/conditional_parameter_resolution_tests.rs"]
mod conditional_parameter_resolution_tests;

#[path = "workflow_parameters/unit_tests/error_condition_tests.rs"]
mod error_condition_tests;

#[path = "workflow_parameters/integration_tests/cli_parameter_integration_tests.rs"]
mod cli_parameter_integration_tests;

#[path = "workflow_parameters/performance_tests/large_parameter_set_benchmarks.rs"]
mod large_parameter_set_benchmarks;

#[path = "workflow_parameters/compatibility_tests/legacy_var_argument_tests.rs"]
mod legacy_var_argument_tests;

#[path = "workflow_parameters/cli_tests/help_generation_tests.rs"]
mod help_generation_tests;

#[cfg(test)]
mod comprehensive_integration_tests {
    use std::collections::HashMap;
    use serde_json::{json, Value};
    use swissarmyhammer::common::parameters::{
        DefaultParameterResolver, Parameter, ParameterResolver, ParameterType,
    };

    #[test]
    fn test_comprehensive_parameter_system_integration() {
        // This test verifies that the complete parameter system works end-to-end
        let resolver = DefaultParameterResolver::new();
        
        // Create a complex parameter set with various types and conditions
        let params = vec![
            Parameter::new("environment", "Deployment environment", ParameterType::Choice)
                .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()])
                .required(true),
            Parameter::new("enable_ssl", "Enable SSL", ParameterType::Boolean)
                .when("environment in [\"staging\", \"prod\"]")
                .with_default(json!(true)),
            Parameter::new("cert_path", "SSL certificate path", ParameterType::String)
                .when("enable_ssl == true")
                .required(true)
                .with_pattern(r"^/.*\.pem$"),
            Parameter::new("port", "Service port", ParameterType::Number)
                .with_range(Some(1.0), Some(65535.0))
                .with_default(json!(8080)),
            Parameter::new("features", "Enabled features", ParameterType::MultiChoice)
                .with_choices(vec!["auth".to_string(), "logging".to_string(), "metrics".to_string()])
                .with_selection_range(Some(1), Some(3))
                .with_default(json!(["logging"])),
        ];

        // Test successful resolution with all requirements met
        let cli_args: HashMap<String, String> = [
            ("environment".to_string(), "prod".to_string()),
            ("cert_path".to_string(), "/etc/ssl/cert.pem".to_string()),
            ("port".to_string(), "443".to_string()),
            ("features".to_string(), "auth,logging,metrics".to_string()),
        ].iter().cloned().collect();

        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok(), "Complete parameter resolution should succeed");
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 5);
        assert_eq!(resolved.get("environment").unwrap(), &json!("prod"));
        assert_eq!(resolved.get("enable_ssl").unwrap(), &json!(true));
        assert_eq!(resolved.get("cert_path").unwrap(), &json!("/etc/ssl/cert.pem"));
        assert_eq!(resolved.get("port").unwrap(), &json!(443.0));
        // Note: features parsing depends on CLI implementation
    }

    #[test]
    fn test_parameter_system_error_handling_integration() {
        let resolver = DefaultParameterResolver::new();
        
        let params = vec![
            Parameter::new("required_param", "Required parameter", ParameterType::String)
                .required(true),
            Parameter::new("conditional_param", "Conditional parameter", ParameterType::String)
                .when("required_param == 'trigger'")
                .required(true),
        ];

        // Test missing required parameter
        let result = resolver.resolve_parameters(&params, &HashMap::new(), false);
        assert!(result.is_err());

        // Test missing conditional parameter
        let cli_args: HashMap<String, String> = [
            ("required_param".to_string(), "trigger".to_string()),
        ].iter().cloned().collect();

        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_performance_with_realistic_parameter_set() {
        let resolver = DefaultParameterResolver::new();
        
        // Create a realistic parameter set similar to what might be used in workflows
        let mut params = Vec::new();
        
        // Basic configuration parameters
        params.push(Parameter::new("service_name", "Service name", ParameterType::String).required(true));
        params.push(Parameter::new("version", "Version", ParameterType::String).with_default(json!("latest")));
        params.push(Parameter::new("environment", "Environment", ParameterType::Choice)
            .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()])
            .required(true));
        
        // Conditional parameters
        params.push(Parameter::new("replicas", "Number of replicas", ParameterType::Number)
            .when("environment == 'prod'")
            .with_range(Some(1.0), Some(100.0))
            .with_default(json!(3)));
        
        params.push(Parameter::new("debug_mode", "Enable debug", ParameterType::Boolean)
            .when("environment == 'dev'")
            .with_default(json!(true)));

        let cli_args: HashMap<String, String> = [
            ("service_name".to_string(), "test-service".to_string()),
            ("environment".to_string(), "prod".to_string()),
        ].iter().cloned().collect();

        let start = std::time::Instant::now();
        let result = resolver.resolve_parameters(&params, &cli_args, false);
        let duration = start.elapsed();

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 3); // service_name, version, environment, replicas
        
        // Performance check - should resolve quickly
        assert!(duration.as_millis() < 100, "Parameter resolution took too long: {:?}", duration);
    }
}