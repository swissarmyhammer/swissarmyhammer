use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptPartialAdapter};
use swissarmyhammer_templating::Template;

fn get_builtin_prompts_path() -> PathBuf {
    // Tests run from the workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/prompts")
}

fn get_builtin_partials_path() -> PathBuf {
    // Shared partials live at builtin/_partials/ (outside prompts)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/_partials")
}

/// Load shared partials from builtin/_partials/ into the library with _partials/ prefix
fn load_shared_partials(library: &mut PromptLibrary) {
    let partials_path = get_builtin_partials_path();
    for entry in walkdir::WalkDir::new(&partials_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md" || e == "liquid") {
            if let Ok(content) = std::fs::read_to_string(path) {
                // Strip YAML frontmatter so it doesn't bleed into rendered output
                let template_content = strip_frontmatter(&content);
                let relative = path.strip_prefix(&partials_path).unwrap();
                let name = relative.with_extension("").to_string_lossy().to_string();
                let prefixed = format!("_partials/{}", name);
                let _ = library.add(Prompt::new(&prefixed, &template_content));
            }
        }
    }
}

/// Strip YAML frontmatter (---\n...\n---\n) from content
fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            return content[3 + end + 4..].to_string();
        }
    }
    content.to_string()
}

#[test]
fn test_detected_projects_includes_rust_instructions() {
    // Create a template context with a Rust project detected
    let mut context = TemplateContext::new();
    context.set(
        "project_types".to_string(),
        json!([{
            "type": "Rust",
            "path": "/test/project",
            "markers": ["Cargo.toml"],
            "workspace": null
        }]),
    );
    // Also set unique_project_types which is required by the template
    context.set("unique_project_types".to_string(), json!(["Rust"]));

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the prompt directly
    let prompt = library
        .get("_partials/detected-projects")
        .expect("Failed to get detected-projects prompt");

    // Create partial adapter for rendering
    let adapter = PromptPartialAdapter::new(Arc::new(library));

    // Create template with partial support
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered detected-projects partial ===");
    println!("{}", rendered);
    println!("=== End of rendered output ===");

    // Verify the output contains Rust-specific content
    assert!(
        rendered.contains("Rust Project"),
        "Should contain 'Rust Project' header"
    );
    assert!(
        rendered.contains("cargo nextest"),
        "Should contain Rust-specific testing instructions mentioning cargo nextest"
    );
    assert!(
        rendered.contains("Cargo.toml"),
        "Should contain marker file name"
    );
}

#[test]
fn test_detected_projects_includes_nodejs_instructions() {
    // Create a template context with a Node.js project detected
    let mut context = TemplateContext::new();
    context.set(
        "project_types".to_string(),
        json!([{
            "type": "NodeJs",
            "path": "/test/project",
            "markers": ["package.json"],
            "workspace": null
        }]),
    );
    // Also set unique_project_types which is required by the template
    context.set("unique_project_types".to_string(), json!(["NodeJs"]));

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the prompt directly
    let prompt = library
        .get("_partials/detected-projects")
        .expect("Failed to get detected-projects prompt");

    // Create partial adapter for rendering
    let adapter = PromptPartialAdapter::new(Arc::new(library));

    // Create template with partial support
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered detected-projects partial (NodeJs) ===");
    println!("{}", rendered);
    println!("=== End of rendered output ===");

    // Verify the output contains Node.js-specific content
    eprintln!("Looking for NodeJs in output...");
    eprintln!(
        "Contains 'NodeJs Project': {}",
        rendered.contains("NodeJs Project")
    );
    eprintln!("Contains 'Node.js': {}", rendered.contains("Node.js"));

    assert!(
        rendered.contains("NodeJs Project") || rendered.contains("Node.js"),
        "Should contain Node.js project header. Rendered output:\n{}",
        rendered
    );
    assert!(
        rendered.contains("npm test") || rendered.contains("yarn test"),
        "Should contain Node.js-specific testing instructions"
    );
    assert!(
        rendered.contains("package.json"),
        "Should contain marker file name"
    );
}

#[test]
fn test_detected_projects_includes_flutter_instructions() {
    // Create a template context with a Flutter project detected
    let mut context = TemplateContext::new();
    context.set(
        "project_types".to_string(),
        json!([{
            "type": "Flutter",
            "path": "/test/flutter-project",
            "markers": ["pubspec.yaml"],
            "workspace": null
        }]),
    );
    // Also set unique_project_types which is required by the template
    context.set("unique_project_types".to_string(), json!(["Flutter"]));

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the prompt directly
    let prompt = library
        .get("_partials/detected-projects")
        .expect("Failed to get detected-projects prompt");

    // Create partial adapter for rendering
    let adapter = PromptPartialAdapter::new(Arc::new(library));

    // Create template with partial support
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered detected-projects partial (Flutter) ===");
    println!("{}", rendered);
    println!("=== End of rendered output ===");

    // Verify the output contains Flutter-specific content
    assert!(
        rendered.contains("Flutter Project"),
        "Should contain 'Flutter Project' header"
    );
    assert!(
        rendered.contains("flutter test") || rendered.contains("fvm flutter test"),
        "Should contain Flutter-specific testing instructions"
    );
    assert!(
        rendered.contains("pubspec.yaml"),
        "Should contain marker file name"
    );
}

#[test]
fn test_detected_projects_deduplicates_instructions() {
    // Create a template context with multiple Rust projects
    let mut context = TemplateContext::new();
    context.set(
        "project_types".to_string(),
        json!([
            {
                "type": "Rust",
                "path": "/test/project1",
                "markers": ["Cargo.toml"],
                "workspace": null
            },
            {
                "type": "Rust",
                "path": "/test/project2",
                "markers": ["Cargo.toml"],
                "workspace": null
            },
            {
                "type": "NodeJs",
                "path": "/test/frontend",
                "markers": ["package.json"],
                "workspace": null
            }
        ]),
    );
    // Also set unique_project_types which is required by the template (deduplicated)
    context.set(
        "unique_project_types".to_string(),
        json!(["Rust", "NodeJs"]),
    );

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the prompt directly
    let prompt = library
        .get("_partials/detected-projects")
        .expect("Failed to get detected-projects prompt");

    // Create partial adapter for rendering
    let adapter = PromptPartialAdapter::new(Arc::new(library));

    // Create template with partial support
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered detected-projects partial (Multiple Projects) ===");
    println!("{}", rendered);
    println!("=== End of rendered output ===");

    // Verify that both project locations are listed
    assert!(
        rendered.contains("/test/project1"),
        "Should list first Rust project"
    );
    assert!(
        rendered.contains("/test/project2"),
        "Should list second Rust project"
    );
    assert!(
        rendered.contains("/test/frontend"),
        "Should list Node.js project"
    );

    // Count occurrences of the Rust guidelines header
    let rust_guideline_count = rendered.matches("Rust Project Guidelines").count();
    assert_eq!(
        rust_guideline_count, 1,
        "Rust guidelines should appear exactly once, not {} times",
        rust_guideline_count
    );

    // Count occurrences of cargo nextest (Rust-specific)
    let nextest_count = rendered.matches("cargo nextest").count();
    // Should appear multiple times in the Rust guidelines section, but the section itself should only appear once
    assert!(
        nextest_count >= 1,
        "Should contain cargo nextest instructions"
    );
}
