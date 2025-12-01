use std::{env, fs};
use swissarmyhammer_config::agent::{
    parse_agent_config, parse_agent_description, AgentError, AgentManager, AgentSource,
};
use swissarmyhammer_config::{AgentConfig, AgentExecutorType, LlamaAgentConfig};
use tempfile::TempDir;

#[test]
fn test_agent_config_system_default_is_claude() {
    let config = AgentConfig::default();
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(!config.quiet);
}

#[test]
fn test_agent_config_factories() {
    let claude_config = AgentConfig::claude_code();
    assert_eq!(claude_config.executor_type(), AgentExecutorType::ClaudeCode);

    let llama_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
    assert_eq!(llama_config.executor_type(), AgentExecutorType::LlamaAgent);
}

#[test]
fn test_agent_config_serialization() {
    let config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());

    // Should serialize to YAML correctly
    let yaml = serde_yaml::to_string(&config).expect("Failed to serialize to YAML");
    assert!(yaml.contains("type: llama-agent"));
    assert!(yaml.contains("quiet: false"));

    // Should deserialize from YAML correctly
    let deserialized: AgentConfig =
        serde_yaml::from_str(&yaml).expect("Failed to deserialize from YAML");
    assert_eq!(config.executor_type(), deserialized.executor_type());
    assert_eq!(config.quiet, deserialized.quiet);
}

// =============================================================================
// ADDITIONAL COMPREHENSIVE TESTS FOR AGENT MANAGER
// =============================================================================

#[test]
fn test_agent_manager_load_builtin_agents_comprehensive() {
    let agents = AgentManager::load_builtin_agents().expect("Should load builtin agents");

    // Should have at least the known builtin agents
    assert!(!agents.is_empty(), "Should have builtin agents");

    let agent_names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"claude-code"),
        "Should have claude-code"
    );
    assert!(
        agent_names.contains(&"qwen-coder"),
        "Should have qwen-coder"
    );
    assert!(
        agent_names.contains(&"qwen-coder-flash"),
        "Should have qwen-coder-flash"
    );

    // All should be builtin source
    for agent in &agents {
        assert_eq!(agent.source, AgentSource::Builtin);
        assert!(!agent.name.is_empty());
        assert!(!agent.content.is_empty());
    }

    // Test that agent configs are valid
    for agent in &agents {
        let parsed_config = parse_agent_config(&agent.content);
        assert!(
            parsed_config.is_ok(),
            "Builtin agent '{}' should have valid config",
            agent.name
        );
    }
}

#[test]
fn test_agent_manager_precedence_logic_detailed() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_home = temp_dir.path().join("home");
    let project_root = temp_dir.path().join("project");

    fs::create_dir_all(&temp_home).expect("Failed to create temp home");
    fs::create_dir_all(&project_root).expect("Failed to create project root");

    // Set up user agents that override builtin
    let user_agents_dir = temp_home.join(".swissarmyhammer").join("agents");
    fs::create_dir_all(&user_agents_dir).expect("Failed to create user agents dir");

    let user_claude_override = r#"---
description: "User override of Claude Code"
---
executor:
  type: claude-code
  config:
    claude_path: /user/claude
    args: ["--user-override"]
quiet: true"#;
    fs::write(
        user_agents_dir.join("claude-code.yaml"),
        user_claude_override,
    )
    .expect("Failed to write user claude override");

    let user_custom = r#"---
description: "User custom agent"
---
executor:
  type: claude-code
  config:
    claude_path: /user/custom
    args: ["--custom"]
quiet: false"#;
    fs::write(user_agents_dir.join("user-custom.yaml"), user_custom)
        .expect("Failed to write user custom agent");

    // Set up project agents that override builtin and user
    let project_agents_dir = project_root.join("agents");
    fs::create_dir_all(&project_agents_dir).expect("Failed to create project agents dir");

    let project_qwen_override = r#"---
description: "Project override of Qwen Coder"
---
executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "project/custom-qwen"
        filename: "model.gguf"
    mcp_server:
      port: 0
      timeout_seconds: 30
quiet: false"#;
    fs::write(
        project_agents_dir.join("qwen-coder.yaml"),
        project_qwen_override,
    )
    .expect("Failed to write project qwen override");

    let project_specific = r#"---
description: "Project specific agent"
---
executor:
  type: claude-code
  config:
    claude_path: /project/specific
    args: ["--project-mode"]
quiet: true"#;
    fs::write(
        project_agents_dir.join("project-specific.yaml"),
        project_specific,
    )
    .expect("Failed to write project specific agent");

    // Save original environment
    let original_home = env::var("HOME").ok();
    let original_dir = env::current_dir().expect("Failed to get current dir");

    // Set test environment
    env::set_var("HOME", &temp_home);
    env::set_current_dir(&project_root).expect("Failed to change to project dir");

    // Test full hierarchy precedence
    let agents = AgentManager::list_agents().expect("Should list all agents with precedence");

    // Restore environment
    env::set_current_dir(&original_dir).expect("Failed to restore dir");
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }

    // Verify precedence rules
    let agent_map: std::collections::HashMap<_, _> = agents
        .iter()
        .map(|agent| (agent.name.as_str(), agent))
        .collect();

    // claude-code should come from user (highest precedence for user override)
    let claude_agent = agent_map
        .get("claude-code")
        .expect("Should have claude-code");
    assert_eq!(
        claude_agent.source,
        AgentSource::User,
        "claude-code should be from user source"
    );
    assert!(
        claude_agent
            .description
            .as_ref()
            .unwrap()
            .contains("User override"),
        "Should have user override description"
    );

    // qwen-coder should come from project (project override)
    let qwen_agent = agent_map.get("qwen-coder").expect("Should have qwen-coder");
    assert_eq!(
        qwen_agent.source,
        AgentSource::Project,
        "qwen-coder should be from project source"
    );
    assert!(
        qwen_agent
            .description
            .as_ref()
            .unwrap()
            .contains("Project override"),
        "Should have project override description"
    );

    // qwen-coder-flash should remain builtin (no overrides)
    let qwen_flash_agent = agent_map
        .get("qwen-coder-flash")
        .expect("Should have qwen-coder-flash");
    assert_eq!(
        qwen_flash_agent.source,
        AgentSource::Builtin,
        "qwen-coder-flash should remain builtin"
    );

    // Should have user-custom agent
    let user_custom_agent = agent_map
        .get("user-custom")
        .expect("Should have user-custom");
    assert_eq!(user_custom_agent.source, AgentSource::User);

    // Should have project-specific agent
    let project_specific_agent = agent_map
        .get("project-specific")
        .expect("Should have project-specific");
    assert_eq!(project_specific_agent.source, AgentSource::Project);
}

#[test]
fn test_agent_manager_config_file_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path();

    let original_dir = env::current_dir().expect("Failed to get current dir");
    env::set_current_dir(project_root).expect("Failed to change to project dir");

    // Test config structure creation
    let config_path =
        AgentManager::ensure_config_structure().expect("Should create config structure");

    assert!(
        config_path.parent().unwrap().exists(),
        "Config directory should exist"
    );
    assert_eq!(
        config_path.file_name().unwrap(),
        "sah.yaml",
        "Should default to YAML format"
    );

    // Test using an agent (creates config)
    AgentManager::use_agent("claude-code").expect("Should use claude-code agent");

    assert!(config_path.exists(), "Config file should be created");

    let config_content = fs::read_to_string(&config_path).expect("Should read config file");
    assert!(
        config_content.contains("agents:"),
        "Should contain agents section"
    );
    assert!(
        config_content.contains("root:"),
        "Should contain root use case"
    );
    assert!(
        config_content.contains("claude-code"),
        "Should contain claude-code config"
    );

    // Test updating existing config
    let original_size = fs::metadata(&config_path).unwrap().len();

    AgentManager::use_agent("qwen-coder").expect("Should update to qwen-coder");

    let updated_content = fs::read_to_string(&config_path).expect("Should read updated config");
    assert!(
        updated_content.contains("agents:"),
        "Should still contain agents section"
    );
    assert!(
        updated_content.contains("qwen-coder"),
        "Should contain new agent config"
    );

    let updated_size = fs::metadata(&config_path).unwrap().len();
    assert_ne!(
        original_size, updated_size,
        "Config file should be modified"
    );

    env::set_current_dir(&original_dir).expect("Failed to restore dir");
}

#[test]
fn test_agent_manager_config_file_preservation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path();

    let original_dir = env::current_dir().expect("Failed to get current dir");
    env::set_current_dir(project_root).expect("Failed to change to project dir");

    // Create existing config with multiple sections
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).expect("Failed to create sah dir");
    let config_path = sah_dir.join("sah.yaml");

    let existing_config = r#"# Existing configuration with comments
prompt:
  default_template: "greeting"
  
workflows:
  - name: "test-workflow"
    description: "Test workflow"
    
other_section:
  preserved_value: "should not be lost"
  nested:
    data: 42
    
existing_agent:
  old_config: "will be replaced"
"#;
    fs::write(&config_path, existing_config).expect("Failed to write existing config");

    // Use agent to update config
    AgentManager::use_agent("claude-code").expect("Should update agent config");

    let updated_config = fs::read_to_string(&config_path).expect("Should read updated config");

    // Should preserve existing sections
    assert!(
        updated_config.contains("prompt:"),
        "Should preserve prompt section"
    );
    assert!(
        updated_config.contains("default_template"),
        "Should preserve prompt config"
    );
    assert!(
        updated_config.contains("workflows:"),
        "Should preserve workflows section"
    );
    assert!(
        updated_config.contains("test-workflow"),
        "Should preserve workflow data"
    );
    assert!(
        updated_config.contains("other_section:"),
        "Should preserve other_section"
    );
    assert!(
        updated_config.contains("preserved_value"),
        "Should preserve nested data"
    );
    assert!(
        updated_config.contains("nested:"),
        "Should preserve nested structure"
    );

    // Should update agents section
    assert!(
        updated_config.contains("agents:"),
        "Should have agents section"
    );
    assert!(
        updated_config.contains("root:"),
        "Should have root use case"
    );

    env::set_current_dir(&original_dir).expect("Failed to restore dir");
}

#[test]
fn test_agent_manager_error_handling_comprehensive() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path();

    let original_dir = env::current_dir().expect("Failed to get current dir");
    env::set_current_dir(project_root).expect("Failed to change to project dir");

    // Test agent not found error
    let result = AgentManager::use_agent("definitely-nonexistent-agent-name-12345");
    assert!(result.is_err(), "Should fail for nonexistent agent");

    match result {
        Err(AgentError::NotFound(name)) => {
            assert_eq!(name, "definitely-nonexistent-agent-name-12345");
        }
        _ => panic!("Should return NotFound error"),
    }

    // Test find_agent_by_name with nonexistent
    let find_result = AgentManager::find_agent_by_name("another-nonexistent-agent");
    assert!(
        find_result.is_err(),
        "Should fail to find nonexistent agent"
    );

    match find_result {
        Err(AgentError::NotFound(name)) => {
            assert_eq!(name, "another-nonexistent-agent");
        }
        _ => panic!("Should return NotFound error for find"),
    }

    // Test successful agent lookup
    let found_agent = AgentManager::find_agent_by_name("claude-code")
        .expect("Should find builtin claude-code agent");
    assert_eq!(found_agent.name, "claude-code");
    assert_eq!(found_agent.source, AgentSource::Builtin);

    env::set_current_dir(&original_dir).expect("Failed to restore dir");
}

#[test]
fn test_agent_manager_invalid_agent_files_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_home = temp_dir.path();

    // Create user agents directory with mix of valid and invalid files
    let user_agents_dir = temp_home.join(".swissarmyhammer").join("agents");
    fs::create_dir_all(&user_agents_dir).expect("Failed to create user agents dir");

    // Valid agent
    let valid_agent = r#"---
description: "Valid test agent"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: false"#;
    fs::write(user_agents_dir.join("valid-agent.yaml"), valid_agent)
        .expect("Failed to write valid agent");

    // Invalid YAML syntax
    let invalid_yaml = "invalid: yaml: content: [unclosed bracket";
    fs::write(user_agents_dir.join("invalid-syntax.yaml"), invalid_yaml)
        .expect("Failed to write invalid yaml");

    // Invalid agent config structure
    let invalid_config = r#"---
description: "Invalid agent config"
---
executor:
  type: unknown-executor-type
  config: "not an object"
invalid_field: true"#;
    fs::write(user_agents_dir.join("invalid-config.yaml"), invalid_config)
        .expect("Failed to write invalid config");

    // Non-YAML file (should be ignored)
    fs::write(
        user_agents_dir.join("not-agent.txt"),
        "This is not an agent file",
    )
    .expect("Failed to write non-yaml file");

    // Save original environment
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    // Test that loading continues despite invalid files
    let user_agents =
        AgentManager::load_user_agents().expect("Should load user agents despite invalid files");

    // Restore environment
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }

    // Should only load the valid agent
    assert_eq!(user_agents.len(), 1, "Should load only valid agent");
    assert_eq!(user_agents[0].name, "valid-agent");
    assert_eq!(user_agents[0].source, AgentSource::User);
    assert!(user_agents[0]
        .description
        .as_ref()
        .unwrap()
        .contains("Valid test agent"));

    // Test that invalid agents are not in the list
    let agent_names: Vec<_> = user_agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        !agent_names.contains(&"invalid-syntax"),
        "Should not load invalid syntax"
    );
    assert!(
        !agent_names.contains(&"invalid-config"),
        "Should not load invalid config"
    );
    assert!(
        !agent_names.contains(&"not-agent"),
        "Should not load non-yaml file"
    );
}

#[test]
fn test_agent_description_parsing_comprehensive() {
    // Test YAML frontmatter parsing
    let yaml_content = r#"---
description: "YAML frontmatter description"
version: "1.0"
author: "Test Author"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(yaml_content);
    assert_eq!(
        description,
        Some("YAML frontmatter description".to_string())
    );

    // Test comment-based parsing
    let comment_content = r#"# Description: Comment-based description
# Additional comment
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(comment_content);
    assert_eq!(description, Some("Comment-based description".to_string()));

    // Test YAML precedence over comments
    let mixed_content = r#"---
description: "YAML takes precedence"
---
# Description: This should be ignored
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(mixed_content);
    assert_eq!(description, Some("YAML takes precedence".to_string()));

    // Test no description
    let no_desc_content = r#"executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(no_desc_content);
    assert_eq!(description, None);

    // Test empty description
    let empty_desc_content = r#"---
description: ""
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(empty_desc_content);
    assert_eq!(description, Some("".to_string()));

    // Test whitespace handling
    let whitespace_content = r#"---
description: "  Trimmed description  "
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_agent_description(whitespace_content);
    assert_eq!(description, Some("Trimmed description".to_string()));
}

#[test]
fn test_agent_config_parsing_comprehensive() {
    // Test parsing with YAML frontmatter
    let frontmatter_content = r#"---
description: "Test agent with frontmatter"
version: "1.0"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: true"#;

    let config = parse_agent_config(frontmatter_content)
        .expect("Should parse agent config with frontmatter");
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(config.quiet);

    // Test parsing pure config
    let pure_config_content = r#"executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "test/model"
        filename: "test.gguf"
    mcp_server:
      port: 0
      timeout_seconds: 30
quiet: false"#;

    let config = parse_agent_config(pure_config_content).expect("Should parse pure agent config");
    assert_eq!(config.executor_type(), AgentExecutorType::LlamaAgent);
    assert!(!config.quiet);

    // Test parsing with comments
    let comment_config_content = r#"# Description: Test agent with comments
# Version: 1.0
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let config = parse_agent_config(comment_config_content)
        .expect("Should parse agent config with comments");
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(!config.quiet);

    // Test parsing invalid config
    let invalid_content = "invalid yaml content [unclosed";
    let result = parse_agent_config(invalid_content);
    assert!(result.is_err(), "Should fail to parse invalid YAML");
}

#[test]
fn test_agent_manager_directory_loading_edge_cases() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test loading from non-existent directory
    let nonexistent_dir = temp_dir.path().join("nonexistent");
    let result = AgentManager::load_agents_from_dir(&nonexistent_dir, AgentSource::User);
    assert!(
        result.is_ok(),
        "Should handle non-existent directory gracefully"
    );
    assert_eq!(
        result.unwrap().len(),
        0,
        "Should return empty list for non-existent dir"
    );

    // Test loading from empty directory
    let empty_dir = temp_dir.path().join("empty");
    fs::create_dir_all(&empty_dir).expect("Failed to create empty dir");
    let result = AgentManager::load_agents_from_dir(&empty_dir, AgentSource::User);
    assert!(result.is_ok(), "Should handle empty directory gracefully");
    assert_eq!(
        result.unwrap().len(),
        0,
        "Should return empty list for empty dir"
    );

    // Test loading directory with only non-YAML files
    let non_yaml_dir = temp_dir.path().join("non_yaml");
    fs::create_dir_all(&non_yaml_dir).expect("Failed to create non-yaml dir");
    fs::write(non_yaml_dir.join("readme.txt"), "Not an agent").expect("Failed to write readme");
    fs::write(non_yaml_dir.join("config.json"), r#"{"not": "agent"}"#)
        .expect("Failed to write json");

    let result = AgentManager::load_agents_from_dir(&non_yaml_dir, AgentSource::User);
    assert!(result.is_ok(), "Should handle directory with no YAML files");
    assert_eq!(result.unwrap().len(), 0, "Should ignore non-YAML files");
}
