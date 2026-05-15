use std::fs;
use swissarmyhammer_common::test_utils::CurrentDirGuard;
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

#[serial_test::serial(cwd)]
#[test]
#[serial_test::serial(cwd)]
fn test_rust_project_detection_populates_context() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir(root.join(".git")).unwrap();

    let project_path = root.join("isolated_project");
    fs::create_dir(&project_path).unwrap();
    fs::write(
        project_path.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/main.rs"), "fn main() {}").unwrap();

    let _guard = CurrentDirGuard::new(&project_path).unwrap();

    let mut context = TemplateContext::new();
    context.set_default_variables();

    let project_types = context
        .get("project_types")
        .expect("project_types should be set");
    assert!(project_types.is_array(), "project_types should be an array");
    let projects = project_types.as_array().unwrap();
    assert!(!projects.is_empty(), "Should detect at least one project");

    let unique_types = context
        .get("unique_project_types")
        .expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(!unique.is_empty());

    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    assert!(has_rust, "Rust should be in unique project types");
}

#[serial_test::serial(cwd)]
#[test]
#[serial_test::serial(cwd)]
fn test_nodejs_project_detection_populates_context() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir(root.join(".git")).unwrap();

    let project_path = root.join("isolated_nodejs_project");
    fs::create_dir(&project_path).unwrap();
    fs::write(
        project_path.join("package.json"),
        r#"{"name": "test", "version": "1.0.0"}"#,
    )
    .unwrap();
    fs::create_dir(project_path.join("node_modules")).unwrap();
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/index.js"), "console.log('test');").unwrap();

    let _guard = CurrentDirGuard::new(&project_path).unwrap();

    let mut context = TemplateContext::new();
    context.set_default_variables();

    let unique_types = context
        .get("unique_project_types")
        .expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(!unique.is_empty());

    let has_nodejs = unique.iter().any(|t| t.as_str() == Some("NodeJs"));
    assert!(
        has_nodejs,
        "Should detect NodeJs project type, found: {:?}",
        unique
    );
}

#[serial_test::serial(cwd)]
#[test]
fn test_multiple_rust_projects_detected() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join(".git")).unwrap();

    let root_path = temp_dir.path().join("monorepo");
    fs::create_dir(&root_path).unwrap();

    let project1 = root_path.join("project1");
    fs::create_dir(&project1).unwrap();
    fs::write(
        project1.join("Cargo.toml"),
        "[package]\nname = \"project1\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir(project1.join("src")).unwrap();
    fs::write(project1.join("src/lib.rs"), "").unwrap();

    let project2 = root_path.join("project2");
    fs::create_dir(&project2).unwrap();
    fs::write(
        project2.join("Cargo.toml"),
        "[package]\nname = \"project2\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir(project2.join("src")).unwrap();
    fs::write(project2.join("src/lib.rs"), "").unwrap();

    let _guard = CurrentDirGuard::new(&root_path).unwrap();

    let mut context = TemplateContext::new();
    context.set_default_variables();

    let project_types = context
        .get("project_types")
        .expect("project_types should be set");
    let projects = project_types.as_array().unwrap();
    assert!(projects.len() >= 2, "Should detect at least two projects");

    let unique_types = context
        .get("unique_project_types")
        .expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    assert!(has_rust, "Rust should be in unique project types");
}

#[serial_test::serial(cwd)]
#[test]
fn test_mixed_projects_detected() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join(".git")).unwrap();

    let root_path = temp_dir.path().join("monorepo");
    fs::create_dir(&root_path).unwrap();

    let rust_project = root_path.join("backend");
    fs::create_dir(&rust_project).unwrap();
    fs::write(
        rust_project.join("Cargo.toml"),
        "[package]\nname = \"backend\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir(rust_project.join("src")).unwrap();
    fs::write(rust_project.join("src/lib.rs"), "").unwrap();

    let node_project = root_path.join("frontend");
    fs::create_dir(&node_project).unwrap();
    fs::write(node_project.join("package.json"), r#"{"name": "frontend"}"#).unwrap();
    fs::create_dir(node_project.join("src")).unwrap();
    fs::write(node_project.join("src/index.js"), "console.log('test');").unwrap();

    let _guard = CurrentDirGuard::new(&root_path).unwrap();

    let mut context = TemplateContext::new();
    context.set_default_variables();

    let project_types = context
        .get("project_types")
        .expect("project_types should be set");
    let projects = project_types.as_array().unwrap();
    assert!(projects.len() >= 2, "Should detect at least two projects");

    let unique_types = context
        .get("unique_project_types")
        .expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(unique.len() >= 2);

    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    let has_nodejs = unique.iter().any(|t| t.as_str() == Some("NodeJs"));
    assert!(has_rust, "Rust should be in unique project types");
    assert!(has_nodejs, "NodeJs should be in unique project types");
}
