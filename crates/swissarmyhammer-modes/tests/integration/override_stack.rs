//! Tests for mode override precedence stack
//!
//! Verifies that modes follow the standard SwissArmyHammer override precedence:
//! builtin → user → project (later overrides earlier)

use swissarmyhammer_modes::ModeRegistry;

#[test]
fn test_builtin_modes_load() {
    let mut registry = ModeRegistry::new();
    let modes = registry.load_all().unwrap();

    // Define expected modes explicitly
    // Note: rule-checker mode removed as part of swissarmyhammer-rules crate removal
    let expected_modes = [
        "general-purpose",
        "Explore",
        "Plan",
        "default",
        "planner",
        "implementer",
        "reviewer",
        "tester",
        "committer",
    ];
    assert_eq!(
        modes.len(),
        expected_modes.len(),
        "Should load {} builtin modes",
        expected_modes.len()
    );

    // Verify original embedded modes are present
    assert!(
        registry.get("general-purpose").is_some(),
        "Should have general-purpose"
    );
    assert!(registry.get("Explore").is_some(), "Should have Explore");
    assert!(registry.get("Plan").is_some(), "Should have Plan");

    // Verify new prompt-referencing modes are present
    assert!(registry.get("default").is_some(), "Should have default");
    assert!(registry.get("planner").is_some(), "Should have planner");
    assert!(
        registry.get("implementer").is_some(),
        "Should have implementer"
    );
    assert!(registry.get("reviewer").is_some(), "Should have reviewer");
    assert!(registry.get("tester").is_some(), "Should have tester");
    assert!(registry.get("committer").is_some(), "Should have committer");
    // Note: rule-checker mode removed as part of swissarmyhammer-rules crate removal

    // Verify mode content for embedded mode
    let explore_mode = registry.get("Explore").unwrap();
    assert_eq!(explore_mode.name(), "Explore");
    assert!(explore_mode.description().contains("codebase"));
    assert!(explore_mode.system_prompt().contains("exploration"));

    // Verify agent-referencing mode
    let planner_mode = registry.get("planner").unwrap();
    assert_eq!(planner_mode.name(), "Planner");
    assert!(planner_mode.uses_agent_reference());
    assert_eq!(planner_mode.agent(), Some("planner"));
}

#[test]
fn test_user_mode_overrides_builtin() {
    // Test that the override mechanism works by adding modes directly
    // VirtualFileSystem loads from actual Git repo, so we test the override
    // behavior through the registry's add() method which simulates precedence

    let mut registry = ModeRegistry::new();

    // Load builtin modes first
    let _modes = registry.load_all().unwrap();

    // Verify builtin Explore is loaded
    let builtin_explore = registry.get("Explore").unwrap();
    assert_eq!(builtin_explore.name(), "Explore");

    // Now add a "user" mode that overrides it
    let user_explore = swissarmyhammer_modes::Mode::new(
        "Explore",
        "Explore (User Override)",
        "User customized Explore mode",
        "This is a user-customized system prompt for Explore mode.",
    );
    registry.add(user_explore);

    // User version should now override builtin
    let explore_mode = registry.get("Explore").unwrap();
    assert_eq!(explore_mode.name(), "Explore (User Override)");
    assert_eq!(explore_mode.description(), "User customized Explore mode");
    assert!(explore_mode.system_prompt().contains("user-customized"));

    // Other builtin modes should still be present
    assert!(registry.get("general-purpose").is_some());
    assert!(registry.get("Plan").is_some());
}

#[test]
fn test_project_mode_overrides_all() {
    // Test that the override precedence works: project > user > builtin
    // We simulate this by adding modes in order and verifying the last one wins

    let mut registry = ModeRegistry::new();

    // Load builtin modes first
    let _modes = registry.load_all().unwrap();

    // Verify builtin Plan exists
    let builtin_plan = registry.get("Plan").unwrap();
    assert_eq!(builtin_plan.name(), "Plan");

    // Add a "user" override
    let user_plan = swissarmyhammer_modes::Mode::new(
        "Plan",
        "Plan (User)",
        "User Plan mode",
        "User Plan prompt",
    );
    registry.add(user_plan);

    // Verify user override is active
    assert_eq!(registry.get("Plan").unwrap().name(), "Plan (User)");

    // Add a "project" override (highest precedence)
    let project_plan = swissarmyhammer_modes::Mode::new(
        "Plan",
        "Plan (Project)",
        "Project-specific Plan mode",
        "Project Plan prompt for this specific codebase.",
    );
    registry.add(project_plan);

    // Project version should override both builtin and user
    let plan_mode = registry.get("Plan").unwrap();
    assert_eq!(plan_mode.name(), "Plan (Project)");
    assert_eq!(plan_mode.description(), "Project-specific Plan mode");
    assert!(plan_mode.system_prompt().contains("Project Plan prompt"));
    assert!(plan_mode.system_prompt().contains("specific codebase"));
}

#[test]
fn test_mode_file_format_validation() {
    // This test verifies mode file format requirements
    // Since we use VirtualFileSystem which loads from actual Git repo,
    // we test format validation through Mode::from_markdown directly

    // Valid mode
    let valid_content = r#"---
name: Valid Mode
description: A properly formatted mode
---
This is the system prompt.
It can have multiple paragraphs.
"#;

    let valid_mode = swissarmyhammer_modes::Mode::from_markdown(valid_content, "valid").unwrap();
    assert_eq!(valid_mode.name(), "Valid Mode");
    assert_eq!(valid_mode.description(), "A properly formatted mode");
    assert!(valid_mode.system_prompt().contains("multiple paragraphs"));

    // Invalid mode - missing description
    let invalid_content = r#"---
name: Invalid Mode
---
System prompt
"#;

    let result = swissarmyhammer_modes::Mode::from_markdown(invalid_content, "invalid");
    assert!(result.is_err(), "Mode without description should fail");

    // Invalid mode - missing name
    let invalid_content2 = r#"---
description: Has description but no name
---
System prompt
"#;

    let result2 = swissarmyhammer_modes::Mode::from_markdown(invalid_content2, "invalid2");
    assert!(result2.is_err(), "Mode without name should fail");
}
