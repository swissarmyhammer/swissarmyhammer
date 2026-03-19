//! Smoke test: every builtin skill renders without template errors
//!
//! Iterates over all `builtin/skills/*/SKILL.md` files, strips frontmatter,
//! renders through the Liquid template engine with partials loaded, and asserts
//! that no Liquid errors or unresolved includes leak into the output.

use markdowndown::frontmatter::strip_frontmatter;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptPartialAdapter};
use swissarmyhammer_templating::Template;

fn get_builtin_skills_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/skills")
}

fn get_builtin_partials_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/_partials")
}

fn get_builtin_prompts_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("builtin/prompts")
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

/// Discover all builtin skill directories containing SKILL.md
fn discover_skills() -> Vec<(String, String)> {
    let skills_path = get_builtin_skills_path();
    let mut skills = Vec::new();

    for entry in std::fs::read_dir(&skills_path).expect("Failed to read builtin/skills") {
        let entry = entry.expect("Failed to read directory entry");
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.exists() {
            let name = entry.file_name().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&skill_md).expect("Failed to read SKILL.md");
            let body = strip_frontmatter(&content);
            skills.push((name, body.to_string()));
        }
    }

    skills.sort_by(|a, b| a.0.cmp(&b.0));
    skills
}

#[test]
fn test_all_builtin_skills_render_without_errors() {
    // Build a prompt library with all partials loaded
    let mut library = PromptLibrary::new();
    library
        .add_directory(get_builtin_prompts_path())
        .expect("Failed to load builtin prompts");
    load_shared_partials(&mut library);

    let library = Arc::new(library);
    let skills = discover_skills();

    assert!(
        !skills.is_empty(),
        "Should discover at least one builtin skill"
    );

    // Template context with variables that skills may reference
    let mut context = TemplateContext::new();
    context.set("version".to_string(), serde_json::json!("0.0.0-test"));
    context.set("arguments".to_string(), serde_json::json!("test arguments"));
    context.set("project_types".to_string(), serde_json::json!([]));
    context.set("unique_project_types".to_string(), serde_json::json!([]));

    let mut passed = 0;
    let mut failed = Vec::new();

    for (name, body) in &skills {
        let adapter = PromptPartialAdapter::new(library.clone());
        match Template::with_partials(body, adapter) {
            Ok(template) => match template.render_with_context(&context) {
                Ok(rendered) => {
                    // Check for Liquid error indicators in rendered output
                    let has_raw_include = rendered.contains("{% include");
                    let has_partial_error = rendered.contains("Unknown partial-template");
                    let has_liquid_error = rendered.contains("liquid:");

                    if has_raw_include || has_partial_error || has_liquid_error {
                        let mut errors = Vec::new();
                        if has_raw_include {
                            errors.push("raw {% include %} tag in output");
                        }
                        if has_partial_error {
                            errors.push("Unknown partial-template error");
                        }
                        if has_liquid_error {
                            errors.push("liquid: error in output");
                        }
                        failed.push(format!("{}: {}", name, errors.join(", ")));
                    } else {
                        passed += 1;
                        println!("  ok  {}", name);
                    }
                }
                Err(e) => {
                    failed.push(format!("{}: render error: {}", name, e));
                }
            },
            Err(e) => {
                failed.push(format!("{}: template parse error: {}", name, e));
            }
        }
    }

    println!(
        "\n{} skills passed, {} failed out of {} total",
        passed,
        failed.len(),
        skills.len()
    );

    if !failed.is_empty() {
        panic!(
            "The following skills failed to render:\n  {}",
            failed.join("\n  ")
        );
    }
}
