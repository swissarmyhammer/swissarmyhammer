//! Tests for rendering built-in modes without errors
//!
//! This test suite ensures that all built-in mode files can be rendered
//! successfully with a properly configured template context.

use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use std::sync::Arc;

/// Test that all built-in mode files can be loaded and rendered without errors
#[tokio::test]
async fn test_all_builtin_modes_can_render() {
    // Get all built-in mode files
    let modes_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/modes");

    if !modes_dir.exists() {
        eprintln!("Modes directory not found: {:?}", modes_dir);
        return;
    }

    let mode_files: Vec<_> = std::fs::read_dir(&modes_dir)
        .expect("Failed to read modes directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();

    assert!(!mode_files.is_empty(), "No mode files found in {:?}", modes_dir);

    // Load all prompts (including system prompts that modes reference)
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)
        .expect("Failed to load prompts");
    let library_arc = Arc::new(library);

    // Create a template context with default variables (including project_types)
    let mut template_context = TemplateContext::load()
        .expect("Failed to load template context");

    // Ensure default variables are set (including project_types detection)
    template_context.set_default_variables();

    // Debug: Check what variables are in the context
    eprintln!("Template context variables: {:?}", template_context.variables().keys().collect::<Vec<_>>());

    // Debug: Check project_types specifically
    if let Some(pt) = template_context.get("project_types") {
        eprintln!("project_types value: {:?}", pt);
    } else {
        eprintln!("WARNING: project_types is NOT set in template context!");
    }

    // Verify project_types is set
    assert!(
        template_context.get("project_types").is_some(),
        "project_types should be set in template context"
    );

    // Test each mode file
    for mode_file in mode_files {
        let mode_name = mode_file.file_name().to_string_lossy().to_string();
        eprintln!("Testing mode: {}", mode_name);

        // Read the mode file to get the prompt reference
        let mode_path = mode_file.path();
        let mode_content = std::fs::read_to_string(&mode_path)
            .expect(&format!("Failed to read mode file: {:?}", mode_path));

        // Parse frontmatter to get the prompt name
        if let Some(prompt_name) = extract_prompt_from_mode(&mode_content) {
            eprintln!("  -> Rendering prompt: {}", prompt_name);

            // Try to render the prompt
            let render_result = library_arc.render(&prompt_name, &template_context);

            match render_result {
                Ok(rendered) => {
                    assert!(!rendered.is_empty(), "Rendered prompt should not be empty for mode: {}", mode_name);
                    eprintln!("  ✓ Successfully rendered {} ({}  bytes)", prompt_name, rendered.len());
                }
                Err(e) => {
                    panic!("Failed to render prompt '{}' for mode '{}': {}", prompt_name, mode_name, e);
                }
            }
        } else {
            eprintln!("  ⚠ No prompt reference found in mode file");
        }
    }
}

/// Extract the prompt name from a mode file's frontmatter
fn extract_prompt_from_mode(content: &str) -> Option<String> {
    // Simple YAML frontmatter parser
    if !content.starts_with("---") {
        return None;
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }

    let frontmatter = parts[1];

    // Find the "prompt:" line
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.starts_with("prompt:") {
            let prompt_name = line
                .trim_start_matches("prompt:")
                .trim()
                .to_string();
            return Some(prompt_name);
        }
    }

    None
}

/// Test that template context includes all required variables for mode rendering
#[tokio::test]
async fn test_template_context_has_required_variables() {
    let mut context = TemplateContext::load()
        .expect("Failed to load template context");

    context.set_default_variables();

    // Check for required variables
    assert!(context.get("model").is_some(), "model variable should be set");
    assert!(context.get("working_directory").is_some(), "working_directory should be set");
    assert!(context.get("cwd").is_some(), "cwd should be set");
    assert!(context.get("project_types").is_some(), "project_types should be set");

    // project_types should be an array
    let project_types = context.get("project_types").unwrap();
    assert!(
        project_types.is_array(),
        "project_types should be an array, got: {:?}",
        project_types
    );
}

/// Test that detected-projects partial can be rendered with empty project_types
#[tokio::test]
async fn test_detected_projects_partial_with_empty_projects() {
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)
        .expect("Failed to load prompts");
    let library_arc = Arc::new(library);

    let mut context = TemplateContext::new();
    context.set("project_types".to_string(), serde_json::json!([]));

    // Should render without error even with empty project_types
    let result = library_arc.render("_partials/detected-projects", &context);

    assert!(result.is_ok(), "Should render with empty project_types: {:?}", result.err());
    let rendered = result.unwrap();
    assert!(rendered.contains("No Projects Detected"), "Should show 'No Projects Detected' message");
}

/// Test that detected-projects partial can be rendered with sample project data
#[tokio::test]
async fn test_detected_projects_partial_with_projects() {
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)
        .expect("Failed to load prompts");
    let library_arc = Arc::new(library);

    let mut context = TemplateContext::new();
    context.set("project_types".to_string(), serde_json::json!([
        {
            "type": "Rust",
            "path": "/path/to/project",
            "markers": ["Cargo.toml"],
            "workspace": null,
            "commands": {
                "build": "cargo build",
                "test": "cargo nextest run --workspace",
                "check": "cargo check",
                "format": "cargo fmt"
            }
        }
    ]));
    // The template also requires unique_project_types (derived from project_types)
    context.set("unique_project_types".to_string(), serde_json::json!(["Rust"]));

    // Should render without error with project data
    let result = library_arc.render("_partials/detected-projects", &context);

    assert!(result.is_ok(), "Should render with project data: {:?}", result.err());
    let rendered = result.unwrap();
    assert!(rendered.contains("Detected Project Types"), "Should show 'Detected Project Types' header");
    assert!(rendered.contains("Rust"), "Should mention Rust project type");
}
