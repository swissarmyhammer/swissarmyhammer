use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use swissarmyhammer::common::{Parameter, ParameterType};
use swissarmyhammer::prelude::*;
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

use rmcp::ServerHandler;

#[test]
fn test_library_creation() {
    let library = PromptLibrary::new();
    let prompts = library.list().unwrap();
    assert!(prompts.is_empty());
}

#[test]
fn test_prompt_creation_and_rendering() {
    let prompt = Prompt::new("greeting", "Hello {{ name }}!")
        .with_description("A simple greeting prompt")
        .with_category("examples")
        .with_tags(vec!["greeting".to_string(), "example".to_string()]);

    let mut template_vars = HashMap::new();
    template_vars.insert("name".to_string(), json!("World"));

    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let mut library = PromptLibrary::new();
    library.add(prompt).unwrap();
    let rendered = library.render("greeting", &template_context).unwrap();
    assert_eq!(rendered, "Hello World!");
}

#[test]
fn test_prompt_with_arguments() {
    let prompt = Prompt::new("complex", "{{ greeting }}, {{ name }}!")
        .add_parameter(
            Parameter::new("greeting", "The greeting to use", ParameterType::String).required(true),
        )
        .add_parameter(
            Parameter::new("name", "The name to greet", ParameterType::String)
                .required(false)
                .with_default(serde_json::Value::String("Friend".to_string())),
        );

    // Test with all arguments provided
    let mut template_vars = HashMap::new();
    template_vars.insert("greeting".to_string(), json!("Hello"));
    template_vars.insert("name".to_string(), json!("Alice"));

    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let mut library = PromptLibrary::new();
    library.add(prompt.clone()).unwrap();
    let rendered = library.render("complex", &template_context).unwrap();
    assert_eq!(rendered, "Hello, Alice!");

    // Test with default value
    let mut template_vars = HashMap::new();
    template_vars.insert("greeting".to_string(), json!("Hi"));

    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let rendered = library.render("complex", &template_context).unwrap();
    assert_eq!(rendered, "Hi, Friend!");
}

#[test]
fn test_missing_required_argument() {
    let prompt = Prompt::new("test", "Hello {{ name }}!")
        .add_parameter(Parameter::new("name", "", ParameterType::String).required(true));

    let template_vars = HashMap::new();
    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let mut library = PromptLibrary::new();
    library.add(prompt).unwrap();
    let result = library.render("test", &template_context);
    assert!(result.is_err());
}

#[test]
fn test_library_with_directory() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir(&prompts_dir).unwrap();

    // Create a test prompt file
    let prompt_content = r#"---
description: "Test prompt"
category: "test"
tags:
  - test
  - example
arguments:
  - name: subject
    description: "The subject to test"
    required: true
---
Testing {{ subject }}!"#;

    fs::write(prompts_dir.join("test.md"), prompt_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    let count = library.add_directory(&prompts_dir).unwrap();
    assert_eq!(count, 1);

    // Get the prompt
    let prompt = library.get("test").unwrap();
    assert_eq!(prompt.name, "test");
    assert_eq!(prompt.description, Some("Test prompt".to_string()));
    assert_eq!(prompt.category, Some("test".to_string()));
    assert_eq!(prompt.tags, vec!["test", "example"]);

    // Render the prompt
    let mut template_vars = HashMap::new();
    template_vars.insert("subject".to_string(), json!("library"));

    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let rendered = library.render("test", &template_context).unwrap();
    assert_eq!(rendered, "Testing library!");
}

#[test]
fn test_library_search() {
    let mut library = PromptLibrary::new();

    // Add some test prompts
    library
        .add(
            Prompt::new("code-review", "Review this code")
                .with_description("A prompt for code review")
                .with_tags(vec!["code".to_string(), "review".to_string()]),
        )
        .unwrap();

    library
        .add(
            Prompt::new("bug-fix", "Fix this bug")
                .with_description("A prompt for fixing bugs")
                .with_tags(vec!["bug".to_string(), "fix".to_string()]),
        )
        .unwrap();

    library
        .add(
            Prompt::new("refactor-code", "Refactor this code")
                .with_description("A prompt for code refactoring")
                .with_tags(vec!["code".to_string(), "refactor".to_string()]),
        )
        .unwrap();

    // Search for prompts containing "code"
    let results = library.search("code").unwrap();
    assert_eq!(results.len(), 2);

    let names: Vec<String> = results.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"code-review".to_string()));
    assert!(names.contains(&"refactor-code".to_string()));
}

#[test]
fn test_template_engine() {
    let engine = TemplateEngine::new();

    let mut args = HashMap::new();
    args.insert("name".to_string(), "World".to_string());
    args.insert("count".to_string(), "5".to_string());

    // Test simple substitution
    let result = engine.render("Hello {{ name }}!", &args).unwrap();
    assert_eq!(result, "Hello World!");

    // Test filters
    let result = engine.render("{{ name | upcase }}", &args).unwrap();
    assert_eq!(result, "WORLD");

    // Test conditionals
    let template = "{% if count %}Count: {{ count }}{% endif %}";
    let result = engine.render(template, &args).unwrap();
    assert_eq!(result, "Count: 5");
}

// Custom filters test - implemented via preprocessing
#[test]
fn test_custom_filters() {
    let engine = TemplateEngine::new();

    let mut args = HashMap::new();
    args.insert("title".to_string(), "Hello World!".to_string());
    args.insert("text".to_string(), "line1\nline2\nline3".to_string());

    // Test slugify filter
    let result = engine.render("{{ title | slugify }}", &args).unwrap();
    assert_eq!(result, "hello-world");

    // Test count_lines filter
    let result = engine.render("{{ text | count_lines }}", &args).unwrap();
    assert_eq!(result, "3");

    // Test indent filter
    let result = engine.render("{{ text | indent: 2 }}", &args).unwrap();
    assert_eq!(result, "  line1\n  line2\n  line3");
}

#[test]
fn test_prompt_loader() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir(&prompts_dir).unwrap();

    // Create multiple test prompt files
    for i in 1..=3 {
        let content = format!(
            r#"---
description: "Test prompt {i}"
category: "test"
---
This is test prompt {i}!"#
        );
        fs::write(prompts_dir.join(format!("test{i}.md")), content).unwrap();
    }

    // Load all prompts
    let loader = PromptLoader::new();
    let prompts = loader.load_directory(&prompts_dir).unwrap();
    assert_eq!(prompts.len(), 3);

    // Check that all prompts were loaded correctly
    for prompt in prompts.iter() {
        assert!(prompt.name.starts_with("test"));
        assert_eq!(prompt.category, Some("test".to_string()));
    }
}

#[test]
fn test_search_engine() {
    use swissarmyhammer::prelude::SearchEngine;

    let mut engine = SearchEngine::new().unwrap();

    let prompts = vec![
        Prompt::new("code-review", "Review this code")
            .with_description("A prompt for reviewing code quality"),
        Prompt::new("bug-fix", "Fix this bug")
            .with_description("A prompt for fixing software bugs"),
        Prompt::new("documentation", "Write documentation")
            .with_description("A prompt for writing technical documentation"),
    ];

    // Index all prompts
    engine.index_prompts(&prompts).unwrap();

    // Search for "code"
    let results = engine.search("code", &prompts).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].prompt.name, "code-review");

    // Fuzzy search
    let results = engine.fuzzy_search("docu", &prompts);
    assert!(!results.is_empty());
    assert_eq!(results[0].prompt.name, "documentation");
}

#[tokio::test]
async fn test_mcp_server() {
    use swissarmyhammer_tools::McpServer;

    let mut library = PromptLibrary::new();
    library
        .add(Prompt::new("test", "Hello {{ name }}!").with_description("Test prompt"))
        .unwrap();

    let server = McpServer::new(library).unwrap();

    // Test server info
    let info = server.get_info();
    assert!(!info.server_info.name.is_empty());
}

// Example usage for documentation
#[test]
fn test_example_usage() {
    // Create a prompt library
    let mut library = PromptLibrary::new();

    // Create a prompt programmatically
    let greeting_prompt = Prompt::new("greeting", "Hello {{ name }}! Welcome to {{ place }}.")
        .with_description("A friendly greeting prompt")
        .with_category("examples")
        .add_parameter(
            Parameter::new("name", "The person's name", ParameterType::String).required(true),
        )
        .add_parameter(
            Parameter::new("place", "The location", ParameterType::String)
                .required(false)
                .with_default(serde_json::Value::String("our application".to_string())),
        );

    // Add to library
    library.add(greeting_prompt).unwrap();

    // Retrieve and use
    let _prompt = library.get("greeting").unwrap();

    let mut template_vars = HashMap::new();
    template_vars.insert("name".to_string(), json!("Alice"));

    let template_context = TemplateContext::with_template_vars(template_vars).unwrap();
    let output = library.render("greeting", &template_context).unwrap();
    assert_eq!(output, "Hello Alice! Welcome to our application.");
}

#[test]
fn test_partial_rendering() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    let partials_dir = prompts_dir.join("partials");
    fs::create_dir_all(&partials_dir).unwrap();

    // Create a partial template
    let partial_content = r#"---
description: "A partial template for headers"
---
# Welcome to {{ app_name }}!"#;

    fs::write(partials_dir.join("header.liquid.md"), partial_content).unwrap();

    // Create a main template that uses the partial
    let main_content = r#"---
description: "Main template using partial"
---
{% include "partials/header" %}

This is the main content."#;

    fs::write(prompts_dir.join("main.liquid.md"), main_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    let count = library.add_directory(&prompts_dir).unwrap();
    assert!(count > 0);

    // Debug: show what partials are available
    let prompts = library.list().unwrap();
    println!(
        "Available partials: {:?}",
        prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Debug: show partial content
    if let Ok(header_prompt) = library.get("partials/header") {
        println!("Header partial template: '{}'", header_prompt.template);
    }

    // Get and render the main template with partial support
    let mut args_map = HashMap::new();
    args_map.insert(
        "app_name".to_string(),
        Value::String("SwissArmyHammer".to_string()),
    );
    let template_context = TemplateContext::from_hash_map(args_map);

    let rendered = library.render("main", &template_context).unwrap();
    let expected = "# Welcome to SwissArmyHammer!\n\nThis is the main content.";
    assert_eq!(rendered, expected);
}

#[test]
fn test_partial_rendering_with_md_extension() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a partial template with .md extension
    let partial_content = r#"---
description: "A partial template for footers"
---
Footer content: {{ year }}"#;

    fs::write(prompts_dir.join("footer.md"), partial_content).unwrap();

    // Create a main template that uses the partial
    let main_content = r#"---
description: "Main template using .md partial"
---
Main content here.

{% include "footer" %}"#;

    fs::write(prompts_dir.join("main.md"), main_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    library.add_directory(&prompts_dir).unwrap();

    // Get and render the main template with partial support
    let mut args_map = HashMap::new();
    args_map.insert("year".to_string(), Value::String("2024".to_string()));
    let template_context = TemplateContext::from_hash_map(args_map);

    let rendered = library.render("main", &template_context).unwrap();
    let expected = "Main content here.\n\nFooter content: 2024";
    assert_eq!(rendered, expected);
}

#[test]
fn test_liquid_file_extension_loading() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create files with different extensions
    let liquid_md_content = r#"---
description: "A liquid.md file"
---
This is a liquid.md file"#;

    let md_content = r#"---
description: "A regular md file"
---
This is a regular md file"#;

    fs::write(prompts_dir.join("test.liquid.md"), liquid_md_content).unwrap();
    fs::write(prompts_dir.join("test2.md"), md_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    let count = library.add_directory(&prompts_dir).unwrap();

    println!("Loaded {count} prompts");

    // List all loaded prompts
    let prompts = library.list().unwrap();
    for prompt in &prompts {
        println!("Loaded prompt: {} from {:?}", prompt.name, prompt.source);
    }

    // Debug: show what partials are available
    println!(
        "Available partials: {:?}",
        prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // We should have loaded both files
    assert!(count >= 2, "Expected at least 2 prompts, found {count}");

    // Check that both prompts are accessible
    let test_prompt = library.get("test");
    let test2_prompt = library.get("test2");

    if let Err(ref e) = test_prompt {
        println!("Could not find 'test' prompt: {e:?}");
    }
    if let Err(ref e) = test2_prompt {
        println!("Could not find 'test2' prompt: {e:?}");
    }

    assert!(test_prompt.is_ok(), "test.liquid.md file should be loaded");
    assert!(test2_prompt.is_ok(), "test2.md file should be loaded");
}

#[test]
fn test_md_liquid_extension() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    let partials_dir = prompts_dir.join("partials");
    fs::create_dir_all(&partials_dir).unwrap();

    // Create a file with .md.liquid extension as specified in the issue
    let partial_content = r#"---
description: "A partial with .md.liquid extension"
---
This is from partials/top!"#;

    fs::write(partials_dir.join("top.md.liquid"), partial_content).unwrap();

    // Create a main template that uses the partial
    let main_content = r#"---
description: "Main template using .md.liquid partial"
---
Before partial
{% render "partials/top" %}
After partial"#;

    fs::write(prompts_dir.join("main.md"), main_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    library.add_directory(&prompts_dir).unwrap();

    // Debug: List what prompts were loaded
    let prompts = library.list().unwrap();
    println!(
        "Loaded prompts: {:?}",
        prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Verify the partial was loaded with correct name
    assert!(
        library.get("partials/top").is_ok(),
        "partials/top should be accessible"
    );

    // Render the main template
    let template_context = TemplateContext::new();
    let rendered = library.render("main", &template_context).unwrap();
    let expected = "Before partial\nThis is from partials/top!\nAfter partial";
    assert_eq!(rendered, expected);
}

#[test]
fn test_partial_rendering_without_variables() {
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a partial template without variables
    let partial_content = r#"---
description: "A simple partial"
---
This is a static partial."#;

    fs::write(prompts_dir.join("simple.md"), partial_content).unwrap();

    // Create a main template that uses the partial
    let main_content = r#"---
description: "Main template using static partial"
---
Before partial
{% render "simple" %}
After partial"#;

    fs::write(prompts_dir.join("main.md"), main_content).unwrap();

    // Load prompts from directory
    let mut library = PromptLibrary::new();
    library.add_directory(&prompts_dir).unwrap();

    // Get and render the main template with partial support
    let template_context = TemplateContext::new(); // No variables needed

    let rendered = library.render("main", &template_context).unwrap();
    let expected = "Before partial\nThis is a static partial.\nAfter partial";
    assert_eq!(rendered, expected);
}
