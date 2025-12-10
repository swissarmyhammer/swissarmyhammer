use std::{env, fs};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{
    parse_model_config, parse_model_description, ModelConfigSource, ModelError, ModelManager,
};
use swissarmyhammer_config::{AgentExecutorType, LlamaAgentConfig, ModelConfig};

// =============================================================================
// TEST HELPER FUNCTIONS
// =============================================================================

/// Helper to set up a temporary home directory with optional agent files
struct TestEnvironment {
    _env: IsolatedTestEnvironment,
    project_root: std::path::PathBuf,
    original_dir: std::path::PathBuf,
}

impl TestEnvironment {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let project_root = env.temp_dir().join("project");

        fs::create_dir_all(&project_root).expect("Failed to create project root");

        // Initialize as git repository for gitroot model discovery
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&project_root)
            .output()
            .expect("Failed to init git repo");

        let original_dir = env::current_dir().expect("Failed to get current dir");

        Self {
            _env: env,
            project_root,
            original_dir,
        }
    }

    fn activate(&self) {
        env::set_var("HOME", self._env.home_path());
        env::set_current_dir(&self.project_root).expect("Failed to change to project dir");
    }

    fn user_agents_dir(&self) -> std::path::PathBuf {
        self._env.swissarmyhammer_dir().join("models")
    }

    fn project_agents_dir(&self) -> std::path::PathBuf {
        self.project_root.join("models")
    }

    fn gitroot_agents_dir(&self) -> std::path::PathBuf {
        self.project_root.join(".swissarmyhammer").join("models")
    }

    fn create_user_agent(&self, name: &str, content: &str) {
        let dir = self.user_agents_dir();
        fs::create_dir_all(&dir).expect("Failed to create user agents dir");
        fs::write(dir.join(format!("{}.yaml", name)), content).expect("Failed to write user agent");
    }

    fn create_project_agent(&self, name: &str, content: &str) {
        let dir = self.project_agents_dir();
        fs::create_dir_all(&dir).expect("Failed to create project agents dir");
        fs::write(dir.join(format!("{}.yaml", name)), content)
            .expect("Failed to write project agent");
    }

    fn create_gitroot_agent(&self, name: &str, content: &str) {
        let dir = self.gitroot_agents_dir();
        fs::create_dir_all(&dir).expect("Failed to create gitroot agents dir");
        fs::write(dir.join(format!("{}.yaml", name)), content)
            .expect("Failed to write gitroot agent");
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original_dir);
        // IsolatedTestEnvironment handles HOME restoration
    }
}

/// Helper for config file operations
struct ConfigFileHelper {
    project_root: std::path::PathBuf,
    original_dir: std::path::PathBuf,
}

impl ConfigFileHelper {
    fn new(project_root: std::path::PathBuf) -> Self {
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&project_root).expect("Failed to change to project dir");
        Self {
            project_root,
            original_dir,
        }
    }

    fn config_path(&self) -> std::path::PathBuf {
        self.project_root.join(".swissarmyhammer").join("sah.yaml")
    }

    fn write_config(&self, content: &str) {
        let path = self.config_path();
        fs::create_dir_all(path.parent().unwrap()).expect("Failed to create config dir");
        fs::write(&path, content).expect("Failed to write config");
    }

    fn read_config(&self) -> String {
        fs::read_to_string(self.config_path()).expect("Failed to read config")
    }

    fn config_exists(&self) -> bool {
        self.config_path().exists()
    }

    fn config_size(&self) -> u64 {
        fs::metadata(self.config_path()).unwrap().len()
    }
}

impl Drop for ConfigFileHelper {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original_dir);
    }
}

fn assert_agent_has_source(
    agents: &[swissarmyhammer_config::model::ModelInfo],
    name: &str,
    expected_source: ModelConfigSource,
) {
    let agent_map: std::collections::HashMap<_, _> = agents
        .iter()
        .map(|agent| (agent.name.as_str(), agent))
        .collect();

    let agent = agent_map
        .get(name)
        .unwrap_or_else(|| panic!("Should have agent '{}'", name));
    assert_eq!(
        agent.source, expected_source,
        "Agent '{}' should have source {:?}",
        name, expected_source
    );
}

fn assert_agent_description_contains(
    agents: &[swissarmyhammer_config::model::ModelInfo],
    name: &str,
    expected_text: &str,
) {
    let agent_map: std::collections::HashMap<_, _> = agents
        .iter()
        .map(|agent| (agent.name.as_str(), agent))
        .collect();

    let agent = agent_map
        .get(name)
        .unwrap_or_else(|| panic!("Should have agent '{}'", name));
    assert!(
        agent.description.as_ref().unwrap().contains(expected_text),
        "Agent '{}' description should contain '{}'",
        name,
        expected_text
    );
}

fn assert_config_contains_sections(config: &str, sections: &[&str]) {
    for section in sections {
        assert!(
            config.contains(section),
            "Config should contain section: {}",
            section
        );
    }
}

fn assert_not_found_error(result: Result<(), ModelError>, expected_name: &str) {
    assert!(result.is_err(), "Should fail for nonexistent agent");
    match result {
        Err(ModelError::NotFound(name)) => {
            assert_eq!(name, expected_name);
        }
        _ => panic!("Should return NotFound error"),
    }
}

// =============================================================================
// BASIC CONFIGURATION TESTS
// =============================================================================

#[test]
fn test_model_config_system_default_is_claude() {
    let config = ModelConfig::default();
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(!config.quiet);
}

#[test]
fn test_model_config_factories() {
    let claude_config = ModelConfig::claude_code();
    assert_eq!(claude_config.executor_type(), AgentExecutorType::ClaudeCode);

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    assert_eq!(llama_config.executor_type(), AgentExecutorType::LlamaAgent);
}

#[test]
fn test_model_config_serialization() {
    let config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());

    // Should serialize to YAML correctly
    let yaml = serde_yaml::to_string(&config).expect("Failed to serialize to YAML");
    assert!(yaml.contains("type: llama-agent"));
    assert!(yaml.contains("quiet: false"));

    // Should deserialize from YAML correctly
    let deserialized: ModelConfig =
        serde_yaml::from_str(&yaml).expect("Failed to deserialize from YAML");
    assert_eq!(config.executor_type(), deserialized.executor_type());
    assert_eq!(config.quiet, deserialized.quiet);
}

// =============================================================================
// BUILTIN AGENTS TESTS
// =============================================================================

#[test]
fn test_model_manager_load_builtin_models_comprehensive() {
    let agents = ModelManager::load_builtin_agents().expect("Should load builtin agents");

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
        assert_eq!(agent.source, ModelConfigSource::Builtin);
        assert!(!agent.name.is_empty());
        assert!(!agent.content.is_empty());
    }

    // Test that agent configs are valid
    for agent in &agents {
        let parsed_config = parse_model_config(&agent.content);
        assert!(
            parsed_config.is_ok(),
            "Builtin agent '{}' should have valid config",
            agent.name
        );
    }
}

// =============================================================================
// AGENT PRECEDENCE TESTS
// =============================================================================

#[test]
fn test_user_agent_overrides_builtin() {
    let env = TestEnvironment::new();

    let user_claude_override = r#"---
description: "User override of Claude Code"
---
executor:
  type: claude-code
  config:
    claude_path: /user/claude
    args: ["--user-override"]
quiet: true"#;

    env.create_user_agent("claude-code", user_claude_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "claude-code", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "claude-code", "User override");
}

#[test]
fn test_project_agent_overrides_user() {
    let env = TestEnvironment::new();

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

    env.create_project_agent("qwen-coder", project_qwen_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "qwen-coder", ModelConfigSource::Project);
    assert_agent_description_contains(&agents, "qwen-coder", "Project override");
}

#[test]
fn test_custom_agents_from_each_source() {
    let env = TestEnvironment::new();

    let user_custom = r#"---
description: "User custom agent"
---
executor:
  type: claude-code
  config:
    claude_path: /user/custom
    args: ["--custom"]
quiet: false"#;

    let project_specific = r#"---
description: "Project specific agent"
---
executor:
  type: claude-code
  config:
    claude_path: /project/specific
    args: ["--project-mode"]
quiet: true"#;

    env.create_user_agent("user-custom", user_custom);
    env.create_project_agent("project-specific", project_specific);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "user-custom", ModelConfigSource::User);
    assert_agent_has_source(&agents, "project-specific", ModelConfigSource::Project);
}

#[test]
fn test_agent_precedence_verification() {
    let env = TestEnvironment::new();
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "qwen-coder-flash", ModelConfigSource::Builtin);
}

#[test]
fn test_gitroot_agent_loading() {
    let env = TestEnvironment::new();
    env.activate();

    // Verify we're in a git repo
    let git_root = swissarmyhammer_common::utils::directory_utils::find_git_repository_root();
    assert!(git_root.is_some(), "Should be in a git repository");

    // Create gitroot agent
    env.create_gitroot_agent(
        "gitroot-test",
        r#"---
description: "Test gitroot agent"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // Should include gitroot agent
    assert_agent_has_source(&agents, "gitroot-test", ModelConfigSource::GitRoot);
    assert_agent_description_contains(&agents, "gitroot-test", "Test gitroot agent");
}

#[test]
fn test_gitroot_agent_overrides_project() {
    let env = TestEnvironment::new();
    env.activate();

    // Create project agent
    env.create_project_agent(
        "override-test",
        r#"---
description: "Project version"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    // Create gitroot agent with same name (should override project)
    env.create_gitroot_agent(
        "override-test",
        r#"---
description: "GitRoot version"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // GitRoot should override Project
    assert_agent_has_source(&agents, "override-test", ModelConfigSource::GitRoot);
    assert_agent_description_contains(&agents, "override-test", "GitRoot version");
}

#[test]
fn test_user_agent_overrides_gitroot() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agent
    env.create_gitroot_agent(
        "override-test",
        r#"---
description: "GitRoot version"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    // Create user agent with same name (should override gitroot)
    env.create_user_agent(
        "override-test",
        r#"---
description: "User version"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // User should override GitRoot
    assert_agent_has_source(&agents, "override-test", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "override-test", "User version");
}

#[test]
fn test_full_precedence_hierarchy_with_gitroot() {
    let env = TestEnvironment::new();
    env.activate();

    // Override claude-code at each level to test full precedence
    env.create_project_agent(
        "claude-code",
        r#"---
description: "Project claude"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    env.create_gitroot_agent(
        "claude-code",
        r#"---
description: "GitRoot claude"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    env.create_user_agent(
        "claude-code",
        r#"---
description: "User claude"
---
quiet: false
executor:
  type: claude-code
  config: {}
"#,
    );

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // User should win (highest precedence)
    assert_agent_has_source(&agents, "claude-code", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "claude-code", "User claude");
}

// =============================================================================
// CONFIG FILE OPERATIONS TESTS
// =============================================================================

#[test]
fn test_model_manager_config_file_operations() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // Test config structure creation
    let config_path =
        ModelManager::ensure_config_structure().expect("Should create config structure");

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
    ModelManager::use_agent("claude-code").expect("Should use claude-code agent");

    assert!(helper.config_exists(), "Config file should be created");

    let config_content = helper.read_config();
    assert_config_contains_sections(&config_content, &["models:", "root:", "claude-code"]);

    // Test updating existing config
    let original_size = helper.config_size();

    ModelManager::use_agent("qwen-coder").expect("Should update to qwen-coder");

    let updated_content = helper.read_config();
    assert_config_contains_sections(&updated_content, &["models:", "qwen-coder"]);

    let updated_size = helper.config_size();
    assert_ne!(
        original_size, updated_size,
        "Config file should be modified"
    );
}

#[test]
fn test_config_file_sections_preserved() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir.clone());

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
"#;

    helper.write_config(existing_config);

    ModelManager::use_agent("claude-code").expect("Should update agent config");

    let updated_config = helper.read_config();

    assert_config_contains_sections(
        &updated_config,
        &[
            "prompt:",
            "default_template",
            "workflows:",
            "test-workflow",
            "other_section:",
            "preserved_value",
            "nested:",
        ],
    );
}

#[test]
fn test_config_models_section_updated() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir.clone());

    let existing_config = r#"existing_agent:
  old_config: "will be replaced"
"#;

    helper.write_config(existing_config);

    ModelManager::use_agent("claude-code").expect("Should update agent config");

    let updated_config = helper.read_config();

    assert_config_contains_sections(&updated_config, &["models:", "root:"]);
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[test]
fn test_model_manager_error_handling_comprehensive() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir.clone());

    // Test agent not found error
    let result = ModelManager::use_agent("definitely-nonexistent-agent-name-12345");
    assert_not_found_error(result, "definitely-nonexistent-agent-name-12345");

    // Test find_agent_by_name with nonexistent
    let find_result = ModelManager::find_agent_by_name("another-nonexistent-agent");
    assert!(
        find_result.is_err(),
        "Should fail to find nonexistent agent"
    );

    match find_result {
        Err(ModelError::NotFound(name)) => {
            assert_eq!(name, "another-nonexistent-agent");
        }
        _ => panic!("Should return NotFound error for find"),
    }

    // Test successful agent lookup
    let found_agent = ModelManager::find_agent_by_name("claude-code")
        .expect("Should find builtin claude-code agent");
    assert_eq!(found_agent.name, "claude-code");
    assert_eq!(found_agent.source, ModelConfigSource::Builtin);
}

// =============================================================================
// INVALID FILE HANDLING TESTS
// =============================================================================

#[test]
fn test_valid_agent_loads_successfully() {
    let env = TestEnvironment::new();

    let valid_agent = r#"---
description: "Valid test agent"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: false"#;

    env.create_user_agent("valid-agent", valid_agent);
    env.activate();

    let user_agents =
        ModelManager::load_user_models().expect("Should load user agents despite invalid files");

    assert_eq!(user_agents.len(), 1, "Should load only valid agent");
    assert_eq!(user_agents[0].name, "valid-agent");
    assert_eq!(user_agents[0].source, ModelConfigSource::User);
    assert!(user_agents[0]
        .description
        .as_ref()
        .unwrap()
        .contains("Valid test agent"));
}

#[test]
fn test_invalid_yaml_syntax_ignored() {
    let env = TestEnvironment::new();

    let invalid_yaml = "invalid: yaml: content: [unclosed bracket";
    let dir = env.user_agents_dir();
    fs::create_dir_all(&dir).expect("Failed to create user agents dir");
    fs::write(dir.join("invalid-syntax.yaml"), invalid_yaml).expect("Failed to write invalid yaml");

    env.activate();

    let user_agents = ModelManager::load_user_models().expect("Should load despite invalid files");

    let agent_names: Vec<_> = user_agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        !agent_names.contains(&"invalid-syntax"),
        "Should not load invalid syntax"
    );
}

#[test]
fn test_invalid_config_structure_ignored() {
    let env = TestEnvironment::new();

    let invalid_config = r#"---
description: "Invalid agent config"
---
executor:
  type: unknown-executor-type
  config: "not an object"
invalid_field: true"#;

    let dir = env.user_agents_dir();
    fs::create_dir_all(&dir).expect("Failed to create user agents dir");
    fs::write(dir.join("invalid-config.yaml"), invalid_config)
        .expect("Failed to write invalid config");

    env.activate();

    let user_agents = ModelManager::load_user_models().expect("Should load despite invalid files");

    let agent_names: Vec<_> = user_agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        !agent_names.contains(&"invalid-config"),
        "Should not load invalid config"
    );
}

#[test]
fn test_non_yaml_files_ignored() {
    let env = TestEnvironment::new();

    let dir = env.user_agents_dir();
    fs::create_dir_all(&dir).expect("Failed to create user agents dir");
    fs::write(dir.join("not-agent.txt"), "This is not an agent file")
        .expect("Failed to write non-yaml file");

    env.activate();

    let user_agents = ModelManager::load_user_models().expect("Should load despite invalid files");

    let agent_names: Vec<_> = user_agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        !agent_names.contains(&"not-agent"),
        "Should not load non-yaml file"
    );
}

// =============================================================================
// DESCRIPTION PARSING TESTS
// =============================================================================

#[test]
fn test_model_description_parsing_comprehensive() {
    let test_cases = vec![
        (
            "yaml_frontmatter",
            r#"---
description: "YAML frontmatter description"
version: "1.0"
author: "Test Author"
---
executor:
  type: claude-code
  config: {}
quiet: false"#,
            Some("YAML frontmatter description".to_string()),
        ),
        (
            "comment_based",
            r#"# Description: Comment-based description
# Additional comment
executor:
  type: claude-code
  config: {}
quiet: false"#,
            Some("Comment-based description".to_string()),
        ),
        (
            "yaml_precedence",
            r#"---
description: "YAML takes precedence"
---
# Description: This should be ignored
executor:
  type: claude-code
  config: {}
quiet: false"#,
            Some("YAML takes precedence".to_string()),
        ),
        (
            "no_description",
            r#"executor:
  type: claude-code
  config: {}
quiet: false"#,
            None,
        ),
        (
            "empty_description",
            r#"---
description: ""
---
executor:
  type: claude-code
  config: {}
quiet: false"#,
            Some("".to_string()),
        ),
        (
            "whitespace_trimmed",
            r#"---
description: "  Trimmed description  "
---
executor:
  type: claude-code
  config: {}
quiet: false"#,
            Some("Trimmed description".to_string()),
        ),
    ];

    for (test_name, content, expected) in test_cases {
        let description = parse_model_description(content);
        assert_eq!(description, expected, "Test case '{}' failed", test_name);
    }
}

// =============================================================================
// CONFIG PARSING TESTS
// =============================================================================

#[test]
fn test_model_config_parsing_comprehensive() {
    struct ParseTestCase {
        name: &'static str,
        content: &'static str,
        expected_type: AgentExecutorType,
        expected_quiet: bool,
    }

    let test_cases = vec![
        ParseTestCase {
            name: "with_frontmatter",
            content: r#"---
description: "Test agent with frontmatter"
version: "1.0"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: true"#,
            expected_type: AgentExecutorType::ClaudeCode,
            expected_quiet: true,
        },
        ParseTestCase {
            name: "pure_config",
            content: r#"executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "test/model"
        filename: "test.gguf"
    mcp_server:
      port: 0
      timeout_seconds: 30
quiet: false"#,
            expected_type: AgentExecutorType::LlamaAgent,
            expected_quiet: false,
        },
        ParseTestCase {
            name: "with_comments",
            content: r#"# Description: Test agent with comments
# Version: 1.0
executor:
  type: claude-code
  config: {}
quiet: false"#,
            expected_type: AgentExecutorType::ClaudeCode,
            expected_quiet: false,
        },
    ];

    for test_case in test_cases {
        let config = parse_model_config(test_case.content)
            .unwrap_or_else(|_| panic!("Should parse config for test '{}'", test_case.name));
        assert_eq!(
            config.executor_type(),
            test_case.expected_type,
            "Test '{}' executor type mismatch",
            test_case.name
        );
        assert_eq!(
            config.quiet, test_case.expected_quiet,
            "Test '{}' quiet flag mismatch",
            test_case.name
        );
    }
}

#[test]
fn test_parse_invalid_config() {
    let invalid_content = "invalid yaml content [unclosed";
    let result = parse_model_config(invalid_content);
    assert!(result.is_err(), "Should fail to parse invalid YAML");
}

// =============================================================================
// DIRECTORY LOADING EDGE CASES
// =============================================================================

#[test]
fn test_model_manager_directory_loading_edge_cases() {
    struct DirTestCase {
        name: &'static str,
        setup: Box<dyn Fn(&std::path::PathBuf)>,
        expected_count: usize,
    }

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    let test_cases = vec![
        DirTestCase {
            name: "nonexistent_directory",
            setup: Box::new(|_| {}),
            expected_count: 0,
        },
        DirTestCase {
            name: "empty_directory",
            setup: Box::new(|dir| {
                fs::create_dir_all(dir).expect("Failed to create dir");
            }),
            expected_count: 0,
        },
        DirTestCase {
            name: "only_non_yaml_files",
            setup: Box::new(|dir| {
                fs::create_dir_all(dir).expect("Failed to create dir");
                fs::write(dir.join("readme.txt"), "Not an agent").expect("Failed to write file");
                fs::write(dir.join("config.json"), r#"{"not": "agent"}"#)
                    .expect("Failed to write json");
            }),
            expected_count: 0,
        },
    ];

    for test_case in test_cases {
        let test_dir = temp_dir.join(test_case.name);
        (test_case.setup)(&test_dir);

        let result = ModelManager::load_agents_from_dir(&test_dir, ModelConfigSource::User);
        assert!(
            result.is_ok(),
            "Test '{}' should handle directory gracefully",
            test_case.name
        );
        assert_eq!(
            result.unwrap().len(),
            test_case.expected_count,
            "Test '{}' expected count mismatch",
            test_case.name
        );
    }
}
