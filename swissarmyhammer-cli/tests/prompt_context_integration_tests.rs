//! Integration tests for prompt rendering with TemplateContext

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use swissarmyhammer::PromptLibrary;
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

#[tokio::test]
async fn test_prompt_render_with_config_integration() {
    // Create temporary directory structure
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a test prompt
    let prompt_content = r#"---
title: "Test Project Prompt"
description: "Tests configuration integration"
---
Project: {{project_name}} v{{version}}
Environment: {{env}}
User: {{user_name}}"#;

    fs::write(prompts_dir.join("test_project.md"), prompt_content).unwrap();

    // Create a test configuration context with environment variables
    let mut template_context = TemplateContext::load().unwrap();
    template_context.set(
        "project_name".to_string(),
        serde_json::json!("SwissArmyHammer"),
    );
    template_context.set("version".to_string(), serde_json::json!("1.0.0"));
    template_context.set("env".to_string(), serde_json::json!("test"));
    template_context.set("user_name".to_string(), serde_json::json!(""));

    // Load prompts
    let mut library = PromptLibrary::new();

    // Add the test prompts directory to the library
    let loader = swissarmyhammer::prompts::PromptLoader::new();
    let prompts = loader.load_directory(&prompts_dir).unwrap();
    for prompt in prompts {
        library.add(prompt).unwrap();
    }

    // Test rendering with config context only
    // Debug: print what variables are available
    eprintln!(
        "Template context variables: {:?}",
        template_context.to_hash_map()
    );
    let result = library
        .render("test_project", &template_context)
        .unwrap();

    // println!("Rendered result: {}", result);
    assert!(result.contains("Project: SwissArmyHammer v1.0.0"));
    assert!(result.contains("Environment: test"));
    // Liquid templates render undefined variables as empty strings
    assert!(result.contains("User:")); // This should be "User:" with empty value

    // Test rendering with user argument override
    let mut user_args_map = HashMap::new();
    user_args_map.insert(
        "project_name".to_string(),
        Value::String("UserProject".to_string()),
    );
    user_args_map.insert(
        "user_name".to_string(),
        Value::String("TestUser".to_string()),
    );
    let user_context = TemplateContext::from_hash_map(user_args_map);

    let mut combined_context = template_context.clone();
    combined_context.merge(user_context);

    let result_with_override = library.render("test_project", &combined_context).unwrap();

    assert!(result_with_override.contains("Project: UserProject v1.0.0")); // User override + config fallback
    assert!(result_with_override.contains("Environment: test")); // Config value
    assert!(result_with_override.contains("User: TestUser")); // User provided
}

#[tokio::test]
async fn test_prompt_render_with_env_and_context_integration() {
    // Create temporary directory structure
    let temp_dir = TempDir::new().unwrap();
    let prompts_dir = temp_dir.path().join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a test prompt that uses environment variables
    let prompt_content = r#"---
title: "Environment Test Prompt"
description: "Tests environment variable integration"
---
App: {{app_name}}
Home: {{HOME}}
Current User: {{USER}}"#;

    fs::write(prompts_dir.join("env_test.md"), prompt_content).unwrap();

    // Create a test configuration context with environment variables
    let mut template_context = TemplateContext::load().unwrap();
    template_context.set("app_name".to_string(), serde_json::json!("TestApp"));

    // Add environment variables that the test expects
    template_context.set(
        "HOME".to_string(),
        serde_json::json!(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())),
    );
    template_context.set(
        "USER".to_string(),
        serde_json::json!(std::env::var("USER").unwrap_or_else(|_| "testuser".to_string())),
    );

    // Load prompts
    let mut library = PromptLibrary::new();
    let loader = swissarmyhammer::prompts::PromptLoader::new();
    let prompts = loader.load_directory(&prompts_dir).unwrap();
    for prompt in prompts {
        library.add(prompt).unwrap();
    }

    // Test rendering with config and environment variables
    let result = library
        .render("env_test", &template_context)
        .unwrap();

    assert!(result.contains("App: TestApp")); // From config
                                              // Environment variables should be available (HOME and USER are typically set)
}
