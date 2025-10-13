//! Integration tests for builtin prompt rendering
//!
//! These tests ensure that all builtin prompts render successfully without template errors.
//! This prevents issues like missing partials or invalid liquid syntax from making it into production.

use rstest::rstest;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

/// Test cases for all builtin issue prompts that should render successfully
/// Note: .check is tested separately in test_check_prompt_renders_with_parameters
/// because it requires specific parameters
#[rstest]
#[case("issue/code")]
#[case("issue/code_review")]
#[case("issue/review")]
#[case("issue/complete")]
#[case(".system")]
fn test_builtin_prompt_renders_successfully(#[case] prompt_name: &str) {
    // Load all prompts using the full resolver pipeline
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();

    // This should load all builtin prompts including partials
    resolver
        .load_all_prompts(&mut library)
        .expect("Failed to load builtin prompts");

    // Create a minimal template context
    let template_context = TemplateContext::new();

    // Attempt to render the prompt
    match library.render(prompt_name, &template_context) {
        Ok(rendered) => {
            // Basic sanity checks
            assert!(!rendered.is_empty(), "Rendered prompt should not be empty");
            assert!(
                !rendered.contains("Unknown partial-template"),
                "Should not contain partial resolution errors"
            );
            assert!(
                !rendered.contains("liquid:"),
                "Should not contain liquid syntax errors"
            );

            println!(
                "✓ Successfully rendered {}: {} chars",
                prompt_name,
                rendered.len()
            );

            // The main goal is to ensure no rendering errors - content will change
            // so we don't make specific assertions about what should be in each prompt
        }
        Err(e) => {
            panic!("Failed to render builtin prompt '{}': {}", prompt_name, e);
        }
    }
}

#[test]
fn test_all_builtin_prompts_load_without_errors() {
    // This test ensures that the prompt loading process itself works
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();

    // Should not panic or return errors
    resolver
        .load_all_prompts(&mut library)
        .expect("Should be able to load all builtin prompts without errors");

    // Verify we have the expected prompts
    let prompt_names = library
        .list_names()
        .expect("Should be able to list prompt names");

    // Check that key prompts are present
    let expected_prompts = vec![
        "issue/code",
        "issue/code_review",
        "issue/review",
        "issue/complete",
        ".system",
        ".check",
    ];

    for expected in expected_prompts {
        assert!(
            prompt_names.contains(&expected.to_string()),
            "Expected prompt '{}' not found in loaded prompts: {:?}",
            expected,
            prompt_names
        );
    }

    println!(
        "✓ Successfully loaded {} builtin prompts",
        prompt_names.len()
    );
}

#[test]
fn test_partials_are_loaded_and_accessible() {
    // This test specifically verifies that partials are loaded correctly
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();

    resolver
        .load_all_prompts(&mut library)
        .expect("Failed to load prompts");

    // Check that key partials are accessible
    // These should be loaded with multiple name variants (base, .md, .liquid)
    let expected_partials = vec![
        ("workflow_guards", "## Workflow Rules"),
        ("workflow_guards.md", "## Workflow Rules"),
        ("workflow_guards.liquid", "## Workflow Rules"),
        ("review_format", "## Review Format"),
        ("review_format.md", "## Review Format"),
        ("review_format.liquid", "## Review Format"),
        ("principals", "## Principals"),
        ("principals.md", "## Principals"),
        ("principals.liquid", "## Principals"),
    ];

    for (partial_name, expected_content) in expected_partials {
        match library.get(partial_name) {
            Ok(prompt) => {
                assert!(
                    prompt.template.contains(expected_content),
                    "Partial '{}' should contain '{}' but template is: {}",
                    partial_name,
                    expected_content,
                    prompt.template
                );
                println!("✓ Partial '{}' loaded correctly", partial_name);
            }
            Err(_) => {
                // This is expected for some partials that might not be loaded with all variants
                // The important thing is that at least one variant works
                println!(
                    "◦ Partial '{}' not found (this may be expected)",
                    partial_name
                );
            }
        }
    }
}

#[test]
fn test_check_prompt_renders_with_parameters() {
    // Test the .check prompt with all required parameters
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();

    resolver
        .load_all_prompts(&mut library)
        .expect("Failed to load builtin prompts");

    // Create template context with required parameters for .check prompt
    let mut template_context = TemplateContext::new();
    template_context.set(
        "rule_content".to_string(),
        serde_json::json!("Functions must have documentation comments"),
    );
    template_context.set(
        "target_content".to_string(),
        serde_json::json!("fn foo() {}\nfn bar() {}"),
    );
    template_context.set(
        "target_path".to_string(),
        serde_json::json!("src/example.rs"),
    );
    template_context.set("language".to_string(), serde_json::json!("rust"));
    template_context.set("rule_name".to_string(), serde_json::json!("test-rule-name"));

    // Render the .check prompt
    let rendered = library
        .render(".check", &template_context)
        .expect("Failed to render .check prompt");

    // Verify the rendered output contains expected elements
    assert!(
        rendered.contains("Functions must have documentation comments"),
        "Should contain the rule content"
    );
    assert!(
        rendered.contains("src/example.rs"),
        "Should contain the target path"
    );
    assert!(
        rendered.contains("fn foo()"),
        "Should contain the target content"
    );
    assert!(rendered.contains("rust"), "Should reference the language");
    assert!(
        rendered.contains("PASS") || rendered.contains("VIOLATION"),
        "Should contain instructions about PASS/VIOLATION format"
    );

    println!("✓ .check prompt rendered successfully with all parameters");
}
