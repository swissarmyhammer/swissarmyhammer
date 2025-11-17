//! Tests for workflow shortcut generation and execution
//!
//! These tests verify that workflow shortcuts are correctly generated and can
//! execute workflows without requiring the 'flow' prefix.

use clap::Command;
use std::sync::Arc;
use swissarmyhammer_cli::dynamic_cli::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_workflow::WorkflowStorage;

// Test helper functions

/// Creates test workflow storage and generates shortcuts
fn create_test_shortcuts() -> (WorkflowStorage, Vec<Command>) {
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);
    (storage, shortcuts)
}

/// Creates test CLI builder with tool registry
fn create_test_cli_builder() -> (Arc<tokio::sync::RwLock<ToolRegistry>>, CliBuilder) {
    let tool_registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
    let cli_builder = CliBuilder::new(tool_registry.clone());
    (tool_registry, cli_builder)
}

/// Asserts that a shortcut has a specific flag (long and optionally short form)
fn assert_shortcut_has_flag(shortcut: &Command, flag_long: &str, flag_short: Option<char>) {
    let args: Vec<_> = shortcut.get_arguments().collect();
    let has_flag = args.iter().any(|arg| {
        arg.get_long() == Some(flag_long) || (flag_short.is_some() && arg.get_short() == flag_short)
    });
    assert!(
        has_flag,
        "Shortcut '{}' should have --{} flag",
        shortcut.get_name(),
        flag_long
    );
}

/// Asserts that a CLI has a specific command
fn assert_has_command(cli: &Command, command_name: &str) {
    let subcommands: Vec<_> = cli.get_subcommands().collect();
    let has_command = subcommands.iter().any(|cmd| cmd.get_name() == command_name);
    assert!(has_command, "CLI should have '{}' command", command_name);
    assert!(!subcommands.is_empty(), "CLI should have some commands");
}

#[test]
fn test_shortcut_generation() {
    let (_storage, shortcuts) = create_test_shortcuts();

    // Should have generated shortcuts for available workflows
    assert!(
        !shortcuts.is_empty(),
        "Should generate at least some shortcuts"
    );

    // Check that each shortcut is a Command
    for shortcut in shortcuts {
        let name = shortcut.get_name();
        assert!(!name.is_empty(), "Shortcut should have a name");
    }
}

#[test]
fn test_name_conflict_resolution() {
    let (storage, shortcuts) = create_test_shortcuts();

    // Reserved names that should get underscore prefix
    // Note: plan and implement are no longer reserved since static commands were removed
    let reserved = [
        "serve", "doctor", "prompt", "rule", "flow", "agent", "validate", "list",
    ];

    // Check if any workflow has a reserved name
    let workflows = storage.list_workflows().expect("Failed to list workflows");
    for workflow in &workflows {
        let workflow_name = workflow.name.to_string();
        if reserved.contains(&workflow_name.as_str()) {
            // Should find a shortcut with underscore prefix
            let prefixed_name = format!("_{}", workflow_name);
            let found = shortcuts.iter().any(|cmd| cmd.get_name() == prefixed_name);
            assert!(
                found,
                "Reserved name '{}' should have underscore prefix '{}'",
                workflow_name, prefixed_name
            );
        }
    }
}

#[test]
fn test_shortcut_has_proper_about_text() {
    let (_storage, shortcuts) = create_test_shortcuts();

    // Each shortcut should have about text mentioning it's a shortcut
    for shortcut in shortcuts {
        let about = shortcut.get_about();
        if let Some(about_text) = about {
            let about_str = about_text.to_string();
            assert!(
                about_str.contains("shortcut for 'flow"),
                "Shortcut '{}' should mention it's a shortcut for flow command, got: {}",
                shortcut.get_name(),
                about_str
            );
        }
    }
}

#[test]
fn test_shortcut_has_standard_flags() {
    let (_storage, shortcuts) = create_test_shortcuts();

    // Each shortcut should have standard workflow flags
    for shortcut in &shortcuts {
        assert_shortcut_has_flag(shortcut, "interactive", Some('i'));
        assert_shortcut_has_flag(shortcut, "dry-run", None);
        assert_shortcut_has_flag(shortcut, "quiet", Some('q'));
        assert_shortcut_has_flag(shortcut, "param", Some('p'));
    }
}

#[test]
fn test_cli_builder_integration() {
    let (_tool_registry, cli_builder) = create_test_cli_builder();
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Build CLI with shortcuts
    let cli = cli_builder.build_cli(Some(&storage));

    // Should have static commands
    assert_has_command(&cli, "flow");
    assert_has_command(&cli, "serve");

    // Should have workflow shortcuts in addition to static commands
    let subcommands: Vec<_> = cli.get_subcommands().collect();
    assert!(
        subcommands.len() > 10,
        "CLI should have static commands plus workflow shortcuts"
    );
}

#[test]
fn test_cli_builder_without_workflow_storage() {
    let (_tool_registry, cli_builder) = create_test_cli_builder();

    // Build CLI without shortcuts (workflow_storage = None)
    let cli = cli_builder.build_cli(None);

    // Should still have static commands and work fine without workflow storage
    assert_has_command(&cli, "flow");
    assert_has_command(&cli, "serve");
}

#[test]
fn test_shortcut_positional_args_for_required_params() {
    let (storage, shortcuts) = create_test_shortcuts();

    // Get workflows to check which have required parameters
    let workflows = storage.list_workflows().expect("Failed to list workflows");

    // For each workflow with required parameters, check positional args
    for workflow in workflows {
        let required_params: Vec<_> = workflow.parameters.iter().filter(|p| p.required).collect();

        if !required_params.is_empty() {
            // Find corresponding shortcut
            let workflow_name = workflow.name.to_string();
            let shortcut = shortcuts.iter().find(|cmd| {
                let name = cmd.get_name();
                name == workflow_name || name == format!("_{}", workflow_name)
            });

            if let Some(shortcut) = shortcut {
                let args: Vec<_> = shortcut.get_arguments().collect();

                // Should have a positional argument
                let has_positional = args.iter().any(|arg| arg.get_id() == "positional");

                assert!(
                    has_positional,
                    "Workflow '{}' with required params should have positional args",
                    workflow_name
                );
            }
        }
    }
}

#[test]
fn test_build_cli_with_warnings_accepts_workflow_storage() {
    let (_tool_registry, cli_builder) = create_test_cli_builder();
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Build CLI with warnings and workflow storage
    let cli = cli_builder.build_cli_with_warnings(Some(&storage));

    // Should have built successfully with workflow shortcuts and static commands
    assert_has_command(&cli, "flow");
}
