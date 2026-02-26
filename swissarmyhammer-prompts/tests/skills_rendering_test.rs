//! Tests that the skills partial renders correctly in system prompt templates
//!
//! Verifies that when available_skills are set in the TemplateContext,
//! the skills section appears in rendered system prompts with skill metadata.

use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptPartialAdapter};
use swissarmyhammer_templating::Template;

fn get_builtin_prompts_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/prompts")
}

fn get_builtin_partials_path() -> PathBuf {
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
                let template_content = strip_frontmatter(&content);
                let relative = path.strip_prefix(&partials_path).unwrap();
                let name = relative.with_extension("").to_string_lossy().to_string();
                let prefixed = format!("_partials/{}", name);
                let _ = library.add(Prompt::new(&prefixed, &template_content));
            }
        }
    }
}

fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            return content[3 + end + 4..].to_string();
        }
    }
    content.to_string()
}

#[test]
fn test_skills_partial_renders_with_available_skills() {
    // Create a template context with available skills
    let mut context = TemplateContext::new();
    context.set(
        "available_skills".to_string(),
        json!([
            {"name": "plan", "description": "Create implementation plans", "source": "builtin"},
            {"name": "commit", "description": "Create git commits", "source": "builtin"},
            {"name": "test", "description": "Write and run tests", "source": "builtin"},
        ]),
    );

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the skills partial
    let prompt = library
        .get("_partials/skills")
        .expect("Failed to get skills partial");

    // Create partial adapter for rendering
    let adapter = PromptPartialAdapter::new(Arc::new(library));

    // Create template with partial support
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered skills partial ===");
    println!("{}", rendered);
    println!("=== End ===");

    // Verify the output contains the skills section
    assert!(
        rendered.contains("## Skills"),
        "Should contain Skills header"
    );
    assert!(rendered.contains("**plan**"), "Should list plan skill");
    assert!(rendered.contains("**commit**"), "Should list commit skill");
    assert!(rendered.contains("**test**"), "Should list test skill");
    assert!(
        rendered.contains("use skill"),
        "Should contain use skill instruction"
    );
    assert!(
        rendered.contains("search skill"),
        "Should contain search skill instruction"
    );
}

#[test]
fn test_skills_partial_hidden_when_no_skills() {
    // Create a template context with empty available_skills
    let mut context = TemplateContext::new();
    context.set("available_skills".to_string(), json!([]));

    // Load the prompt library with builtin prompts
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    // Get the skills partial
    let prompt = library
        .get("_partials/skills")
        .expect("Failed to get skills partial");

    let adapter = PromptPartialAdapter::new(Arc::new(library));
    let template =
        Template::with_partials(&prompt.template, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    // Should not contain the skills section when empty
    assert!(
        !rendered.contains("## Skills"),
        "Should NOT contain Skills header when no skills available"
    );
}

#[test]
fn test_default_system_prompt_includes_skills_section() {
    // Create a template context with available skills (simulating set_default_variables)
    let mut context = TemplateContext::new();
    context.set(
        "available_skills".to_string(),
        json!([
            {"name": "plan", "description": "Create implementation plans", "source": "builtin"},
            {"name": "commit", "description": "Create git commits", "source": "builtin"},
        ]),
    );
    // Set other required variables
    context.set("project_types".to_string(), json!([]));
    context.set("unique_project_types".to_string(), json!([]));

    // Load the default agent's instructions (system prompt content moved from .system/default)
    let agent_md_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/agents/default/AGENT.md");
    let agent_content =
        std::fs::read_to_string(&agent_md_path).expect("Failed to read default agent AGENT.md");
    // Extract body after frontmatter
    let instructions = agent_content
        .strip_prefix("---")
        .and_then(|s| s.find("---").map(|i| &s[i + 3..]))
        .unwrap_or(&agent_content)
        .trim();

    // Load partials for template rendering
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    let adapter = PromptPartialAdapter::new(Arc::new(library));
    let template =
        Template::with_partials(instructions, adapter).expect("Failed to create template");

    let rendered = template
        .render_with_context(&context)
        .expect("Failed to render template");

    println!("=== Rendered default agent instructions ===");
    println!("{}", rendered);
    println!("=== End ===");

    // The default agent instructions should include the skills section
    assert!(
        rendered.contains("## Skills"),
        "Default agent instructions should contain Skills section"
    );
    assert!(
        rendered.contains("**plan**"),
        "Default agent instructions should list plan skill"
    );
    assert!(
        rendered.contains("**commit**"),
        "Default agent instructions should list commit skill"
    );
}
