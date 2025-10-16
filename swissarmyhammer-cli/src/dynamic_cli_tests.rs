//! Tests for dynamic CLI builder

use super::*;
use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

#[test]
fn test_string_interning_deduplication() {
    // Test that the same string is only leaked once
    let s1 = intern_string("test_string".to_string());
    let s2 = intern_string("test_string".to_string());

    // Both should point to the same memory address
    assert_eq!(s1 as *const str, s2 as *const str);
}

#[test]
fn test_string_interning_different_strings() {
    // Test that different strings get different addresses
    let s1 = intern_string("string1".to_string());
    let s2 = intern_string("string2".to_string());

    // Should have different memory addresses
    assert_ne!(s1 as *const str, s2 as *const str);
}

#[test]
fn test_validation_stats_all_valid() {
    let stats = CliValidationStats {
        total_tools: 10,
        valid_tools: 10,
        invalid_tools: 0,
        validation_errors: 0,
    };

    assert!(stats.is_all_valid());
    assert_eq!(stats.success_rate(), 100.0);

    let summary = stats.summary();
    assert!(summary.contains("✅"));
    assert!(summary.contains("All 10 CLI tools are valid"));
}

#[test]
fn test_validation_stats_some_invalid() {
    let stats = CliValidationStats {
        total_tools: 10,
        valid_tools: 7,
        invalid_tools: 3,
        validation_errors: 5,
    };

    assert!(!stats.is_all_valid());
    assert_eq!(stats.success_rate(), 70.0);

    let summary = stats.summary();
    assert!(summary.contains("⚠️"));
    assert!(summary.contains("7 of 10"));
    assert!(summary.contains("70.0%"));
    assert!(summary.contains("5 validation errors"));
}

#[test]
fn test_validation_stats_zero_tools() {
    let stats = CliValidationStats {
        total_tools: 0,
        valid_tools: 0,
        invalid_tools: 0,
        validation_errors: 0,
    };

    assert!(stats.is_all_valid());
    assert_eq!(stats.success_rate(), 100.0); // Should handle division by zero gracefully
}

#[test]
fn test_cli_builder_creates_tool_registry() {
    // Create a tool registry
    let registry = Arc::new(ToolRegistry::new());

    // Create CLI builder - this tests that CliBuilder::new() succeeds
    let _builder = CliBuilder::new(registry.clone());

    // Builder should be created successfully without panicking
}

#[test]
fn test_cli_builder_graceful_degradation() {
    // Create a tool registry
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry);

    // Build CLI with warnings should not panic even with no workflows
    let cli = builder.build_cli_with_warnings(None);

    // Should successfully create CLI
    assert_eq!(cli.get_name(), "swissarmyhammer");

    // Should have basic structure
    assert!(cli.get_subcommands().any(|cmd| cmd.get_name() == "serve"));
    assert!(cli.get_subcommands().any(|cmd| cmd.get_name() == "doctor"));
}

#[test]
fn test_precompute_arg_data_types() {
    // Test boolean type
    let bool_schema = json!({
        "type": "boolean",
        "description": "Enable feature"
    });
    let bool_arg = CliBuilder::precompute_arg_data("feature_flag", &bool_schema, false);
    assert!(matches!(bool_arg.arg_type, ArgType::Boolean));
    assert_eq!(bool_arg.help, Some("Enable feature".to_string()));

    // Test integer type
    let int_schema = json!({
        "type": "integer",
        "description": "Port number"
    });
    let int_arg = CliBuilder::precompute_arg_data("port", &int_schema, true);
    assert!(matches!(int_arg.arg_type, ArgType::Integer));
    assert!(int_arg.is_required);

    // Test array type
    let array_schema = json!({
        "type": "array",
        "description": "List of files"
    });
    let array_arg = CliBuilder::precompute_arg_data("files", &array_schema, false);
    assert!(matches!(array_arg.arg_type, ArgType::Array));

    // Test enum values
    let enum_schema = json!({
        "type": "string",
        "enum": ["dev", "staging", "prod"],
        "description": "Environment"
    });
    let enum_arg = CliBuilder::precompute_arg_data("env", &enum_schema, true);
    assert!(matches!(enum_arg.arg_type, ArgType::String));
    assert_eq!(
        enum_arg.possible_values,
        Some(vec![
            "dev".to_string(),
            "staging".to_string(),
            "prod".to_string()
        ])
    );

    // Test default value
    let default_schema = json!({
        "type": "string",
        "default": "localhost",
        "description": "Host"
    });
    let default_arg = CliBuilder::precompute_arg_data("host", &default_schema, false);
    assert_eq!(default_arg.default_value, Some("localhost".to_string()));
}

#[test]
fn test_build_cli_basic_structure() {
    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry);

    // Build CLI
    let cli = builder.build_cli(None);

    // Verify basic structure
    assert_eq!(cli.get_name(), "swissarmyhammer");

    // Verify core subcommands exist
    let subcommand_names: Vec<&str> = cli.get_subcommands().map(|cmd| cmd.get_name()).collect();

    assert!(subcommand_names.contains(&"serve"));
    assert!(subcommand_names.contains(&"doctor"));
    assert!(subcommand_names.contains(&"prompt"));
    assert!(subcommand_names.contains(&"flow"));
    assert!(subcommand_names.contains(&"validate"));
    assert!(subcommand_names.contains(&"plan"));
    assert!(subcommand_names.contains(&"implement"));
    assert!(subcommand_names.contains(&"agent"));
    assert!(subcommand_names.contains(&"rule"));
}
