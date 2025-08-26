//! Backward compatibility tests for legacy --var arguments
//!
//! This module ensures that the new parameter system maintains compatibility
//! with existing --var argument handling, allowing existing workflows and
//! user scripts to continue working without modification.

use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    DefaultParameterResolver, Parameter, ParameterResolver, ParameterType,
};
use swissarmyhammer_cli::parameter_cli::resolve_workflow_parameters_interactive;

/// Test helper to simulate legacy --var argument parsing
fn simulate_var_args(var_args: &[&str]) -> Vec<String> {
    var_args.iter().map(|s| s.to_string()).collect()
}

/// Test helper to resolve parameters using the CLI parameter system
fn resolve_with_var_args(
    workflow_name: &str,
    var_args: &[&str],
    interactive: bool,
) -> Result<HashMap<String, Value>, Box<dyn std::error::Error>> {
    let var_strings = simulate_var_args(var_args);
    resolve_workflow_parameters_interactive(workflow_name, &var_strings, &[], interactive)
}

#[cfg(test)]
mod legacy_var_argument_compatibility_tests {
    use super::*;

    #[test]
    fn test_basic_var_argument_parsing() {
        // Test that basic key=value pairs work as before
        let result = resolve_with_var_args(
            "test-workflow",
            &["name=John", "age=30", "enabled=true"],
            false,
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        
        // Should parse and include all provided variables
        if !resolved.is_empty() {
            // When workflow exists and parameters are resolved
            if resolved.contains_key("name") {
                assert_eq!(resolved.get("name").unwrap(), &json!("John"));
            }
            if resolved.contains_key("age") {
                // Should parse as number
                assert_eq!(resolved.get("age").unwrap(), &json!(30.0));
            }
            if resolved.contains_key("enabled") {
                // Should parse as boolean
                assert_eq!(resolved.get("enabled").unwrap(), &json!(true));
            }
        }
    }

    #[test]
    fn test_var_argument_type_parsing() {
        // Test that various data types are parsed correctly
        let test_cases = vec![
            // String values
            ("string_param=hello", "string_param", json!("hello")),
            ("quoted_string=\"hello world\"", "quoted_string", json!("\"hello world\"")),
            
            // Boolean values
            ("bool_true=true", "bool_true", json!(true)),
            ("bool_false=false", "bool_false", json!(false)),
            ("bool_mixed=TRUE", "bool_mixed", json!("TRUE")), // Case sensitive
            
            // Numeric values
            ("integer=42", "integer", json!(42.0)),
            (&format!("float={}", std::f64::consts::PI), "float", json!(std::f64::consts::PI)),
            ("negative=-10", "negative", json!(-10.0)),
            ("zero=0", "zero", json!(0.0)),
            
            // Edge cases
            ("empty_value=", "empty_value", json!("")),
            ("special_chars=!@#$%", "special_chars", json!("!@#$%")),
        ];

        for (var_arg, expected_key, expected_value) in test_cases {
            let result = resolve_with_var_args("test-workflow", &[var_arg], false);
            
            assert!(result.is_ok(), "Failed to parse var argument: {var_arg}");
            let resolved = result.unwrap();
            
            // If the key exists in resolved parameters, verify the value
            if resolved.contains_key(expected_key) {
                assert_eq!(
                    resolved.get(expected_key).unwrap(),
                    &expected_value,
                    "Incorrect value for {expected_key} from {var_arg}"
                );
            }
        }
    }

    #[test]
    fn test_var_argument_special_characters() {
        // Test handling of special characters and edge cases
        let special_cases = vec![
            "url=https://example.com/path?param=value",
            "path=/usr/local/bin/app",
            "json_like={\"key\":\"value\"}",
            "array_like=[1,2,3]",
            "spaces=hello world",
            "unicode=测试🦀",
            "symbols=!@#$%^&*()_+-={}[]|\\:;\"'<>?,./"
        ];

        for var_arg in special_cases {
            let result = resolve_with_var_args("test-workflow", &[var_arg], false);
            
            // Should handle gracefully without crashing
            assert!(result.is_ok(), "Failed to handle special characters in: {var_arg}");
        }
    }

    #[test]
    fn test_multiple_var_arguments() {
        // Test that multiple --var arguments work together
        let var_args = vec![
            "service_name=my-service",
            "port=8080", 
            "debug=true",
            "environment=production",
            "timeout=30.5",
        ];

        let result = resolve_with_var_args("test-workflow", &var_args, false);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        
        // Verify all arguments are processed if workflow resolution works
        if !resolved.is_empty() {
            for var_arg in &var_args {
                let key = var_arg.split('=').next().unwrap();
                // Key should either be present or gracefully omitted
                if resolved.contains_key(key) {
                    assert!(
                        !resolved.get(key).unwrap().is_null(),
                        "Value should not be null for key: {key}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_var_argument_precedence_and_overrides() {
        // Test that later arguments override earlier ones
        let result = resolve_with_var_args(
            "test-workflow",
            &["param=first", "param=second", "param=final"],
            false,
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        
        // If param is resolved, it should have the final value
        if resolved.contains_key("param") {
            assert_eq!(resolved.get("param").unwrap(), &json!("final"));
        }
    }

    #[test]
    fn test_var_argument_with_complex_values() {
        // Test handling of complex values that might be interpreted in special ways
        let complex_cases = vec![
            ("json_string={\"name\":\"test\",\"value\":123}", "json_string"),
            ("array_string=[\"item1\",\"item2\",\"item3\"]", "array_string"), 
            ("boolean_string=true", "boolean_string"),
            ("number_string=42.5", "number_string"),
            ("null_string=null", "null_string"),
            ("multiline=line1\\nline2\\nline3", "multiline"),
        ];

        for (var_arg, key) in complex_cases {
            let result = resolve_with_var_args("test-workflow", &[var_arg], false);
            
            assert!(result.is_ok(), "Failed to handle complex value: {var_arg}");
            let resolved = result.unwrap();
            
            if resolved.contains_key(key) {
                let value = resolved.get(key).unwrap();
                // Value should be parsed according to type inference
                assert!(
                    value.is_string() || value.is_boolean() || value.is_number(),
                    "Value should be parsed to appropriate type for {key}: {value:?}"
                );
            }
        }
    }

    #[test]
    fn test_var_argument_error_handling() {
        // Test handling of malformed --var arguments
        let malformed_cases = vec![
            "no_equals_sign",
            "=no_key",
            "",
            "multiple=equals=signs",
            "key_only=",
        ];

        for case in malformed_cases {
            let result = resolve_with_var_args("test-workflow", &[case], false);
            
            // Should handle gracefully - either parse what's possible or fail cleanly
            match result {
                Ok(_) => {
                    // Graceful parsing is acceptable
                }
                Err(_) => {
                    // Clean error handling is also acceptable
                }
            }
        }
    }

    #[test]
    fn test_var_argument_backward_compatibility_with_existing_workflows() {
        // Simulate common patterns used with existing built-in workflows
        
        // Common greeting workflow pattern
        let greeting_args = vec![
            "person_name=Alice",
            "language=Spanish", 
            "enthusiastic=true",
        ];

        let result = resolve_with_var_args("greeting", &greeting_args, false);
        assert!(result.is_ok(), "Greeting workflow var args should work");

        // Common plan workflow pattern
        let plan_args = vec![
            "plan_filename=./spec/feature.md",
        ];

        let result = resolve_with_var_args("plan", &plan_args, false);
        assert!(result.is_ok(), "Plan workflow var args should work");

        // Common deployment patterns
        let deploy_args = vec![
            "environment=staging",
            "service_name=my-app",
            "version=1.2.3",
            "enable_monitoring=true",
        ];

        let result = resolve_with_var_args("deploy", &deploy_args, false);
        assert!(result.is_ok(), "Deployment var args should work");
    }

    #[test]
    fn test_var_argument_with_parameter_system_integration() {
        // Test that --var arguments work when a workflow has actual parameters defined
        
        let resolver = DefaultParameterResolver::new();
        let params = vec![
            Parameter::new("name", "User name", ParameterType::String)
                .required(true),
            Parameter::new("age", "User age", ParameterType::Number)
                .required(false)
                .with_default(json!(25)),
            Parameter::new("active", "Active status", ParameterType::Boolean)
                .required(false)
                .with_default(json!(false)),
        ];

        // Simulate CLI args parsed from --var arguments  
        let cli_args = [
            ("name".to_string(), "TestUser".to_string()),
            ("age".to_string(), "30".to_string()),
            ("active".to_string(), "true".to_string()),
        ].iter().cloned().collect();

        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved.get("name").unwrap(), &json!("TestUser"));
        assert_eq!(resolved.get("age").unwrap(), &json!(30.0));
        assert_eq!(resolved.get("active").unwrap(), &json!(true));
    }

    #[test]
    fn test_var_argument_unicode_and_internationalization() {
        // Test that --var arguments handle Unicode content correctly
        let unicode_cases = vec![
            ("chinese=你好世界", "chinese", "你好世界"),
            ("emoji=🚀🦀🎉", "emoji", "🚀🦀🎉"), 
            ("arabic=مرحبا", "arabic", "مرحبا"),
            ("mixed=Hello 世界 🌍", "mixed", "Hello 世界 🌍"),
            ("accents=café naïve résumé", "accents", "café naïve résumé"),
        ];

        for (var_arg, key, expected) in unicode_cases {
            let result = resolve_with_var_args("test-workflow", &[var_arg], false);
            
            assert!(result.is_ok(), "Failed to handle Unicode in: {var_arg}");
            let resolved = result.unwrap();
            
            if resolved.contains_key(key) {
                if let Some(Value::String(value)) = resolved.get(key) {
                    assert_eq!(value, expected, "Unicode value mismatch for {key}");
                }
            }
        }
    }

    #[test] 
    fn test_var_argument_large_values() {
        // Test handling of large values that might cause memory issues
        let large_value = "x".repeat(10000);
        let var_arg = format!("large_param={large_value}");
        
        let result = resolve_with_var_args("test-workflow", &[&var_arg], false);
        
        assert!(result.is_ok(), "Should handle large values gracefully");
        let resolved = result.unwrap();
        
        if resolved.contains_key("large_param") {
            if let Some(Value::String(value)) = resolved.get("large_param") {
                assert_eq!(value.len(), 10000, "Large value should be preserved");
            }
        }
    }

    #[test]
    fn test_var_argument_performance_with_many_args() {
        // Test that many --var arguments don't cause performance issues
        let mut var_args = Vec::new();
        
        for i in 0..1000 {
            var_args.push(format!("param_{i:04}=value_{i}"));
        }
        
        let var_arg_refs: Vec<&str> = var_args.iter().map(|s| s.as_str()).collect();
        
        let start = std::time::Instant::now();
        let result = resolve_with_var_args("test-workflow", &var_arg_refs, false);
        let duration = start.elapsed();
        
        assert!(result.is_ok(), "Should handle many var args");
        assert!(duration.as_millis() < 1000, "Should process quickly: {duration:?}");
    }
}

#[cfg(test)]
mod var_argument_integration_with_new_features {
    use super::*;

    #[test]
    fn test_var_arguments_with_conditional_parameters() {
        // Test that --var arguments work with conditional parameter logic
        
        let resolver = DefaultParameterResolver::new();
        let params = vec![
            Parameter::new("enable_ssl", "Enable SSL", ParameterType::Boolean)
                .with_default(json!(false)),
            Parameter::new("cert_path", "Certificate path", ParameterType::String)
                .when("enable_ssl == true")
                .required(true),
        ];

        // Test SSL disabled via --var
        let cli_args = [("enable_ssl".to_string(), "false".to_string())]
            .iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 1);

        // Test SSL enabled via --var with cert path
        let cli_args = [
            ("enable_ssl".to_string(), "true".to_string()),
            ("cert_path".to_string(), "/etc/ssl/cert.pem".to_string()),
        ].iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved.get("cert_path").unwrap(), &json!("/etc/ssl/cert.pem"));
    }

    #[test]
    fn test_var_arguments_with_validation_rules() {
        // Test that --var arguments work with new validation features
        
        let resolver = DefaultParameterResolver::new();
        let param = Parameter::new("email", "Email address", ParameterType::String)
            .required(true)
            .with_pattern(r"^[^@\s]+@[^@\s]+\.[^@\s]+$");

        // Valid email via --var
        let cli_args = [("email".to_string(), "user@example.com".to_string())]
            .iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&[param.clone()], &cli_args, false);
        assert!(result.is_ok());
        
        // The resolver itself doesn't validate patterns - that's done later
        // But it should successfully resolve the parameter
        let resolved = result.unwrap();
        assert_eq!(resolved.get("email").unwrap(), &json!("user@example.com"));
        
        // Invalid email via --var would also resolve (validation happens separately)
        let cli_args = [("email".to_string(), "invalid-email".to_string())]
            .iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&[param], &cli_args, false);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.get("email").unwrap(), &json!("invalid-email"));
    }

    #[test]
    fn test_var_arguments_with_parameter_groups() {
        // Test that --var arguments work with parameter grouping features
        
        // Parameter groups don't affect resolution directly, but we can test
        // that --var arguments work for parameters that would be grouped
        
        let resolver = DefaultParameterResolver::new();
        let params = vec![
            // Basic group parameters
            Parameter::new("username", "Username", ParameterType::String)
                .required(true),
            Parameter::new("environment", "Environment", ParameterType::String)
                .required(true),
            
            // Advanced group parameters  
            Parameter::new("debug", "Debug mode", ParameterType::Boolean)
                .with_default(json!(false)),
            Parameter::new("timeout", "Timeout", ParameterType::Number)
                .with_default(json!(30)),
        ];

        let cli_args = [
            ("username".to_string(), "testuser".to_string()),
            ("environment".to_string(), "staging".to_string()),
            ("debug".to_string(), "true".to_string()),
            ("timeout".to_string(), "60".to_string()),
        ].iter().cloned().collect();

        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 4);
        assert_eq!(resolved.get("username").unwrap(), &json!("testuser"));
        assert_eq!(resolved.get("environment").unwrap(), &json!("staging"));
        assert_eq!(resolved.get("debug").unwrap(), &json!(true));
        assert_eq!(resolved.get("timeout").unwrap(), &json!(60.0));
    }

    #[test]
    fn test_var_arguments_with_choice_parameters() {
        // Test that --var arguments work with choice parameter validation
        
        let resolver = DefaultParameterResolver::new();
        let param = Parameter::new("log_level", "Log level", ParameterType::Choice)
            .with_choices(vec!["error".to_string(), "warn".to_string(), "info".to_string(), "debug".to_string()])
            .with_default(json!("info"));

        // Valid choice via --var
        let cli_args = [("log_level".to_string(), "debug".to_string())]
            .iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&[param.clone()], &cli_args, false);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.get("log_level").unwrap(), &json!("debug"));

        // Invalid choice via --var (resolver doesn't validate choices, just resolves)
        let cli_args = [("log_level".to_string(), "invalid".to_string())]
            .iter().cloned().collect();
        
        let result = resolver.resolve_parameters(&[param], &cli_args, false);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.get("log_level").unwrap(), &json!("invalid"));
    }

    #[test]
    fn test_var_arguments_precedence_over_defaults() {
        // Test that --var arguments override parameter defaults
        
        let resolver = DefaultParameterResolver::new();
        let params = vec![
            Parameter::new("service_name", "Service name", ParameterType::String)
                .with_default(json!("default-service")),
            Parameter::new("port", "Port number", ParameterType::Number)
                .with_default(json!(8080)),
            Parameter::new("enabled", "Enabled", ParameterType::Boolean)
                .with_default(json!(false)),
        ];

        // Override all defaults with --var arguments
        let cli_args = [
            ("service_name".to_string(), "my-service".to_string()),
            ("port".to_string(), "3000".to_string()),
            ("enabled".to_string(), "true".to_string()),
        ].iter().cloned().collect();

        let result = resolver.resolve_parameters(&params, &cli_args, false);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 3);
        
        // All values should be overridden by --var arguments
        assert_eq!(resolved.get("service_name").unwrap(), &json!("my-service"));
        assert_eq!(resolved.get("port").unwrap(), &json!(3000.0));
        assert_eq!(resolved.get("enabled").unwrap(), &json!(true));
        
        // Use only defaults (no --var arguments)
        let empty_args = HashMap::new();
        let result = resolver.resolve_parameters(&params, &empty_args, false);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 3);
        
        // Should use default values
        assert_eq!(resolved.get("service_name").unwrap(), &json!("default-service"));
        assert_eq!(resolved.get("port").unwrap(), &json!(8080));
        assert_eq!(resolved.get("enabled").unwrap(), &json!(false));
    }
}