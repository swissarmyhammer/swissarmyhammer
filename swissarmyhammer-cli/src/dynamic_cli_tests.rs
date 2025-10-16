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

/// Helper function to extract help text from a CLI command
fn get_help_text(cli: &Command) -> String {
    use clap::error::ErrorKind;

    // Try to get matches which will fail with help error
    match cli
        .clone()
        .try_get_matches_from(vec!["swissarmyhammer", "--help"])
    {
        Err(e) if e.kind() == ErrorKind::DisplayHelp => {
            format!("{}", e)
        }
        Ok(_) => panic!("Expected help error"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_mcp_tool_categories_appear_in_help() {
    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry.clone());

    // Build CLI without workflows
    let cli = builder.build_cli(None);

    // Get help text
    let help = get_help_text(&cli);

    // Verify MCP tool categories appear in help text
    let categories = registry.get_cli_categories();
    for category in categories {
        assert!(
            help.contains(&category),
            "Help text should contain MCP tool category '{}'",
            category
        );
    }
}

#[test]
fn test_workflow_shortcuts_appear_in_help_when_present() {
    use swissarmyhammer_workflow::WorkflowStorage;

    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry);

    // Try to load workflow storage
    if let Ok(storage) = WorkflowStorage::file_system() {
        // Build CLI with workflows
        let cli = builder.build_cli(Some(&storage));

        // Get help text
        let help = get_help_text(&cli);

        // If workflows are available, verify they appear in help
        let workflows = storage.list_workflows().unwrap_or_default();
        if !workflows.is_empty() {
            // Just verify at least one workflow appears
            let has_workflow = workflows.iter().any(|w| help.contains(&w.name.to_string()));
            assert!(
                has_workflow,
                "Help text should contain workflow shortcuts when workflows are present"
            );
        }
    }
}

#[test]
fn test_static_commands_appear_before_mcp_tools() {
    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry.clone());

    // Build CLI
    let cli = builder.build_cli(None);

    // Get help text
    let help = get_help_text(&cli);

    // Find positions of static command (serve) and first MCP tool category
    let serve_pos = help.find("serve").expect("'serve' should appear in help");

    // Find first MCP tool category
    let categories = registry.get_cli_categories();
    if !categories.is_empty() {
        let first_category = &categories[0];
        if let Some(tool_pos) = help.find(first_category) {
            // Static commands should appear before MCP tool categories
            assert!(
                serve_pos < tool_pos,
                "Static commands should appear before MCP tool categories"
            );
        }
    }
}

#[test]
fn test_workflows_appear_before_mcp_tools_when_present() {
    use swissarmyhammer_workflow::WorkflowStorage;

    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry.clone());

    // Try to load workflow storage
    if let Ok(storage) = WorkflowStorage::file_system() {
        let workflows = storage.list_workflows().unwrap_or_default();

        // Only test if workflows are actually present
        if !workflows.is_empty() {
            // Build CLI with workflows
            let cli = builder.build_cli(Some(&storage));

            // Get help text
            let help = get_help_text(&cli);

            // Find position of first workflow and first MCP tool category
            if let Some(first_workflow) = workflows.first() {
                let workflow_pos = help.find(&first_workflow.name.to_string());

                let categories = registry.get_cli_categories();
                if !categories.is_empty() {
                    let first_category_pos = help.find(&categories[0]);

                    if let (Some(wf_pos), Some(cat_pos)) = (workflow_pos, first_category_pos) {
                        // Workflow should appear before MCP tool category
                        assert!(
                            wf_pos < cat_pos,
                            "Workflows should appear before MCP tool categories"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_mcp_tool_categories_are_sorted() {
    // Create a tool registry and CLI builder
    let registry = Arc::new(ToolRegistry::new());
    let builder = CliBuilder::new(registry.clone());

    // Build CLI
    let cli = builder.build_cli(None);

    // Get all MCP tool category names
    let categories = registry.get_cli_categories();
    if categories.len() > 1 {
        // Get the category names from the built CLI
        let subcommand_names: Vec<&str> = cli.get_subcommands().map(|cmd| cmd.get_name()).collect();

        // Filter to only MCP tool categories
        let mut mcp_categories: Vec<String> = categories.iter().map(|s| s.to_string()).collect();
        mcp_categories.sort();

        // Find first MCP category position
        if let Some(first_cat_pos) = subcommand_names
            .iter()
            .position(|name| *name == mcp_categories[0])
        {
            // Check that subsequent MCP categories appear in sorted order
            for cat in mcp_categories.iter().skip(1) {
                if let Some(cat_pos) = subcommand_names.iter().position(|name| *name == cat) {
                    // Each category should appear after the previous one
                    assert!(
                        cat_pos > first_cat_pos,
                        "MCP tool categories should be sorted alphabetically"
                    );
                }
            }
        }
    }
}

#[test]
fn test_workflow_shortcuts_are_sorted_when_built() {
    use swissarmyhammer_workflow::WorkflowStorage;

    // Try to load workflow storage
    if let Ok(storage) = WorkflowStorage::file_system() {
        let workflows = storage.list_workflows().unwrap_or_default();

        // Only test if we have multiple workflows
        if workflows.len() > 1 {
            // Create a tool registry and CLI builder
            let registry = Arc::new(ToolRegistry::new());
            let builder = CliBuilder::new(registry);

            // Build CLI with workflows - this is where sorting happens
            let cli = builder.build_cli(Some(&storage));

            // Filter to workflow commands (those with "shortcut for 'flow" in their about text)
            let workflow_names: Vec<&str> = cli
                .get_subcommands()
                .filter(|cmd| {
                    cmd.get_about()
                        .is_some_and(|about| about.to_string().contains("shortcut for 'flow"))
                })
                .map(|cmd| cmd.get_name())
                .collect();

            // If we have workflows, verify they appear in sorted order
            if workflow_names.len() > 1 {
                let mut sorted_workflow_names = workflow_names.clone();
                sorted_workflow_names.sort();

                assert_eq!(
                    workflow_names, sorted_workflow_names,
                    "Workflow shortcuts should be sorted alphabetically"
                );
            }
        }
    }
}
