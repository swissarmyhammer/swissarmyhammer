use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

fn get_builtin_prompts_path() -> PathBuf {
    // Get the workspace root
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    workspace_root.join("builtin/prompts")
}

#[test]
fn test_rust_project_detection_renders_guidelines() {
    // Store original directory first
    let original_dir = std::env::current_dir().unwrap();
    
    // Create a temporary directory OUTSIDE of any git repo
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    
    // Create a .git directory to act as a boundary and prevent walking up to parent projects
    fs::create_dir(root.join(".git")).unwrap();
    
    // Create a subdirectory for the project to avoid picking up parent directories
    let project_path = root.join("isolated_project");
    fs::create_dir(&project_path).unwrap();
    
    // Create Cargo.toml to trigger Rust project detection
    fs::write(project_path.join("Cargo.toml"), "[package]\nname = \"test\"\nversion = \"0.1.0\"").unwrap();
    
    // Create src directory to make it look more like a real project
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/main.rs"), "fn main() {}").unwrap();
    
    // Change to the project directory
    std::env::set_current_dir(&project_path).unwrap();
    
    // Create template context with project detection
    let mut context = TemplateContext::new();
    context.set_default_variables();
    
    // Verify project was detected
    let project_types = context.get("project_types").expect("project_types should be set");
    assert!(project_types.is_array(), "project_types should be an array");
    let projects = project_types.as_array().unwrap();
    assert!(!projects.is_empty(), "Should detect at least one project");
    
    // Verify unique_project_types was set and contains Rust
    let unique_types = context.get("unique_project_types").expect("unique_project_types should be set");
    assert!(unique_types.is_array(), "unique_project_types should be an array");
    let unique = unique_types.as_array().unwrap();
    assert!(!unique.is_empty(), "Should have at least one unique project type");
    
    // Check that Rust is in the unique types
    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    assert!(has_rust, "Rust should be in unique project types");
    
    // Load prompts and render detected-projects partial
    let mut library = PromptLibrary::new();
    library.add_directory(get_builtin_prompts_path()).expect("Failed to load prompts");
    
    let rendered = library.render("_partials/detected-projects", &context)
        .expect("Failed to render detected-projects");
    
    println!("=== Rendered Output ===");
    println!("{}", rendered);
    println!("=== End Output ===");
    
    // Verify the rendered output contains Rust-specific content
    assert!(rendered.contains("Rust Project"), "Should contain 'Rust Project' header");
    assert!(rendered.contains("cargo nextest"), "Should contain Rust-specific cargo nextest instructions");
    assert!(rendered.contains("Cargo.toml"), "Should contain Cargo.toml marker");
    assert!(rendered.contains("Project Guidelines"), "Should contain Project Guidelines section");
    
    // Restore original directory BEFORE TempDir is dropped
    // Use let _ to ignore the result since the temp dir might already be cleaning up
    let _ = std::env::set_current_dir(&original_dir);
    // Now temp_dir will be dropped and cleaned up safely
}

#[test]
fn test_nodejs_project_detection_renders_guidelines() {
    // Store original directory first
    let original_dir = std::env::current_dir().unwrap();
    
    // Create a temporary directory OUTSIDE of any git repo
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    
    // Create a .git directory to act as a boundary and prevent walking up to parent projects
    fs::create_dir(root.join(".git")).unwrap();
    
    // Create a subdirectory for the project
    let project_path = root.join("isolated_nodejs_project");
    fs::create_dir(&project_path).unwrap();
    
    // Create package.json to trigger Node.js project detection
    fs::write(project_path.join("package.json"), r#"{"name": "test", "version": "1.0.0"}"#).unwrap();
    
    // Create node_modules and src to make it look like a real project
    fs::create_dir(project_path.join("node_modules")).unwrap();
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/index.js"), "console.log('test');").unwrap();
    
    // Change to the project directory
    std::env::set_current_dir(&project_path).unwrap();
    
    // Create template context with project detection
    let mut context = TemplateContext::new();
    context.set_default_variables();
    
    // Verify unique_project_types was set
    let unique_types = context.get("unique_project_types").expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(!unique.is_empty(), "Should have at least one unique project type");
    
    // Verify it's actually detecting NodeJs
    let has_nodejs = unique.iter().any(|t| t.as_str() == Some("NodeJs"));
    assert!(has_nodejs, "Should detect NodeJs project type, found: {:?}", unique);
    
    // Load prompts and render
    let mut library = PromptLibrary::new();
    let prompts_path = get_builtin_prompts_path();
    println!("Loading prompts from: {:?}", prompts_path);
    println!("Path exists: {}", prompts_path.exists());
    library.add_directory(&prompts_path).expect("Failed to load prompts");
    
    let rendered = library.render("_partials/detected-projects", &context)
        .expect("Failed to render detected-projects");
    
    println!("=== Rendered Output ===");
    println!("{}", rendered);
    println!("=== End Output ===");
    
    // Verify the guidelines section exists and is not empty
    assert!(rendered.contains("Project Guidelines"), "Should contain Project Guidelines section");
    assert!(rendered.contains("Common Commands") || rendered.contains("Testing Strategy"), 
        "Should contain project-specific guidelines");
    
    // Restore original directory BEFORE TempDir is dropped
    // Use let _ to ignore the result since the temp dir might already be cleaning up
    let _ = std::env::set_current_dir(&original_dir);
    // Now temp_dir will be dropped and cleaned up safely
}

#[test]
fn test_multiple_rust_projects_renders_guidelines_once() {
    // Store original directory first
    let original_dir = std::env::current_dir().unwrap();
    
    // Create a temporary directory with multiple Rust projects
    let temp_dir = TempDir::new().unwrap();
    
    // Create a .git directory at temp root to act as a boundary
    fs::create_dir(temp_dir.path().join(".git")).unwrap();
    
    let root_path = temp_dir.path().join("monorepo");
    fs::create_dir(&root_path).unwrap();
    
    // Create first Rust project
    let project1 = root_path.join("project1");
    fs::create_dir(&project1).unwrap();
    fs::write(project1.join("Cargo.toml"), "[package]\nname = \"project1\"\nversion = \"0.1.0\"").unwrap();
    fs::create_dir(project1.join("src")).unwrap();
    fs::write(project1.join("src/lib.rs"), "").unwrap();
    
    // Create second Rust project
    let project2 = root_path.join("project2");
    fs::create_dir(&project2).unwrap();
    fs::write(project2.join("Cargo.toml"), "[package]\nname = \"project2\"\nversion = \"0.1.0\"").unwrap();
    fs::create_dir(project2.join("src")).unwrap();
    fs::write(project2.join("src/lib.rs"), "").unwrap();
    
    // Change to the monorepo directory to detect the projects
    std::env::set_current_dir(&root_path).unwrap();
    
    // Create template context with project detection
    let mut context = TemplateContext::new();
    context.set_default_variables();
    
    // Verify both projects were detected
    let project_types = context.get("project_types").expect("project_types should be set");
    let projects = project_types.as_array().unwrap();
    assert!(projects.len() >= 2, "Should detect at least two projects");
    
    // Verify unique_project_types has Rust
    let unique_types = context.get("unique_project_types").expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(!unique.is_empty(), "Should have at least one unique project type");
    
    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    assert!(has_rust, "Rust should be in unique project types");
    
    // Load prompts and render
    let mut library = PromptLibrary::new();
    library.add_directory(get_builtin_prompts_path()).expect("Failed to load prompts");
    
    let rendered = library.render("_partials/detected-projects", &context)
        .expect("Failed to render detected-projects");
    
    println!("=== Rendered Output ===");
    println!("{}", rendered);
    println!("=== End Output ===");
    
    // Verify both project locations are listed
    assert!(rendered.contains("project1"), "Should list project1");
    assert!(rendered.contains("project2"), "Should list project2");
    
    // Count occurrences of the Rust guidelines header - should appear exactly once
    let rust_guideline_count = rendered.matches("Rust Project Guidelines").count();
    assert_eq!(rust_guideline_count, 1, 
        "Rust guidelines should appear exactly once, not {} times", rust_guideline_count);
    
    // Restore original directory BEFORE TempDir is dropped
    // Use let _ to ignore the result since the temp dir might already be cleaning up
    let _ = std::env::set_current_dir(&original_dir);
    // Now temp_dir will be dropped and cleaned up safely
}

#[test]
fn test_mixed_projects_renders_multiple_guidelines() {
    // Store original directory first
    let original_dir = std::env::current_dir().unwrap();
    
    // Create a temporary directory with mixed project types
    let temp_dir = TempDir::new().unwrap();
    
    // Create a .git directory at temp root to act as a boundary
    fs::create_dir(temp_dir.path().join(".git")).unwrap();
    
    let root_path = temp_dir.path().join("monorepo");
    fs::create_dir(&root_path).unwrap();
    
    // Create Rust project
    let rust_project = root_path.join("backend");
    fs::create_dir(&rust_project).unwrap();
    fs::write(rust_project.join("Cargo.toml"), "[package]\nname = \"backend\"\nversion = \"0.1.0\"").unwrap();
    fs::create_dir(rust_project.join("src")).unwrap();
    fs::write(rust_project.join("src/lib.rs"), "").unwrap();
    
    // Create Node.js project
    let node_project = root_path.join("frontend");
    fs::create_dir(&node_project).unwrap();
    fs::write(node_project.join("package.json"), r#"{"name": "frontend"}"#).unwrap();
    fs::create_dir(node_project.join("src")).unwrap();
    fs::write(node_project.join("src/index.js"), "console.log('test');").unwrap();
    
    // Change to the root directory
    std::env::set_current_dir(&root_path).unwrap();
    
    // Create template context with project detection
    let mut context = TemplateContext::new();
    context.set_default_variables();
    
    // Verify both project types were detected
    let project_types = context.get("project_types").expect("project_types should be set");
    let projects = project_types.as_array().unwrap();
    assert!(projects.len() >= 2, "Should detect at least two projects");
    
    // Verify unique_project_types has both Rust and NodeJs
    let unique_types = context.get("unique_project_types").expect("unique_project_types should be set");
    let unique = unique_types.as_array().unwrap();
    assert!(unique.len() >= 2, "Should have at least two unique project types");
    
    let has_rust = unique.iter().any(|t| t.as_str() == Some("Rust"));
    let has_nodejs = unique.iter().any(|t| t.as_str() == Some("NodeJs"));
    assert!(has_rust, "Rust should be in unique project types");
    assert!(has_nodejs, "NodeJs should be in unique project types");
    
    // Load prompts and render
    let mut library = PromptLibrary::new();
    library.add_directory(get_builtin_prompts_path()).expect("Failed to load prompts");
    
    let rendered = library.render("_partials/detected-projects", &context)
        .expect("Failed to render detected-projects");
    
    println!("=== Rendered Output ===");
    println!("{}", rendered);
    println!("=== End Output ===");
    
    // Verify both project locations are listed
    assert!(rendered.contains("backend"), "Should list backend");
    assert!(rendered.contains("frontend"), "Should list frontend");
    
    // Verify both sets of guidelines appear exactly once each
    let rust_count = rendered.matches("Rust Project Guidelines").count();
    assert_eq!(rust_count, 1, "Rust guidelines should appear exactly once");
    
    let nodejs_count = rendered.matches("Node.js Project Guidelines").count();
    assert_eq!(nodejs_count, 1, "Node.js guidelines should appear exactly once");
    
    // Restore original directory BEFORE TempDir is dropped
    // Use let _ to ignore the result since the temp dir might already be cleaning up
    let _ = std::env::set_current_dir(&original_dir);
    // Now temp_dir will be dropped and cleaned up safely
}
