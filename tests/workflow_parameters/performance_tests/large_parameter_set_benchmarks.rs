//! Performance benchmarks for large parameter sets
//!
//! This module tests the performance characteristics of the parameter system
//! when dealing with large numbers of parameters, complex validation rules,
//! and conditional parameter chains.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use swissarmyhammer::common::parameters::{
    DefaultParameterResolver, Parameter, ParameterResolver, ParameterType, ParameterValidator,
    ValidationRules, CommonPatterns,
};

/// Performance test configuration
struct PerfTestConfig {
    /// Number of parameters to create
    parameter_count: usize,
    /// Maximum acceptable duration for operation
    max_duration: Duration,
    /// Test description
    description: &'static str,
}

impl PerfTestConfig {
    fn new(parameter_count: usize, max_duration_ms: u64, description: &'static str) -> Self {
        Self {
            parameter_count,
            max_duration: Duration::from_millis(max_duration_ms),
            description,
        }
    }
}

/// Run a timed operation and assert it completes within the expected duration
fn time_operation<F, R>(config: &PerfTestConfig, operation: F) -> R
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = operation();
    let duration = start.elapsed();

    assert!(
        duration <= config.max_duration,
        "{} took {duration:?}, expected <= {:?} (parameter count: {})",
        config.description,
        config.max_duration,
        config.parameter_count
    );

    println!(
        "✓ {} completed in {duration:?} (limit: {:?}, parameters: {})",
        config.description,
        config.max_duration,
        config.parameter_count
    );

    result
}

/// Create a large set of test parameters with defaults
fn create_large_parameter_set(count: usize, with_validation: bool) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(count);

    for i in 0..count {
        let param_type = match i % 5 {
            0 => ParameterType::String,
            1 => ParameterType::Boolean,
            2 => ParameterType::Number,
            3 => ParameterType::Choice,
            _ => ParameterType::MultiChoice,
        };

        let mut param = Parameter::new(
            format!("param_{i:04}"),
            format!("Parameter {i} for performance testing"),
            param_type.clone(),
        ).with_default(match param_type {
            ParameterType::String => json!(format!("default_value_{i}")),
            ParameterType::Boolean => json!(i % 2 == 0),
            ParameterType::Number => json!(i as f64),
            ParameterType::Choice => json!("option_a"),
            ParameterType::MultiChoice => json!(["option_a"]),
        });

        // Add validation rules for some parameters if requested
        if with_validation && i % 10 == 0 {
            param = match param_type {
                ParameterType::String => param.with_length_range(Some(1), Some(100))
                    .with_pattern(r"^[a-zA-Z0-9_]+$"),
                ParameterType::Number => param.with_range(Some(0.0), Some(1000.0))
                    .with_step(1.0),
                ParameterType::Choice => param.with_choices(vec![
                    "option_a".to_string(),
                    "option_b".to_string(),
                    "option_c".to_string(),
                ]),
                ParameterType::MultiChoice => param.with_choices(vec![
                    "option_a".to_string(),
                    "option_b".to_string(),
                    "option_c".to_string(),
                ]).with_selection_range(Some(1), Some(3)),
                _ => param,
            };
        }

        parameters.push(param);
    }

    parameters
}

/// Create a set of conditional parameters with dependency chains
fn create_conditional_parameter_chain(chain_length: usize) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(chain_length + 1);

    // Base parameter that triggers the chain
    parameters.push(
        Parameter::new("trigger", "Chain trigger", ParameterType::Boolean)
            .with_default(json!(true))
    );

    // Create a chain of conditional parameters
    for i in 0..chain_length {
        let condition = if i == 0 {
            "trigger == true".to_string()
        } else {
            format!("chain_param_{} == true", i - 1)
        };

        parameters.push(
            Parameter::new(
                format!("chain_param_{i}"),
                format!("Chain parameter {i}"),
                ParameterType::Boolean,
            )
            .when(condition)
            .with_default(json!(true))
        );
    }

    parameters
}

#[cfg(test)]
mod large_parameter_set_tests {
    use super::*;

    #[test]
    fn test_large_parameter_resolution_performance() {
        let test_cases = vec![
            PerfTestConfig::new(100, 100, "100 parameter resolution"),
            PerfTestConfig::new(500, 500, "500 parameter resolution"),
            PerfTestConfig::new(1000, 1000, "1000 parameter resolution"),
        ];

        let resolver = DefaultParameterResolver::new();

        for config in test_cases {
            let parameters = create_large_parameter_set(config.parameter_count, false);
            let cli_args = HashMap::new(); // Use all defaults

            let resolved = time_operation(&config, || {
                resolver.resolve_parameters(&parameters, &cli_args, false)
            });

            assert!(resolved.is_ok());
            let resolved_params = resolved.unwrap();
            assert_eq!(resolved_params.len(), config.parameter_count);
        }
    }

    #[test]
    fn test_large_parameter_set_with_validation() {
        let test_cases = vec![
            PerfTestConfig::new(100, 150, "100 parameters with validation"),
            PerfTestConfig::new(500, 750, "500 parameters with validation"),
            PerfTestConfig::new(1000, 1500, "1000 parameters with validation"),
        ];

        let resolver = DefaultParameterResolver::new();

        for config in test_cases {
            let parameters = create_large_parameter_set(config.parameter_count, true);
            let cli_args = HashMap::new(); // Use all defaults

            let resolved = time_operation(&config, || {
                resolver.resolve_parameters(&parameters, &cli_args, false)
            });

            assert!(resolved.is_ok());
            let resolved_params = resolved.unwrap();
            assert_eq!(resolved_params.len(), config.parameter_count);
        }
    }

    #[test]
    fn test_large_parameter_set_validation_performance() {
        let test_cases = vec![
            PerfTestConfig::new(100, 50, "Validate 100 parameters"),
            PerfTestConfig::new(500, 250, "Validate 500 parameters"),
            PerfTestConfig::new(1000, 500, "Validate 1000 parameters"),
        ];

        let validator = ParameterValidator::new();

        for config in test_cases {
            let parameters = create_large_parameter_set(config.parameter_count, true);
            
            // Create values that match the parameters
            let values: HashMap<String, Value> = parameters
                .iter()
                .map(|p| {
                    let value = match p.parameter_type {
                        ParameterType::String => json!(format!("valid_value_{}", p.name)),
                        ParameterType::Boolean => json!(true),
                        ParameterType::Number => json!(42.0),
                        ParameterType::Choice => json!("option_a"),
                        ParameterType::MultiChoice => json!(["option_a"]),
                    };
                    (p.name.clone(), value)
                })
                .collect();

            let result = time_operation(&config, || {
                validator.validate_parameters(&parameters, &values)
            });

            assert!(result.is_ok(), "Validation should succeed for valid parameters");
        }
    }

    #[test]
    fn test_parameter_creation_performance() {
        let test_cases = vec![
            PerfTestConfig::new(1000, 100, "Create 1000 parameters"),
            PerfTestConfig::new(5000, 500, "Create 5000 parameters"),
            PerfTestConfig::new(10000, 1000, "Create 10000 parameters"),
        ];

        for config in test_cases {
            let parameters = time_operation(&config, || {
                create_large_parameter_set(config.parameter_count, true)
            });

            assert_eq!(parameters.len(), config.parameter_count);
        }
    }

    #[test]
    fn test_cli_arg_parsing_performance() {
        let test_cases = vec![
            PerfTestConfig::new(100, 50, "Parse 100 CLI arguments"),
            PerfTestConfig::new(500, 250, "Parse 500 CLI arguments"),
            PerfTestConfig::new(1000, 500, "Parse 1000 CLI arguments"),
        ];

        let resolver = DefaultParameterResolver::new();

        for config in test_cases {
            // Create CLI args map
            let cli_args: HashMap<String, String> = (0..config.parameter_count)
                .map(|i| (format!("param_{i:04}"), format!("value_{i}")))
                .collect();

            // Simple parameters for parsing
            let parameters: Vec<Parameter> = (0..config.parameter_count)
                .map(|i| Parameter::new(
                    format!("param_{i:04}"),
                    format!("Parameter {i}"),
                    ParameterType::String,
                ))
                .collect();

            let resolved = time_operation(&config, || {
                resolver.resolve_parameters(&parameters, &cli_args, false)
            });

            assert!(resolved.is_ok());
            let resolved_params = resolved.unwrap();
            assert_eq!(resolved_params.len(), config.parameter_count);
        }
    }
}

#[cfg(test)]
mod conditional_parameter_performance_tests {
    use super::*;

    #[test]
    fn test_conditional_parameter_chain_resolution() {
        let test_cases = vec![
            PerfTestConfig::new(10, 50, "10-parameter conditional chain"),
            PerfTestConfig::new(25, 100, "25-parameter conditional chain"),
            PerfTestConfig::new(50, 200, "50-parameter conditional chain"),
        ];

        let resolver = DefaultParameterResolver::new();

        for config in test_cases {
            let parameters = create_conditional_parameter_chain(config.parameter_count);
            let cli_args = HashMap::new(); // Use all defaults

            let resolved = time_operation(&config, || {
                resolver.resolve_parameters(&parameters, &cli_args, false)
            });

            assert!(resolved.is_ok());
            let resolved_params = resolved.unwrap();
            // Should include trigger + all chain parameters
            assert_eq!(resolved_params.len(), config.parameter_count + 1);
        }
    }

    #[test]
    fn test_complex_conditional_branching_performance() {
        let config = PerfTestConfig::new(100, 200, "Complex conditional branching");
        
        let mut parameters = Vec::new();

        // Create base parameters
        for i in 0..10 {
            parameters.push(
                Parameter::new(
                    format!("base_{i}"),
                    format!("Base parameter {i}"),
                    ParameterType::Boolean,
                ).with_default(json!(i % 2 == 0))
            );
        }

        // Create conditional parameters with various conditions
        for i in 0..90 {
            let base_index = i % 10;
            let condition = format!("base_{base_index} == true");
            
            parameters.push(
                Parameter::new(
                    format!("conditional_{i}"),
                    format!("Conditional parameter {i}"),
                    ParameterType::String,
                )
                .when(condition)
                .with_default(json!(format!("value_{i}")))
            );
        }

        let resolver = DefaultParameterResolver::new();
        let cli_args = HashMap::new(); // Use all defaults

        let resolved = time_operation(&config, || {
            resolver.resolve_parameters(&parameters, &cli_args, false)
        });

        assert!(resolved.is_ok());
        let resolved_params = resolved.unwrap();
        
        // Should include all base parameters + conditionals where base is true
        assert!(resolved_params.len() >= 10); // At least base parameters
        assert!(resolved_params.len() <= 100); // At most all parameters
    }

    #[test]
    fn test_deeply_nested_conditions() {
        let config = PerfTestConfig::new(20, 150, "Deeply nested conditions");
        
        let mut parameters = Vec::new();

        // Create a deep nesting of conditions: param_0 -> param_1 -> ... -> param_N
        for i in 0..config.parameter_count {
            let condition = if i == 0 {
                None // First parameter has no condition
            } else {
                Some(format!("nested_param_{} == 'enable'", i - 1))
            };

            let mut param = Parameter::new(
                format!("nested_param_{i}"),
                format!("Nested parameter {i}"),
                ParameterType::String,
            ).with_default(json!("enable"));

            if let Some(cond) = condition {
                param = param.when(cond);
            }

            parameters.push(param);
        }

        let resolver = DefaultParameterResolver::new();
        let cli_args = HashMap::new(); // Use all defaults

        let resolved = time_operation(&config, || {
            resolver.resolve_parameters(&parameters, &cli_args, false)
        });

        assert!(resolved.is_ok());
        let resolved_params = resolved.unwrap();
        
        // All parameters should be resolved because they form a valid chain
        assert_eq!(resolved_params.len(), config.parameter_count);
    }

    #[test]
    fn test_circular_dependency_detection_performance() {
        let config = PerfTestConfig::new(2, 100, "Circular dependency detection");
        
        // Create circular dependency: param_a depends on param_b, param_b depends on param_a
        let parameters = vec![
            Parameter::new("param_a", "Parameter A", ParameterType::String)
                .when("param_b == 'enable'")
                .with_default(json!("value_a")),
            Parameter::new("param_b", "Parameter B", ParameterType::String)
                .when("param_a == 'value_a'")
                .with_default(json!("enable")),
        ];

        let resolver = DefaultParameterResolver::new();
        let cli_args = HashMap::new();

        let result = time_operation(&config, || {
            resolver.resolve_parameters(&parameters, &cli_args, false)
        });

        // Should detect circular dependency and fail quickly
        assert!(result.is_err());
        match result.unwrap_err() {
            swissarmyhammer::common::parameters::ParameterError::ValidationFailed { message } => {
                assert!(message.contains("circular dependency"));
            }
            other => panic!("Expected ValidationFailed error, got: {other:?}"),
        }
    }
}

#[cfg(test)]
mod memory_usage_tests {
    use super::*;

    #[test]
    fn test_parameter_memory_efficiency() {
        // Test that large parameter sets don't consume excessive memory
        let sizes = vec![1000, 5000, 10000];

        for size in sizes {
            let config = PerfTestConfig::new(size, 2000, "Memory efficient parameter creation");
            
            let parameters = time_operation(&config, || {
                create_large_parameter_set(size, true)
            });

            // Verify all parameters are created
            assert_eq!(parameters.len(), size);
            
            // Basic memory usage check - ensure parameters don't hold excessive data
            for param in parameters.iter().take(10) {
                assert!(!param.name.is_empty());
                assert!(!param.description.is_empty());
                // Description shouldn't be unreasonably long
                assert!(param.description.len() < 1000);
            }

            // Test that we can drop the parameters without issues
            drop(parameters);
        }
    }

    #[test]
    fn test_resolution_memory_efficiency() {
        let config = PerfTestConfig::new(1000, 1000, "Memory efficient resolution");
        
        let parameters = create_large_parameter_set(config.parameter_count, false);
        let resolver = DefaultParameterResolver::new();
        let cli_args = HashMap::new();

        let resolved = time_operation(&config, || {
            resolver.resolve_parameters(&parameters, &cli_args, false)
        });

        assert!(resolved.is_ok());
        let resolved_params = resolved.unwrap();
        
        // Verify resolved parameters
        assert_eq!(resolved_params.len(), config.parameter_count);
        
        // Check that resolved values are reasonable
        for (key, value) in resolved_params.iter().take(10) {
            assert!(!key.is_empty());
            match value {
                Value::String(s) => assert!(!s.is_empty()),
                Value::Bool(_) | Value::Number(_) => {},
                Value::Array(arr) => assert!(!arr.is_empty()),
                other => panic!("Unexpected value type: {other:?}"),
            }
        }
    }

    #[test]
    fn test_validation_memory_efficiency() {
        let config = PerfTestConfig::new(1000, 500, "Memory efficient validation");
        
        let parameters = create_large_parameter_set(config.parameter_count, true);
        let validator = ParameterValidator::new();
        
        // Create reasonable values
        let values: HashMap<String, Value> = parameters
            .iter()
            .map(|p| (p.name.clone(), json!(format!("valid_{}", p.name))))
            .collect();

        let result = time_operation(&config, || {
            validator.validate_parameters(&parameters, &values)
        });

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod scalability_tests {
    use super::*;

    #[test]
    fn test_parameter_lookup_scalability() {
        // Test that parameter lookup doesn't degrade significantly with size
        let sizes = vec![100, 500, 1000, 2000];
        let mut previous_duration = Duration::from_nanos(0);

        for size in sizes {
            let parameters = create_large_parameter_set(size, false);
            let values: HashMap<String, Value> = parameters
                .iter()
                .map(|p| (p.name.clone(), p.default.clone().unwrap_or(json!("test"))))
                .collect();

            let validator = ParameterValidator::new();
            
            let start = Instant::now();
            let result = validator.validate_parameters(&parameters, &values);
            let duration = start.elapsed();

            assert!(result.is_ok());
            
            println!("Validation for {size} parameters took: {duration:?}");
            
            if size > 100 {
                // Ensure scaling is not exponential
                let ratio = duration.as_nanos() as f64 / previous_duration.as_nanos() as f64;
                assert!(
                    ratio < 10.0,
                    "Performance degraded too much: {size} parameters took {ratio:.2}x longer than previous size"
                );
            }
            
            previous_duration = duration;
        }
    }

    #[test]
    fn test_conditional_resolution_scalability() {
        // Test that conditional resolution scales reasonably
        let chain_lengths = vec![5, 10, 20, 35];
        let mut previous_duration = Duration::from_nanos(0);

        for length in chain_lengths {
            let parameters = create_conditional_parameter_chain(length);
            let resolver = DefaultParameterResolver::new();
            let cli_args = HashMap::new();

            let start = Instant::now();
            let result = resolver.resolve_parameters(&parameters, &cli_args, false);
            let duration = start.elapsed();

            assert!(result.is_ok());
            let resolved = result.unwrap();
            assert_eq!(resolved.len(), length + 1); // +1 for trigger

            println!("Conditional chain of {length} took: {duration:?}");

            if length > 5 {
                // Ensure scaling is roughly linear, not exponential
                let ratio = duration.as_nanos() as f64 / previous_duration.as_nanos() as f64;
                assert!(
                    ratio < 5.0,
                    "Conditional resolution scaled poorly: {length} chain took {ratio:.2}x longer than previous"
                );
            }

            previous_duration = duration;
        }
    }
}