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

/// Finds a shortcut for a workflow by name, handling underscore prefix for reserved names
fn find_shortcut_for_workflow<'a>(
    shortcuts: &'a [Command],
    workflow_name: &str,
) -> Option<&'a Command> {
    shortcuts.iter().find(|cmd| {
        let name = cmd.get_name();
        name == workflow_name || name == format!("_{}", workflow_name)
    })
}

/// Asserts that a shortcut has all standard workflow flags
fn assert_has_standard_flags(shortcut: &Command) {
    let expected_flags = [
        ("interactive", Some('i')),
        ("dry-run", None),
        ("quiet", Some('q')),
        ("param", Some('p')),
    ];

    for (flag_long, flag_short) in expected_flags {
        assert_shortcut_has_flag(shortcut, flag_long, flag_short);
    }
}

/// Builds CLI with workflow storage and verifies basic setup
fn build_and_verify_basic_cli(
    cli_builder: &CliBuilder,
    storage: Option<&WorkflowStorage>,
) -> Command {
    let cli = cli_builder.build_cli(storage);

    // Verify basic static commands exist
    assert_has_command(&cli, "flow");
    assert_has_command(&cli, "serve");

    cli
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
            let found = find_shortcut_for_workflow(&shortcuts, &workflow_name);
            assert!(
                found.is_some(),
                "Reserved name '{}' should have a shortcut (possibly with underscore prefix)",
                workflow_name
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
        assert_has_standard_flags(shortcut);
    }
}

#[test]
fn test_cli_builder_integration() {
    let (_tool_registry, cli_builder) = create_test_cli_builder();
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Build CLI with shortcuts and verify basic setup
    let cli = build_and_verify_basic_cli(&cli_builder, Some(&storage));

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

    // Build CLI without shortcuts (workflow_storage = None) and verify basic setup
    build_and_verify_basic_cli(&cli_builder, None);
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
            let shortcut = find_shortcut_for_workflow(&shortcuts, &workflow_name);

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
    assert_has_command(&cli, "serve");
}
