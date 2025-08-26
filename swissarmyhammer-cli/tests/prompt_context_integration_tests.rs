//! Integration tests for prompt rendering with TemplateContext

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

    // Create a test configuration context
    let mut template_context = TemplateContext::new();
    template_context.set(
        "project_name".to_string(),
        serde_json::json!("SwissArmyHammer"),
    );
    template_context.set("version".to_string(), serde_json::json!("1.0.0"));
    template_context.set("env".to_string(), serde_json::json!("test"));

    // Load prompts
    let mut library = PromptLibrary::new();

    // Add the test prompts directory to the library
    let loader = swissarmyhammer::prompts::PromptLoader::new();
    let prompts = loader.load_directory(&prompts_dir).unwrap();
    for prompt in prompts {
        library.add(prompt).unwrap();
    }

    // Test rendering with config context only
    let empty_args = HashMap::new();
    let result = library
        .render_prompt_with_context("test_project", &template_context, &empty_args)
        .unwrap();

    // println!("Rendered result: {}", result);
    assert!(result.contains("Project: SwissArmyHammer v1.0.0"));
    assert!(result.contains("Environment: test"));
    // Liquid templates render undefined variables as empty strings
    assert!(result.contains("User:")); // This should be "User:" with empty value

    // Test rendering with user argument override
    let mut user_args = HashMap::new();
    user_args.insert("project_name".to_string(), "UserProject".to_string());
    user_args.insert("user_name".to_string(), "TestUser".to_string());

    let result_with_override = library
        .render_prompt_with_context("test_project", &template_context, &user_args)
        .unwrap();

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

    // Create a test configuration context
    let mut template_context = TemplateContext::new();
    template_context.set("app_name".to_string(), serde_json::json!("TestApp"));

    // Load prompts
    let mut library = PromptLibrary::new();
    let loader = swissarmyhammer::prompts::PromptLoader::new();
    let prompts = loader.load_directory(&prompts_dir).unwrap();
    for prompt in prompts {
        library.add(prompt).unwrap();
    }

    // Test rendering with config and environment variables
    let empty_args = HashMap::new();
    let result = library
        .render_prompt_with_env_and_context("env_test", &template_context, &empty_args)
        .unwrap();

    assert!(result.contains("App: TestApp")); // From config
                                              // Environment variables should be available (HOME and USER are typically set)
}
