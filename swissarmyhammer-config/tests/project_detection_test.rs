use std::env;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_project_detection::detect_projects;

#[test]
fn test_project_detection_in_swissarmyhammer_repo() {
    // This test runs in the swissarmyhammer repository, which is a Rust project
    let cwd = env::current_dir().expect("Failed to get current directory");

    // Find the workspace root (where the root Cargo.toml is)
    let workspace_root = cwd
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("swissarmyhammer-config").exists())
        .expect("Could not find workspace root");

    println!("Testing project detection in: {:?}", workspace_root);

    // Test direct project detection
    let projects = detect_projects(workspace_root, Some(3)).expect("Project detection failed");

    assert!(!projects.is_empty(), "Should detect at least one project");

    // Should detect the root Rust workspace
    let has_rust = projects.iter().any(|p| {
        matches!(
            p.project_type,
            swissarmyhammer_project_detection::ProjectType::Rust
        )
    });
    assert!(has_rust, "Should detect Rust project in workspace root");

    println!("✅ Found {} projects", projects.len());
    for project in &projects {
        println!(
            "  - {:?} at {}",
            project.project_type,
            project.path.display()
        );
    }
}

#[test]
fn test_template_context_has_project_types() {
    // Test that TemplateContext properly sets project_types variable
    let mut ctx = TemplateContext::new();
    ctx.set_default_variables();

    let project_types = ctx
        .get("project_types")
        .expect("project_types variable should be set in JS context");

    println!("✅ project_types variable is set");
    println!("Value: {:#?}", project_types);

    // Should be an array
    assert!(project_types.is_array(), "project_types should be an array");

    let array = project_types.as_array().unwrap();
    assert!(!array.is_empty(), "project_types should not be empty (should detect at least the swissarmyhammer Rust workspace)");
}

#[test]
fn test_project_types_structure() {
    // Test that each project in project_types has the expected structure
    let mut ctx = TemplateContext::new();
    ctx.set_default_variables();

    let project_types = ctx
        .get("project_types")
        .expect("project_types should be set")
        .as_array()
        .expect("project_types should be an array")
        .clone();

    for (i, project) in project_types.iter().enumerate() {
        let obj = project
            .as_object()
            .unwrap_or_else(|| panic!("Project {} should be an object", i));

        // Check required fields
        assert!(
            obj.contains_key("type"),
            "Project {} should have 'type' field",
            i
        );
        assert!(
            obj.contains_key("path"),
            "Project {} should have 'path' field",
            i
        );
        assert!(
            obj.contains_key("markers"),
            "Project {} should have 'markers' field",
            i
        );

        // Markers should be an array of strings
        let markers = obj
            .get("markers")
            .unwrap()
            .as_array()
            .expect("markers should be an array");
        assert!(
            !markers.is_empty(),
            "Project {} should have at least one marker",
            i
        );

        println!(
            "✅ Project {}: type={}, path={}, markers={}",
            i,
            obj.get("type").unwrap(),
            obj.get("path").unwrap(),
            markers.len()
        );
    }
}
