//! Unit tests for CLI generation components with exclusion support
//!
//! These tests validate the CLI generation system's ability to respect exclusion
//! markers and generate appropriate CLI commands from non-excluded tools.

use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_cli::generation::{
    CliGenerator, CommandBuilder, GeneratedCommand, GenerationConfig, GenerationError,
    NamingStrategy, ParseError,
};
use swissarmyhammer_tools::cli::{RegistryCliExclusionDetector, ToolCliMetadata};
use swissarmyhammer_tools::ToolRegistry;
use super::super::common::test_utils::{CliExclusionTestEnvironment, TestRegistryFixture};

/// Test CommandBuilder schema parsing with various tool schemas
mod command_builder_tests {
    use super::*;

    #[test]
    fn test_parse_simple_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Tool name parameter"
                }
            },
            "required": ["name"]
        });

        let result = CommandBuilder::parse_schema("test_tool", &schema);
        assert!(result.is_ok(), "Simple schema parsing should succeed");

        let command = result.unwrap();
        assert_eq!(command.tool_name, "test_tool");
        assert_eq!(command.arguments.len(), 1);
        
        let arg = &command.arguments[0];
        assert_eq!(arg.name, "name");
        assert!(arg.required);
        assert_eq!(arg.value_type, "string");
    }

    #[test]
    fn test_parse_complex_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "required_param": {
                    "type": "string", 
                    "description": "Required parameter"
                },
                "optional_param": {
                    "type": "integer",
                    "description": "Optional parameter",
                    "default": 42
                },
                "boolean_flag": {
                    "type": "boolean",
                    "description": "Boolean flag parameter"
                },
                "enum_param": {
                    "type": "string",
                    "enum": ["option1", "option2", "option3"],
                    "description": "Enumerated parameter"
                }
            },
            "required": ["required_param"]
        });

        let result = CommandBuilder::parse_schema("complex_tool", &schema);
        assert!(result.is_ok(), "Complex schema parsing should succeed");

        let command = result.unwrap();
        assert_eq!(command.arguments.len(), 4);

        // Find required parameter
        let required_arg = command.arguments
            .iter()
            .find(|arg| arg.name == "required-param")
            .expect("Required parameter should exist");
        assert!(required_arg.required);

        // Find optional parameter
        let optional_arg = command.arguments
            .iter()
            .find(|arg| arg.name == "optional-param")
            .expect("Optional parameter should exist");
        assert!(!optional_arg.required);

        // Find boolean parameter
        let boolean_arg = command.arguments
            .iter()
            .find(|arg| arg.name == "boolean-flag")
            .expect("Boolean parameter should exist");
        assert_eq!(boolean_arg.value_type, "boolean");

        // Find enum parameter
        let enum_arg = command.arguments
            .iter()
            .find(|arg| arg.name == "enum-param")
            .expect("Enum parameter should exist");
        assert!(enum_arg.has_constraints());
    }

    #[test]
    fn test_parse_invalid_schema() {
        let invalid_schema = serde_json::json!({
            "type": "invalid_type",
            "properties": "not_an_object"
        });

        let result = CommandBuilder::parse_schema("invalid_tool", &invalid_schema);
        assert!(result.is_err(), "Invalid schema should return error");

        match result.unwrap_err() {
            GenerationError::ParseError(_) => {}, // Expected
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_empty_schema() {
        let empty_schema = serde_json::json!({});

        let result = CommandBuilder::parse_schema("empty_tool", &empty_schema);
        assert!(result.is_ok(), "Empty schema should be valid");

        let command = result.unwrap();
        assert!(command.arguments.is_empty());
        assert!(command.options.is_empty());
    }

    #[test]
    fn test_argument_naming_conversion() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "snake_case_param": {"type": "string"},
                "camelCaseParam": {"type": "string"},
                "kebab-case-param": {"type": "string"},
                "UPPER_CASE_PARAM": {"type": "string"}
            }
        });

        let result = CommandBuilder::parse_schema("naming_tool", &schema).unwrap();
        
        // All parameter names should be converted to kebab-case for CLI
        let expected_names = [
            "snake-case-param",
            "camel-case-param", 
            "kebab-case-param",
            "upper-case-param"
        ];

        for expected_name in &expected_names {
            assert!(
                result.arguments.iter().any(|arg| arg.name == *expected_name),
                "Expected parameter name '{}' not found",
                expected_name
            );
        }
    }

    #[test]
    fn test_schema_with_nested_objects() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "simple_param": {"type": "string"},
                "nested_object": {
                    "type": "object",
                    "properties": {
                        "inner_param": {"type": "string"}
                    }
                }
            }
        });

        let result = CommandBuilder::parse_schema("nested_tool", &schema);
        assert!(result.is_ok(), "Nested object schema should parse successfully");

        let command = result.unwrap();
        // Nested objects might be flattened or handled specially
        assert!(!command.arguments.is_empty());
    }
}

/// Test CliGenerator with exclusion detection
mod cli_generator_tests {
    use super::*;

    #[test]
    fn test_generator_with_empty_registry() {
        let empty_registry = Arc::new(ToolRegistry::new());
        let generator = CliGenerator::new(empty_registry);

        let result = generator.generate_commands();
        assert!(result.is_ok(), "Empty registry should generate empty command list");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_generator_respects_exclusions() {
        let env = CliExclusionTestEnvironment::new();
        let generator = CliGenerator::new(Arc::new(env.fixture.registry));

        let result = generator.generate_commands();
        assert!(result.is_ok(), "Generation should succeed");

        let commands = result.unwrap();
        let command_tool_names: Vec<&String> = commands
            .iter()
            .map(|cmd| &cmd.tool_name)
            .collect();

        // No excluded tools should appear in generated commands
        for excluded_name in &env.fixture.excluded_tool_names {
            assert!(
                !command_tool_names.contains(&excluded_name),
                "Excluded tool '{}' should not generate a command",
                excluded_name
            );
        }

        // All included tools should appear in generated commands
        for included_name in &env.fixture.included_tool_names {
            assert!(
                command_tool_names.contains(&included_name),
                "Included tool '{}' should generate a command",
                included_name
            );
        }
    }

    #[test]
    fn test_generator_with_custom_config() {
        let env = CliExclusionTestEnvironment::new();
        
        let config = GenerationConfig {
            naming_strategy: NamingStrategy::Flatten,
            command_prefix: Some("test".to_string()),
            use_subcommands: false,
            max_commands: 100,
        };

        let generator = CliGenerator::new(Arc::new(env.fixture.registry))
            .with_config(config);

        let result = generator.generate_commands();
        assert!(result.is_ok(), "Custom config generation should succeed");

        let commands = result.unwrap();
        
        // Commands should have the prefix
        for command in &commands {
            assert!(
                command.name.starts_with("test-"),
                "Command '{}' should have prefix 'test-'",
                command.name
            );
        }
    }

    #[test]
    fn test_generator_with_max_command_limit() {
        let env = CliExclusionTestEnvironment::with_tool_counts(0, 10); // 10 included tools
        
        let config = GenerationConfig {
            max_commands: 3, // Limit to 3 commands
            ..Default::default()
        };

        let generator = CliGenerator::new(Arc::new(env.fixture.registry))
            .with_config(config);

        let result = generator.generate_commands();
        
        if env.fixture.included_tool_names.len() > 3 {
            // Should hit the limit and return an error
            assert!(result.is_err(), "Should return error when exceeding max commands");
            
            match result.unwrap_err() {
                GenerationError::TooManyCommands { .. } => {}, // Expected
                other => panic!("Expected TooManyCommands error, got {:?}", other),
            }
        } else {
            // Should succeed if we have few enough tools
            assert!(result.is_ok());
            let commands = result.unwrap();
            assert!(commands.len() <= 3);
        }
    }

    #[test]
    fn test_generator_naming_strategies() {
        let env = CliExclusionTestEnvironment::new();
        let registry = Arc::new(env.fixture.registry);

        // Test KeepOriginal strategy
        let keep_original_config = GenerationConfig {
            naming_strategy: NamingStrategy::KeepOriginal,
            ..Default::default()
        };
        let generator = CliGenerator::new(registry.clone()).with_config(keep_original_config);
        let original_commands = generator.generate_commands().unwrap();

        // Test GroupByDomain strategy
        let group_by_domain_config = GenerationConfig {
            naming_strategy: NamingStrategy::GroupByDomain,
            use_subcommands: true,
            ..Default::default()
        };
        let generator = CliGenerator::new(registry.clone()).with_config(group_by_domain_config);
        let domain_commands = generator.generate_commands().unwrap();

        // Test Flatten strategy
        let flatten_config = GenerationConfig {
            naming_strategy: NamingStrategy::Flatten,
            ..Default::default()
        };
        let generator = CliGenerator::new(registry.clone()).with_config(flatten_config);
        let flattened_commands = generator.generate_commands().unwrap();

        // All strategies should respect exclusions and generate the same number of commands
        // (though their structure may differ)
        let expected_count = env.fixture.included_tool_names.len();
        
        // Allow for flexibility in naming strategies - some might create hierarchies
        assert!(original_commands.len() >= expected_count);
        assert!(domain_commands.len() >= expected_count); 
        assert!(flattened_commands.len() >= expected_count);

        // All generated commands should be for included tools only
        for command in &original_commands {
            assert!(
                env.fixture.included_tool_names.contains(&command.tool_name),
                "Generated command should be for included tool"
            );
        }
    }

    #[test]
    fn test_generator_error_handling() {
        let env = CliExclusionTestEnvironment::new();

        // Test invalid prefix
        let invalid_config = GenerationConfig {
            command_prefix: Some("".to_string()), // Empty prefix should be invalid
            ..Default::default()
        };

        let generator = CliGenerator::new(Arc::new(env.fixture.registry))
            .with_config(invalid_config);

        let result = generator.generate_commands();
        assert!(result.is_err(), "Empty prefix should cause validation error");

        match result.unwrap_err() {
            GenerationError::ConfigValidation(_) => {}, // Expected
            other => panic!("Expected ConfigValidation error, got {:?}", other),
        }
    }

    #[test]
    fn test_generated_command_structure() {
        let env = CliExclusionTestEnvironment::new();
        let generator = CliGenerator::new(Arc::new(env.fixture.registry));

        let result = generator.generate_commands();
        assert!(result.is_ok());

        let commands = result.unwrap();
        assert!(!commands.is_empty(), "Should generate some commands");

        for command in &commands {
            // Validate command structure
            assert!(!command.name.is_empty(), "Command name should not be empty");
            assert!(!command.tool_name.is_empty(), "Tool name should not be empty");
            
            // Command names should use kebab-case
            assert!(
                !command.name.contains('_'),
                "Command name '{}' should not contain underscores",
                command.name
            );

            // Arguments should be properly ordered (required first)
            let mut found_optional = false;
            for arg in &command.arguments {
                if !arg.required {
                    found_optional = true;
                } else if found_optional {
                    panic!(
                        "Required argument '{}' found after optional arguments in command '{}'",
                        arg.name, command.name
                    );
                }
            }

            // All argument and option names should use kebab-case
            for arg in &command.arguments {
                assert!(
                    !arg.name.contains('_'),
                    "Argument name '{}' should not contain underscores",
                    arg.name
                );
            }

            for option in &command.options {
                assert!(
                    !option.name.contains('_'),
                    "Option name '{}' should not contain underscores",
                    option.name
                );
            }
        }
    }
}

/// Test GenerationConfig validation and defaults
mod generation_config_tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GenerationConfig::default();
        
        assert_eq!(config.naming_strategy, NamingStrategy::KeepOriginal);
        assert!(config.command_prefix.is_none());
        assert!(!config.use_subcommands);
        assert_eq!(config.max_commands, 1000);
    }

    #[test]
    fn test_config_validation_valid() {
        let valid_configs = vec![
            GenerationConfig::default(),
            GenerationConfig {
                naming_strategy: NamingStrategy::GroupByDomain,
                command_prefix: Some("prefix".to_string()),
                use_subcommands: true,
                max_commands: 50,
            },
            GenerationConfig {
                naming_strategy: NamingStrategy::Flatten,
                command_prefix: None,
                use_subcommands: false,
                max_commands: 1,
            },
        ];

        for config in valid_configs {
            let result = config.validate();
            assert!(result.is_ok(), "Valid config should pass validation: {:?}", config);
        }
    }

    #[test]
    fn test_config_validation_invalid() {
        let invalid_configs = vec![
            GenerationConfig {
                command_prefix: Some("".to_string()), // Empty prefix
                ..Default::default()
            },
            GenerationConfig {
                command_prefix: Some("invalid prefix with spaces".to_string()), // Spaces
                ..Default::default()
            },
            GenerationConfig {
                max_commands: 0, // Zero max commands
                ..Default::default()
            },
        ];

        for config in invalid_configs {
            let result = config.validate();
            assert!(result.is_err(), "Invalid config should fail validation: {:?}", config);
        }
    }

    #[test]
    fn test_naming_strategy_enum() {
        // Test all variants exist and can be created
        let strategies = vec![
            NamingStrategy::KeepOriginal,
            NamingStrategy::GroupByDomain,
            NamingStrategy::Flatten,
        ];

        for strategy in strategies {
            let config = GenerationConfig {
                naming_strategy: strategy,
                ..Default::default()
            };
            assert!(config.validate().is_ok());
        }
    }
}

/// Test error types and handling
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_generation_error_types() {
        // Test that all error variants can be created
        let errors = vec![
            GenerationError::ParseError(ParseError::InvalidSchema("test".to_string())),
            GenerationError::ConfigValidation("test validation error".to_string()),
            GenerationError::TooManyCommands {
                limit: 10,
                actual: 15,
            },
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty(), "Error should have non-empty display");
            
            // Test debug formatting
            let debug_string = format!("{:?}", error);
            assert!(!debug_string.is_empty(), "Error should have debug representation");
        }
    }

    #[test]
    fn test_parse_error_types() {
        let parse_errors = vec![
            ParseError::InvalidSchema("invalid schema".to_string()),
            ParseError::UnsupportedFeature("unsupported feature".to_string()),
            ParseError::MissingRequiredField("missing field".to_string()),
        ];

        for error in parse_errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
    }

    #[test]
    fn test_error_source_chain() {
        let parse_error = ParseError::InvalidSchema("root cause".to_string());
        let generation_error = GenerationError::ParseError(parse_error);

        let error_string = generation_error.to_string();
        assert!(error_string.contains("root cause"));

        // Test error source chain
        let source = generation_error.source();
        assert!(source.is_some());
    }
}