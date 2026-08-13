//! `ModelManager` against a real directory.
//!
//! Loading builtin, user and project models, the precedence between them,
//! finding an agent by name, and detecting the configuration file.

use super::*;

#[test]
fn test_agent_manager_load_builtin_models() {
    let agents = ModelManager::load_builtin_models().expect("Failed to load builtin models");

    // Should contain at least the known builtin agents
    assert!(!agents.is_empty(), "Builtin agents should not be empty");

    // All agents should have Builtin source
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::Builtin);
        assert!(!agent.name.is_empty(), "Agent name should not be empty");
        assert!(
            !agent.content.is_empty(),
            "Agent content should not be empty"
        );
    }

    // Check for known builtin agents
    let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"nomic-embed-code"),
        "Should contain the nomic-embed-code embedding model"
    );
    assert!(
        agent_names.contains(&"qwen-embedding"),
        "Should contain the qwen-embedding embedding model"
    );
}

#[test]
fn test_agent_manager_load_agents_from_missing_dir() {
    use std::path::Path;

    let non_existent_dir = Path::new("/non/existent/directory");
    let result = ModelManager::load_models_from_dir(non_existent_dir, ModelConfigSource::User);

    assert!(result.is_ok(), "Should handle missing directory gracefully");
    let agents = result.unwrap();
    assert!(
        agents.is_empty(),
        "Should return empty vector for missing directory"
    );
}

#[test]
fn test_agent_manager_load_models_from_dir_with_temp_files() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create test agent files
    let agent1_content = r#"---
description: "Test agent 1"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;
    fs::write(temp_path.join("test-agent-1.yaml"), agent1_content)
        .expect("Failed to write test agent 1");

    let agent2_content = r#"# Description: Test agent 2
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
      filename: test.gguf
    normalize: true
quiet: false"#;
    fs::write(temp_path.join("test-agent-2.yaml"), agent2_content)
        .expect("Failed to write test agent 2");

    // Create a non-YAML file that should be ignored
    fs::write(temp_path.join("not-an-agent.txt"), "ignored content")
        .expect("Failed to write non-yaml file");

    let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::Project);
    if let Err(e) = &result {
        eprintln!("Error loading agents: {:?}", e);
    }
    assert!(
        result.is_ok(),
        "Should load agents from directory successfully: {:?}",
        result
    );

    let agents = result.unwrap();
    println!("Loaded {} agents", agents.len());
    if agents.is_empty() {
        println!("No agents loaded. Directory contents:");
        for entry in std::fs::read_dir(temp_path).unwrap() {
            let entry = entry.unwrap();
            println!("  {:?}", entry.path());
        }
    }
    assert_eq!(agents.len(), 2, "Should load exactly 2 YAML files");

    // Check that all agents have correct source
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::Project);
    }

    // Find specific agents
    let agent1 = agents.iter().find(|a| a.name == "test-agent-1");
    let agent2 = agents.iter().find(|a| a.name == "test-agent-2");

    assert!(agent1.is_some(), "Should find test-agent-1");
    assert!(agent2.is_some(), "Should find test-agent-2");

    let agent1 = agent1.unwrap();
    let agent2 = agent2.unwrap();

    assert_eq!(agent1.description, Some("Test agent 1".to_string()));
    assert_eq!(agent2.description, Some("Test agent 2".to_string()));
}

#[test]
fn test_agent_manager_load_user_agents() {
    let result = ModelManager::load_user_models();

    // Should not fail even if no user agents exist
    assert!(
        result.is_ok(),
        "Should handle user agent loading gracefully"
    );

    let agents = result.unwrap();
    // All agents should have User source
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::User);
    }
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_load_project_models() {
    let result = ModelManager::load_project_models();

    // Should not fail even if no project agents exist
    assert!(
        result.is_ok(),
        "Should handle project agent loading gracefully"
    );

    let agents = result.unwrap();
    // All agents should have Project source
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::Project);
    }
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_list_agents_precedence() {
    // This test verifies the complete agent discovery hierarchy with precedence
    let result = ModelManager::list_agents();

    assert!(result.is_ok(), "list_agents() should not fail");
    let agents = result.unwrap();

    // Should contain at least built-in agents
    assert!(
        !agents.is_empty(),
        "Should contain at least built-in agents"
    );

    // Verify precedence: user > project > builtin
    // If there are duplicate names, user/project should override builtin
    let mut seen_names = std::collections::HashSet::new();
    for agent in &agents {
        if seen_names.contains(&agent.name) {
            panic!(
                "Duplicate agent name found: {}. Precedence system should prevent duplicates.",
                agent.name
            );
        }
        seen_names.insert(&agent.name);
    }

    // All agents should have proper source assignments
    for agent in &agents {
        match agent.source {
            ModelConfigSource::Builtin
            | ModelConfigSource::Project
            | ModelConfigSource::GitRoot
            | ModelConfigSource::User => {
                // Valid source
            }
        }
        assert!(!agent.name.is_empty(), "Agent name should not be empty");
        assert!(
            !agent.content.is_empty(),
            "Agent content should not be empty"
        );
    }

    // Should contain known builtin agents unless overridden
    let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"nomic-embed-code"),
        "Should contain the nomic-embed-code embedding model"
    );
    assert!(
        agent_names.contains(&"qwen-embedding"),
        "Should contain the qwen-embedding embedding model"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_list_agents_overriding_with_temp_files() {
    use std::fs;

    let temp_project_dir = tempfile::TempDir::new().expect("Failed to create temp project dir");
    let temp_user_dir = tempfile::TempDir::new().expect("Failed to create temp user dir");

    // Create project model that overrides a builtin model
    let project_override_content = r#"---
description: "Project-overridden embedding model"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: project/embed
quiet: true"#;

    let project_agents_dir = temp_project_dir.path().join("models");
    fs::create_dir_all(&project_agents_dir).expect("Failed to create project agents dir");
    fs::write(
        project_agents_dir.join("qwen-embedding.yaml"),
        project_override_content,
    )
    .expect("Failed to write project qwen-embedding model");

    // Create user model that overrides the project model
    let user_override_content = r#"---
description: "User-overridden embedding model"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: user/embed
quiet: false"#;

    let user_agents_dir = temp_user_dir.path().join("models");
    fs::create_dir_all(&user_agents_dir).expect("Failed to create user agents dir");
    fs::write(
        user_agents_dir.join("qwen-embedding.yaml"),
        user_override_content,
    )
    .expect("Failed to write user qwen-embedding model");

    // Create a unique project model
    let unique_project_content = r#"---
description: "Unique project agent"
---
executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: project/unique
quiet: false"#;
    fs::write(
        project_agents_dir.join("unique-project.yaml"),
        unique_project_content,
    )
    .expect("Failed to write unique project agent");

    // Mock home directory for user agents test
    // Note: This is tricky to test without mocking the dirs::home_dir() function
    // For now, we'll test the directory loading function directly

    // Test direct directory loading instead since we can't easily mock home_dir
    let project_agents =
        ModelManager::load_models_from_dir(&project_agents_dir, ModelConfigSource::Project);
    assert!(
        project_agents.is_ok(),
        "Should load project agents successfully"
    );

    let project_agents = project_agents.unwrap();
    assert_eq!(project_agents.len(), 2, "Should load 2 project agents");

    // Verify project agents
    let override_agent = project_agents.iter().find(|a| a.name == "qwen-embedding");
    assert!(
        override_agent.is_some(),
        "Should find overridden qwen-embedding model"
    );
    let override_agent = override_agent.unwrap();
    assert_eq!(override_agent.source, ModelConfigSource::Project);
    assert_eq!(
        override_agent.description,
        Some("Project-overridden embedding model".to_string())
    );

    let unique_agent = project_agents.iter().find(|a| a.name == "unique-project");
    assert!(unique_agent.is_some(), "Should find unique project agent");
    let unique_agent = unique_agent.unwrap();
    assert_eq!(unique_agent.source, ModelConfigSource::Project);
    assert_eq!(
        unique_agent.description,
        Some("Unique project agent".to_string())
    );

    // Test user agents
    let user_agents = ModelManager::load_models_from_dir(&user_agents_dir, ModelConfigSource::User);
    assert!(user_agents.is_ok(), "Should load user agents successfully");

    let user_agents = user_agents.unwrap();
    assert_eq!(user_agents.len(), 1, "Should load 1 user agent");

    let user_override = &user_agents[0];
    assert_eq!(user_override.name, "qwen-embedding");
    assert_eq!(user_override.source, ModelConfigSource::User);
    assert_eq!(
        user_override.description,
        Some("User-overridden embedding model".to_string())
    );
}

#[test]
fn test_agent_manager_list_agents_validation_errors() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create multiple invalid YAML files with different types of errors
    let invalid_yaml_content = "invalid: yaml: content: [unclosed";
    fs::write(temp_path.join("invalid-yaml.yaml"), invalid_yaml_content)
        .expect("Failed to write invalid YAML agent");

    let invalid_config_content = r#"---
description: "Invalid agent config"
---
executor:
  type: unknown-executor-type
  config: {}
quiet: not-a-boolean"#;
    fs::write(
        temp_path.join("invalid-config.yaml"),
        invalid_config_content,
    )
    .expect("Failed to write invalid config agent");

    // Create multiple valid agent files
    let valid_content1 = r#"---
description: "Valid agent 1"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;
    fs::write(temp_path.join("valid-agent-1.yaml"), valid_content1)
        .expect("Failed to write valid agent 1");

    let valid_content2 = r#"---
description: "Valid agent 2"
---
executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: test/ane
quiet: true"#;
    fs::write(temp_path.join("valid-agent-2.yaml"), valid_content2)
        .expect("Failed to write valid agent 2");

    // Test that loading continues despite invalid agents and loads only valid ones
    let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::Project);

    // The function should succeed and load only valid agents
    assert!(
        result.is_ok(),
        "Should successfully load valid agents while skipping invalid ones"
    );

    let agents = result.unwrap();
    assert_eq!(
        agents.len(),
        2,
        "Should load exactly 2 valid agents, skipping 2 invalid ones"
    );

    // Verify the loaded agents are the valid ones
    let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"valid-agent-1"),
        "Should contain valid-agent-1"
    );
    assert!(
        agent_names.contains(&"valid-agent-2"),
        "Should contain valid-agent-2"
    );

    // Verify agent details
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::Project);
        assert!(!agent.name.is_empty());
        assert!(!agent.content.is_empty());
        assert!(agent.description.is_some());
    }

    let agent1 = agents.iter().find(|a| a.name == "valid-agent-1").unwrap();
    assert_eq!(agent1.description, Some("Valid agent 1".to_string()));

    let agent2 = agents.iter().find(|a| a.name == "valid-agent-2").unwrap();
    assert_eq!(agent2.description, Some("Valid agent 2".to_string()));
}

#[test]
fn test_agent_manager_list_agents_empty_directories() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let empty_dir = temp_dir.path().join("empty_agents");
    std::fs::create_dir_all(&empty_dir).expect("Failed to create empty dir");

    let result = ModelManager::load_models_from_dir(&empty_dir, ModelConfigSource::Project);
    assert!(result.is_ok(), "Should handle empty directory gracefully");

    let agents = result.unwrap();
    assert!(
        agents.is_empty(),
        "Should return empty vector for empty directory"
    );
}

#[test]
fn test_agent_manager_list_agents_non_yaml_files() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create non-YAML files that should be ignored
    fs::write(temp_path.join("not-an-agent.txt"), "This is not an agent")
        .expect("Failed to write txt file");
    fs::write(temp_path.join("also-not-agent.json"), r#"{"not": "agent"}"#)
        .expect("Failed to write json file");
    fs::write(temp_path.join("README.md"), "# Agent Directory").expect("Failed to write readme");

    // Create one valid YAML agent
    let valid_content = r#"executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;
    fs::write(temp_path.join("real-agent.yaml"), valid_content)
        .expect("Failed to write valid agent");

    let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::User);
    assert!(
        result.is_ok(),
        "Should load agents while ignoring non-YAML files"
    );

    let agents = result.unwrap();
    assert_eq!(agents.len(), 1, "Should load only the YAML file");
    assert_eq!(agents[0].name, "real-agent");
    assert_eq!(agents[0].source, ModelConfigSource::User);
}

#[test]
fn test_agent_manager_find_agent_by_name_existing() {
    let result = ModelManager::find_agent_by_name("qwen-embedding");
    assert!(result.is_ok(), "Should find existing qwen-embedding model");

    let agent = result.unwrap();
    assert_eq!(agent.name, "qwen-embedding");
    assert_eq!(agent.source, ModelConfigSource::Builtin);
    assert!(!agent.content.is_empty());
}

#[test]
fn test_agent_manager_find_agent_by_name_not_found() {
    let result = ModelManager::find_agent_by_name("non-existent-agent");
    assert!(
        result.is_err(),
        "Should return error for non-existent agent"
    );

    match result {
        Err(ModelError::NotFound(name)) => {
            assert_eq!(name, "non-existent-agent");
        }
        _ => panic!("Should return NotFound error"),
    }
}

#[test]
fn test_agent_manager_find_agent_by_name_precedence() {
    // This test will pass the existing agent names from builtin agents
    // Test with known builtin agent
    let result = ModelManager::find_agent_by_name("nomic-embed-code");
    assert!(result.is_ok(), "Should find nomic-embed-code model");

    let agent = result.unwrap();
    assert_eq!(agent.name, "nomic-embed-code");
    // Should be builtin unless overridden by project or user agents
    assert_eq!(agent.source, ModelConfigSource::Builtin);
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_detect_config_file_no_config() {
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let result = ModelManager::detect_config_file(&ModelPaths::sah());
    assert!(
        result.is_none(),
        "Should return None when no config files exist"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_detect_config_file_yaml_exists() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
    let yaml_path = sah_dir.join("sah.yaml");
    fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");

    let result = ModelManager::detect_config_file(&ModelPaths::sah());
    assert!(result.is_some(), "Should find yaml config file");

    let found_path = result.unwrap();
    assert_eq!(
        found_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should find sah.yaml file"
    );
    assert!(
        found_path.ends_with(".sah/sah.yaml"),
        "Should end with .sah/sah.yaml"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_detect_config_file_toml_fallback() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
    let toml_path = sah_dir.join("sah.toml");
    fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

    let result = ModelManager::detect_config_file(&ModelPaths::sah());
    assert!(result.is_some(), "Should find toml config file");

    let found_path = result.unwrap();
    assert_eq!(
        found_path.file_name(),
        Some(std::ffi::OsStr::new("sah.toml")),
        "Should find sah.toml file"
    );
    assert!(
        found_path.ends_with(".sah/sah.toml"),
        "Should end with .sah/sah.toml"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_detect_config_file_yaml_precedence() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    // Create .sah directory with both yaml and toml configs
    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
    let yaml_path = sah_dir.join("sah.yaml");
    let toml_path = sah_dir.join("sah.toml");
    fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");
    fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

    let result = ModelManager::detect_config_file(&ModelPaths::sah());
    assert!(result.is_some(), "Should find config file");

    let found_path = result.unwrap();
    assert_eq!(
        found_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should prefer yaml over toml"
    );
    assert!(
        found_path.ends_with(".sah/sah.yaml"),
        "Should end with .sah/sah.yaml"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_ensure_config_structure_creates_directory() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(
        result.is_ok(),
        "Should successfully create config structure"
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should return path to sah.yaml"
    );
    assert!(
        config_path.ends_with(".sah/sah.yaml"),
        "Should end with .sah/sah.yaml"
    );

    // Check that the directory was created
    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    assert!(sah_dir.exists(), "Should create .sah directory");
    assert!(sah_dir.is_dir(), "Should create directory, not file");
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_ensure_config_structure_existing_directory() {
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    // Pre-create the directory
    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(
        result.is_ok(),
        "Should handle existing directory gracefully"
    );

    let config_path = result.unwrap();
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.yaml")),
        "Should return path to sah.yaml"
    );
    assert!(
        config_path.ends_with(".sah/sah.yaml"),
        "Should end with .sah/sah.yaml"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_agent_manager_ensure_config_structure_with_existing_config() {
    use std::fs;

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    // Create a .git directory to prevent config discovery from walking up to the real repo
    fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
    let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

    // Pre-create directory and existing config file
    let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
    let existing_config = sah_dir.join("sah.toml");
    fs::write(&existing_config, "[existing]\nvalue = true\n")
        .expect("Failed to write existing config");

    let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
    assert!(result.is_ok(), "Should handle existing config gracefully");

    let config_path = result.unwrap();
    // Should return existing toml config path, not create new yaml
    assert_eq!(
        config_path.file_name(),
        Some(std::ffi::OsStr::new("sah.toml")),
        "Should return existing config file"
    );
    assert!(
        config_path.ends_with(".sah/sah.toml"),
        "Should return existing toml config"
    );
}
