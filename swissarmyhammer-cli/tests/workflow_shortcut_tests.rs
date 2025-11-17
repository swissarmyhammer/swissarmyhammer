//! Tests for workflow shortcut generation and execution
//!
//! These tests verify that workflow shortcuts are correctly generated and can
//! execute workflows without requiring the 'flow' prefix.

use std::sync::Arc;
use swissarmyhammer_cli::dynamic_cli::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_workflow::WorkflowStorage;

#[test]
fn test_shortcut_generation() {
    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Get shortcuts
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);

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
    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Get shortcuts
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);

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
    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Get shortcuts
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);

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
    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Get shortcuts
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);

    // Each shortcut should have standard workflow flags
    for shortcut in &shortcuts {
        let args: Vec<_> = shortcut.get_arguments().collect();

        // Check for --interactive/-i
        let has_interactive = args
            .iter()
            .any(|arg| arg.get_long() == Some("interactive") || arg.get_short() == Some('i'));
        assert!(
            has_interactive,
            "Shortcut '{}' should have --interactive flag",
            shortcut.get_name()
        );

        // Check for --dry-run
        let has_dry_run = args.iter().any(|arg| arg.get_long() == Some("dry-run"));
        assert!(
            has_dry_run,
            "Shortcut '{}' should have --dry-run flag",
            shortcut.get_name()
        );

        // Check for --quiet/-q
        let has_quiet = args
            .iter()
            .any(|arg| arg.get_long() == Some("quiet") || arg.get_short() == Some('q'));
        assert!(
            has_quiet,
            "Shortcut '{}' should have --quiet flag",
            shortcut.get_name()
        );

        // Check for --param/-p
        let has_param = args
            .iter()
            .any(|arg| arg.get_long() == Some("param") || arg.get_short() == Some('p'));
        assert!(
            has_param,
            "Shortcut '{}' should have --param flag",
            shortcut.get_name()
        );
    }
}

#[test]
fn test_cli_builder_integration() {
    // Create a tool registry and CLI builder
    let tool_registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
    let cli_builder = CliBuilder::new(tool_registry);

    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Build CLI with shortcuts
    let cli = cli_builder.build_cli(Some(&storage));

    // Get all subcommands
    let subcommands: Vec<_> = cli.get_subcommands().collect();

    // Should have static commands
    let has_flow = subcommands.iter().any(|cmd| cmd.get_name() == "flow");
    assert!(has_flow, "CLI should have 'flow' command");

    let has_serve = subcommands.iter().any(|cmd| cmd.get_name() == "serve");
    assert!(has_serve, "CLI should have 'serve' command");

    // Should have workflow shortcuts in addition to static commands
    assert!(
        subcommands.len() > 10,
        "CLI should have static commands plus workflow shortcuts"
    );
}

#[test]
fn test_cli_builder_without_workflow_storage() {
    // Create a tool registry and CLI builder
    let tool_registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
    let cli_builder = CliBuilder::new(tool_registry);

    // Build CLI without shortcuts (workflow_storage = None)
    let cli = cli_builder.build_cli(None);

    // Get all subcommands
    let subcommands: Vec<_> = cli.get_subcommands().collect();

    // Should still have static commands
    let has_flow = subcommands.iter().any(|cmd| cmd.get_name() == "flow");
    assert!(has_flow, "CLI should have 'flow' command");

    let has_serve = subcommands.iter().any(|cmd| cmd.get_name() == "serve");
    assert!(has_serve, "CLI should have 'serve' command");

    // Should work fine without workflow storage
    assert!(!subcommands.is_empty(), "CLI should have some commands");
}

#[test]
fn test_shortcut_positional_args_for_required_params() {
    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Get workflows to check which have required parameters
    let workflows = storage.list_workflows().expect("Failed to list workflows");

    // Get shortcuts
    let shortcuts = CliBuilder::build_workflow_shortcuts(&storage);

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
    // Create a tool registry and CLI builder
    let tool_registry = Arc::new(tokio::sync::RwLock::new(ToolRegistry::new()));
    let cli_builder = CliBuilder::new(tool_registry);

    // Create workflow storage
    let storage = WorkflowStorage::file_system().expect("Failed to create workflow storage");

    // Build CLI with warnings and workflow storage
    let cli = cli_builder.build_cli_with_warnings(Some(&storage));

    // Should have built successfully
    let subcommands: Vec<_> = cli.get_subcommands().collect();
    assert!(!subcommands.is_empty(), "CLI should have commands");

    // Should have workflow shortcuts
    let has_static_commands = subcommands.iter().any(|cmd| cmd.get_name() == "flow");
    assert!(has_static_commands, "Should have static commands");
}
