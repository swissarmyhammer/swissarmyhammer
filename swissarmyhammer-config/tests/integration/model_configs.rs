use std::{env, fs};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{
    parse_model_config, parse_model_description, ModelConfigSource, ModelError, ModelManager,
};
use swissarmyhammer_config::{LlamaAgentConfig, ModelConfig, ModelExecutorType};

// =============================================================================
// TEST CONSTANTS
// =============================================================================

/// Port 0 indicates auto-allocation by the operating system
const MCP_SERVER_PORT_AUTO: u16 = 0;

/// Default timeout in seconds for MCP server operations
const MCP_SERVER_TIMEOUT_SECONDS: u64 = 30;

/// Test data value used in nested configuration examples
const TEST_DATA_VALUE: i32 = 42;

// =============================================================================
// TEST HELPER FUNCTIONS
// =============================================================================

/// Generate a test agent YAML configuration with optional description
fn test_agent_yaml(description: &str, agent_type: &str, custom_config: Option<&str>) -> String {
    let config_section = match agent_type {
        "claude-code" => {
            if let Some(config) = custom_config {
                config.to_string()
            } else {
                r#"config:
    claude_path: /test/claude
    args: ["--test"]"#
                    .to_string()
            }
        }
        "llama-agent" => {
            if let Some(config) = custom_config {
                config.to_string()
            } else {
                format!(
                    r#"config:
    model:
      source: !HuggingFace
        repo: "test/model"
        filename: "test.gguf"
    mcp_server:
      port: {}
      timeout_seconds: {}"#,
                    MCP_SERVER_PORT_AUTO, MCP_SERVER_TIMEOUT_SECONDS
                )
            }
        }
        _ => panic!("Unsupported agent type: {}", agent_type),
    };

    format!(
        r#"---
description: "{}"
---
executor:
  type: {}
  {}
quiet: false"#,
        description, agent_type, config_section
    )
}

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

    fn create_agent(&self, name: &str, content: &str, dir: &std::path::Path) {
        fs::create_dir_all(dir).expect("Failed to create agents dir");
        fs::write(dir.join(format!("{}.yaml", name)), content).expect("Failed to write agent");
    }

    fn create_user_agent(&self, name: &str, content: &str) {
        self.create_agent(name, content, &self.user_agents_dir());
    }

    fn create_project_agent(&self, name: &str, content: &str) {
        self.create_agent(name, content, &self.project_agents_dir());
    }

    fn create_gitroot_agent(&self, name: &str, content: &str) {
        self.create_agent(name, content, &self.gitroot_agents_dir());
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

fn find_agent<'a>(
    agents: &'a [swissarmyhammer_config::model::ModelInfo],
    name: &str,
) -> &'a swissarmyhammer_config::model::ModelInfo {
    agents
        .iter()
        .find(|agent| agent.name == name)
        .unwrap_or_else(|| panic!("Should have agent '{}'", name))
}

fn assert_agent_has_source(
    agents: &[swissarmyhammer_config::model::ModelInfo],
    name: &str,
    expected_source: ModelConfigSource,
) {
    let agent = find_agent(agents, name);
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
    let agent = find_agent(agents, name);
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
fn test_agent_config_system_default_is_claude() {
    let config = ModelConfig::default();
    assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
    assert!(!config.quiet);
}

#[test]
fn test_agent_config_factories() {
    let claude_config = ModelConfig::claude_code();
    assert_eq!(claude_config.executor_type(), ModelExecutorType::ClaudeCode);

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    assert_eq!(llama_config.executor_type(), ModelExecutorType::LlamaAgent);
}

#[test]
fn test_agent_config_serialization() {
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
fn test_agent_manager_load_builtin_models_comprehensive() {
    let agents = ModelManager::load_builtin_models().expect("Should load builtin agents");

    // Should have at least some builtin agents
    assert!(!agents.is_empty(), "Should have builtin agents");

    // All should be builtin source
    for agent in &agents {
        assert_eq!(agent.source, ModelConfigSource::Builtin);
        assert!(!agent.name.is_empty(), "Agent name should not be empty");
        assert!(
            !agent.content.is_empty(),
            "Agent content should not be empty"
        );
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

    // Verify at least claude-code exists (the reference implementation)
    let agent_names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"claude-code"),
        "Should have claude-code as reference implementation"
    );
}

// =============================================================================
// AGENT PRECEDENCE TESTS
// =============================================================================

#[test]
fn test_user_agent_overrides_builtin() {
    let env = TestEnvironment::new();

    let user_claude_override = test_agent_yaml(
        "User override of Claude Code",
        "claude-code",
        Some(
            r#"config:
    claude_path: /user/claude
    args: ["--user-override"]"#,
        ),
    );

    env.create_user_agent("claude-code", &user_claude_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "claude-code", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "claude-code", "User override");
}

#[test]
fn test_project_agent_overrides_user() {
    let env = TestEnvironment::new();

    let project_qwen_override = test_agent_yaml(
        "Project override of Qwen Coder",
        "llama-agent",
        Some(
            r#"config:
    model:
      source: !HuggingFace
        repo: "project/custom-qwen"
        filename: "model.gguf"
    mcp_server:
      port: 0
      timeout_seconds: 30"#,
        ),
    );

    env.create_project_agent("qwen-coder", &project_qwen_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "qwen-coder", ModelConfigSource::Project);
    assert_agent_description_contains(&agents, "qwen-coder", "Project override");
}

#[test]
fn test_custom_agents_from_each_source() {
    let env = TestEnvironment::new();

    let user_custom = test_agent_yaml(
        "User custom agent",
        "claude-code",
        Some(
            r#"config:
    claude_path: /user/custom
    args: ["--custom"]"#,
        ),
    );

    let project_specific = test_agent_yaml(
        "Project specific agent",
        "claude-code",
        Some(
            r#"config:
    claude_path: /project/specific
    args: ["--project-mode"]"#,
        ),
    );

    env.create_user_agent("user-custom", &user_custom);
    env.create_project_agent("project-specific", &project_specific);
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

    assert_agent_has_source(&agents, "qwen-next", ModelConfigSource::Builtin);
}

#[test]
fn test_gitroot_agent_loading() {
    let env = TestEnvironment::new();
    env.activate();

    // Verify we're in a git repo
    let git_root = swissarmyhammer_common::utils::directory_utils::find_git_repository_root();
    assert!(git_root.is_some(), "Should be in a git repository");

    // Create gitroot agent
    let gitroot_agent = test_agent_yaml("Test gitroot agent", "claude-code", None);
    env.create_gitroot_agent("gitroot-test", &gitroot_agent);

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
    let project_agent = test_agent_yaml("Project version", "claude-code", None);
    env.create_project_agent("override-test", &project_agent);

    // Create gitroot agent with same name (should override project)
    let gitroot_agent = test_agent_yaml("GitRoot version", "claude-code", None);
    env.create_gitroot_agent("override-test", &gitroot_agent);

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
    let gitroot_agent = test_agent_yaml("GitRoot version", "claude-code", None);
    env.create_gitroot_agent("override-test", &gitroot_agent);

    // Create user agent with same name (should override gitroot)
    let user_agent = test_agent_yaml("User version", "claude-code", None);
    env.create_user_agent("override-test", &user_agent);

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
    let project_agent = test_agent_yaml("Project claude", "claude-code", None);
    env.create_project_agent("claude-code", &project_agent);

    let gitroot_agent = test_agent_yaml("GitRoot claude", "claude-code", None);
    env.create_gitroot_agent("claude-code", &gitroot_agent);

    let user_agent = test_agent_yaml("User claude", "claude-code", None);
    env.create_user_agent("claude-code", &user_agent);

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // User should win (highest precedence)
    assert_agent_has_source(&agents, "claude-code", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "claude-code", "User claude");
}

// =============================================================================
// AGENTS MAP AND USE CASE TESTS
// =============================================================================

#[test]
fn test_agents_map_structure_creation() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // Use agent for root use case
    ModelManager::use_agent("claude-code").expect("Should use claude-code agent");

    assert!(helper.config_exists(), "Config file should be created");

    let config_content = helper.read_config();
    assert!(
        config_content.contains("agents:"),
        "Config should contain agents map"
    );
    assert!(
        config_content.contains("root:"),
        "Config should contain root use case"
    );
    assert!(
        config_content.contains("claude-code"),
        "Config should contain agent name"
    );
}

#[test]
fn test_use_case_to_agent_name_mapping() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Configure different agents for different use cases
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should configure rules agent");
    ModelManager::use_agent_for_use_case(
        "qwen-next",
        swissarmyhammer_config::AgentUseCase::Workflows,
    )
    .expect("Should configure workflows agent");

    // Verify each use case resolves to correct agent
    let root_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get root agent");
    assert_eq!(root_agent, Some("claude-code".to_string()));

    let rules_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Rules)
            .expect("Should get rules agent");
    assert_eq!(rules_agent, Some("qwen-coder".to_string()));

    let workflows_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Workflows)
            .expect("Should get workflows agent");
    assert_eq!(workflows_agent, Some("qwen-next".to_string()));
}

#[test]
fn test_use_case_fallback_to_root() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Configure only root agent
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");

    // Verify rules use case falls back to root
    let rules_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Rules)
            .expect("Should get agent for rules");
    assert_eq!(
        rules_agent,
        Some("claude-code".to_string()),
        "Rules should fall back to root agent"
    );

    // Verify workflows use case falls back to root
    let workflows_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Workflows)
            .expect("Should get agent for workflows");
    assert_eq!(
        workflows_agent,
        Some("claude-code".to_string()),
        "Workflows should fall back to root agent"
    );
}

#[test]
fn test_model_use_case_enum_variants() {
    use swissarmyhammer_config::AgentUseCase;

    // Test all variants exist and convert to strings correctly
    assert_eq!(AgentUseCase::Root.to_string(), "root");
    assert_eq!(AgentUseCase::Rules.to_string(), "rules");
    assert_eq!(AgentUseCase::Workflows.to_string(), "workflows");

    // Test parsing from strings
    assert_eq!("root".parse::<AgentUseCase>().unwrap(), AgentUseCase::Root);
    assert_eq!(
        "rules".parse::<AgentUseCase>().unwrap(),
        AgentUseCase::Rules
    );
    assert_eq!(
        "workflows".parse::<AgentUseCase>().unwrap(),
        AgentUseCase::Workflows
    );

    // Test case insensitivity
    assert_eq!("ROOT".parse::<AgentUseCase>().unwrap(), AgentUseCase::Root);
    assert_eq!(
        "Rules".parse::<AgentUseCase>().unwrap(),
        AgentUseCase::Rules
    );
    assert_eq!(
        "WORKFLOWS".parse::<AgentUseCase>().unwrap(),
        AgentUseCase::Workflows
    );

    // Test invalid variant
    let invalid_result = "invalid".parse::<AgentUseCase>();
    assert!(invalid_result.is_err());
    assert!(invalid_result
        .unwrap_err()
        .contains("Invalid use case: 'invalid'"));
}

#[test]
fn test_resolve_agent_config_for_use_case() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Configure different agents for different use cases
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should configure rules agent");

    // Resolve root use case
    let root_config =
        ModelManager::resolve_agent_config_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should resolve root config");
    assert_eq!(
        root_config.executor_type(),
        swissarmyhammer_config::ModelExecutorType::ClaudeCode
    );

    // Resolve rules use case
    let rules_config = ModelManager::resolve_agent_config_for_use_case(
        swissarmyhammer_config::AgentUseCase::Rules,
    )
    .expect("Should resolve rules config");
    assert_eq!(
        rules_config.executor_type(),
        swissarmyhammer_config::ModelExecutorType::LlamaAgent
    );
}

#[test]
fn test_resolve_agent_config_default_fallback() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Don't configure any agents - should fall back to default claude-code
    let config =
        ModelManager::resolve_agent_config_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should resolve to default");
    assert_eq!(
        config.executor_type(),
        swissarmyhammer_config::ModelExecutorType::ClaudeCode
    );
}

#[test]
fn test_get_agent_for_use_case_no_config() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Should return None when no config exists
    let result = ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should handle no config gracefully");
    assert_eq!(result, None);
}

#[test]
fn test_agents_map_in_config_file_operations() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // Create initial config with root agent
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");

    let initial_config = helper.read_config();
    assert!(initial_config.contains("agents:"), "Should have agents map");
    assert!(initial_config.contains("root: claude-code"));

    // Add rules agent to existing config
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should add rules agent");

    let updated_config = helper.read_config();
    assert!(
        updated_config.contains("agents:"),
        "Should preserve agents map"
    );
    assert!(
        updated_config.contains("root: claude-code"),
        "Should preserve root agent"
    );
    assert!(
        updated_config.contains("rules: qwen-coder"),
        "Should add rules agent"
    );

    // Add workflows agent
    ModelManager::use_agent_for_use_case(
        "qwen-next",
        swissarmyhammer_config::AgentUseCase::Workflows,
    )
    .expect("Should add workflows agent");

    let final_config = helper.read_config();
    assert!(final_config.contains("agents:"));
    assert!(final_config.contains("root: claude-code"));
    assert!(final_config.contains("rules: qwen-coder"));
    assert!(final_config.contains("workflows: qwen-next"));
}

#[test]
fn test_agents_map_preserved_with_other_config_sections() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // Create config with other sections
    let existing_config = r#"# Existing configuration
prompt:
  default_template: "greeting"

workflows:
  - name: "test-workflow"
    description: "Test workflow"

agents:
  root: claude-code
"#;

    helper.write_config(existing_config);

    // Update agents map with new use case
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should add rules agent");

    let updated_config = helper.read_config();

    // Verify all sections are preserved
    assert!(
        updated_config.contains("prompt:"),
        "Should preserve prompt section"
    );
    assert!(
        updated_config.contains("workflows:"),
        "Should preserve workflows section"
    );
    assert!(
        updated_config.contains("agents:"),
        "Should preserve agents section"
    );
    assert!(
        updated_config.contains("root: claude-code"),
        "Should preserve root agent"
    );
    assert!(
        updated_config.contains("rules: qwen-coder"),
        "Should add rules agent"
    );
}

#[test]
fn test_use_agent_updates_root_use_case() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // use_agent should update root use case
    ModelManager::use_agent("claude-code").expect("Should use claude-code");

    let config_content = helper.read_config();
    assert!(config_content.contains("agents:"));
    assert!(config_content.contains("root: claude-code"));

    // Verify via get_agent_for_use_case
    let root_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get root agent");
    assert_eq!(root_agent, Some("claude-code".to_string()));
}

#[test]
fn test_overwrite_existing_use_case_agent() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Set initial root agent
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");

    let initial_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get root agent");
    assert_eq!(initial_agent, Some("claude-code".to_string()));

    // Overwrite with different agent
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should overwrite root agent");

    let updated_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get updated root agent");
    assert_eq!(updated_agent, Some("qwen-coder".to_string()));
}

// =============================================================================
// CONFIG FILE OPERATIONS TESTS
// =============================================================================

#[test]
fn test_agent_manager_config_file_operations() {
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
    assert_config_contains_sections(&config_content, &["agents:", "root:", "claude-code"]);

    // Test updating existing config
    let original_size = helper.config_size();

    ModelManager::use_agent("qwen-coder").expect("Should update to qwen-coder");

    let updated_content = helper.read_config();
    assert_config_contains_sections(&updated_content, &["agents:", "qwen-coder"]);

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

    let existing_config = format!(
        r#"# Existing configuration with comments
prompt:
  default_template: "greeting"

workflows:
  - name: "test-workflow"
    description: "Test workflow"

other_section:
  preserved_value: "should not be lost"
  nested:
    data: {}
"#,
        TEST_DATA_VALUE
    );

    helper.write_config(&existing_config);

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

    assert_config_contains_sections(&updated_config, &["agents:", "root:"]);
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[test]
fn test_agent_manager_error_handling_comprehensive() {
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

    let valid_agent = test_agent_yaml("Valid test agent", "claude-code", None);
    env.create_user_agent("valid-agent", &valid_agent);
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
fn test_agent_description_yaml_frontmatter() {
    let content = r#"---
description: "YAML frontmatter description"
version: "1.0"
author: "Test Author"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(
        description,
        Some("YAML frontmatter description".to_string())
    );
}

#[test]
fn test_agent_description_comment_based() {
    let content = r#"# Description: Comment-based description
# Additional comment
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("Comment-based description".to_string()));
}

#[test]
fn test_agent_description_yaml_precedence() {
    let content = r#"---
description: "YAML takes precedence"
---
# Description: This should be ignored
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("YAML takes precedence".to_string()));
}

#[test]
fn test_agent_description_no_description() {
    let content = r#"executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, None);
}

#[test]
fn test_agent_description_empty_description() {
    let content = r#"---
description: ""
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("".to_string()));
}

#[test]
fn test_agent_description_whitespace_trimmed() {
    let content = r#"---
description: "  Trimmed description  "
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("Trimmed description".to_string()));
}

// =============================================================================
// CONFIG PARSING TESTS
// =============================================================================

#[test]
fn test_agent_config_parsing_with_frontmatter() {
    let content = r#"---
description: "Test agent with frontmatter"
version: "1.0"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: true"#;

    let config = parse_model_config(content).expect("Should parse config with frontmatter");
    assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
    assert!(config.quiet);
}

#[test]
fn test_agent_config_parsing_pure_config() {
    let content = format!(
        r#"executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "test/model"
        filename: "test.gguf"
    mcp_server:
      port: {}
      timeout_seconds: {}
quiet: false"#,
        MCP_SERVER_PORT_AUTO, MCP_SERVER_TIMEOUT_SECONDS
    );

    let config = parse_model_config(&content).expect("Should parse pure config");
    assert_eq!(config.executor_type(), ModelExecutorType::LlamaAgent);
    assert!(!config.quiet);
}

#[test]
fn test_agent_config_parsing_with_comments() {
    let content = r#"# Description: Test agent with comments
# Version: 1.0
executor:
  type: claude-code
  config: {}
quiet: false"#;

    let config = parse_model_config(content).expect("Should parse config with comments");
    assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
    assert!(!config.quiet);
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
fn test_agent_manager_directory_loading_edge_cases() {
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

        let result = ModelManager::load_models_from_dir(&test_dir, ModelConfigSource::User);
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

// =============================================================================
// GLOBAL --AGENT FLAG OVERRIDE TESTS
// =============================================================================

#[test]
fn test_global_agent_flag_override_concept() {
    // This test verifies the concept that a runtime --agent flag can override
    // all use case assignments without modifying the config file.
    //
    // The actual override happens in the CLI/MCP integration layer, but we test
    // the model config system's ability to support this pattern.

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let _helper = ConfigFileHelper::new(temp_dir);

    // Setup: Configure different agents for different use cases
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should configure rules agent");
    ModelManager::use_agent_for_use_case(
        "qwen-next",
        swissarmyhammer_config::AgentUseCase::Workflows,
    )
    .expect("Should configure workflows agent");

    // Verify initial state - each use case has its own agent
    let root_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get root agent");
    assert_eq!(root_agent, Some("claude-code".to_string()));

    let rules_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Rules)
            .expect("Should get rules agent");
    assert_eq!(rules_agent, Some("qwen-coder".to_string()));

    let workflows_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Workflows)
            .expect("Should get workflows agent");
    assert_eq!(workflows_agent, Some("qwen-next".to_string()));

    // The runtime override would happen in the CLI layer by:
    // 1. Reading the --agent flag value
    // 2. Loading that agent's ModelConfig once
    // 3. Using that same config for all use cases without writing to config file
    //
    // This test verifies that we can load any agent's config for runtime override
    let override_agent_info =
        ModelManager::find_agent_by_name("qwen-coder").expect("Should find override agent");
    let override_config = parse_model_config(&override_agent_info.content)
        .expect("Should parse override agent config");

    // Verify the override config is valid and can be used
    assert_eq!(
        override_config.executor_type(),
        swissarmyhammer_config::ModelExecutorType::LlamaAgent
    );

    // Verify that the config file was NOT modified
    // (the override is runtime-only)
    let root_still_claude =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should still get root agent from config");
    assert_eq!(
        root_still_claude,
        Some("claude-code".to_string()),
        "Config file should remain unchanged"
    );
}

#[test]
fn test_global_agent_override_with_nonexistent_agent() {
    // Test that attempting to override with a nonexistent agent fails gracefully
    let result = ModelManager::find_agent_by_name("definitely-nonexistent-agent-12345");

    assert!(result.is_err(), "Should fail for nonexistent agent");
    match result {
        Err(ModelError::NotFound(name)) => {
            assert_eq!(name, "definitely-nonexistent-agent-12345");
        }
        _ => panic!("Should return NotFound error"),
    }
}

#[test]
fn test_global_agent_override_preserves_config_file() {
    // Verify that using an agent for override doesn't modify the config file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let helper = ConfigFileHelper::new(temp_dir);

    // Setup initial config
    ModelManager::use_agent_for_use_case("claude-code", swissarmyhammer_config::AgentUseCase::Root)
        .expect("Should configure root agent");
    ModelManager::use_agent_for_use_case("qwen-coder", swissarmyhammer_config::AgentUseCase::Rules)
        .expect("Should configure rules agent");

    let initial_config = helper.read_config();
    let initial_size = helper.config_size();

    // Simulate what the CLI would do for --agent override:
    // 1. Load the override agent's config
    let override_agent_info =
        ModelManager::find_agent_by_name("qwen-next").expect("Should find override agent");
    let _override_config =
        parse_model_config(&override_agent_info.content).expect("Should parse override config");

    // 2. Use that config for all operations (happens in CLI layer)
    // 3. Verify config file was NOT modified

    let current_config = helper.read_config();
    let current_size = helper.config_size();

    assert_eq!(
        initial_size, current_size,
        "Config file size should not change"
    );
    assert_eq!(
        initial_config, current_config,
        "Config file content should not change"
    );

    // Verify the stored use case assignments are unchanged
    let root_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Root)
            .expect("Should get root agent");
    assert_eq!(
        root_agent,
        Some("claude-code".to_string()),
        "Root agent should still be claude-code"
    );

    let rules_agent =
        ModelManager::get_agent_for_use_case(swissarmyhammer_config::AgentUseCase::Rules)
            .expect("Should get rules agent");
    assert_eq!(
        rules_agent,
        Some("qwen-coder".to_string()),
        "Rules agent should still be qwen-coder"
    );
}

#[test]
fn test_global_agent_override_all_builtin_agents() {
    // Test that all builtin agents can be used as runtime overrides
    let builtin_agents = ModelManager::load_builtin_models().expect("Should load builtin agents");

    assert!(!builtin_agents.is_empty(), "Should have builtin agents");

    // Verify each builtin agent can be loaded and parsed for override
    for agent in builtin_agents {
        let agent_info = ModelManager::find_agent_by_name(&agent.name)
            .unwrap_or_else(|_| panic!("Should find agent '{}'", agent.name));

        let config = parse_model_config(&agent_info.content)
            .unwrap_or_else(|_| panic!("Should parse agent '{}' config", agent.name));

        // Verify the config is valid for runtime use
        match config.executor_type() {
            swissarmyhammer_config::ModelExecutorType::ClaudeCode
            | swissarmyhammer_config::ModelExecutorType::LlamaAgent => {
                // Valid executor type
            }
        }
    }
}

#[test]
fn test_global_agent_override_with_custom_user_agent() {
    // Test that custom user agents can be used as runtime overrides
    let env = TestEnvironment::new();

    let custom_agent = test_agent_yaml(
        "Custom user agent for override",
        "claude-code",
        Some(
            r#"config:
    claude_path: /custom/claude
    args: ["--override-mode"]"#,
        ),
    );

    env.create_user_agent("custom-override", &custom_agent);
    env.activate();

    // Verify the custom agent can be loaded for override
    let agent_info =
        ModelManager::find_agent_by_name("custom-override").expect("Should find custom user agent");

    assert_eq!(agent_info.source, ModelConfigSource::User);

    let config = parse_model_config(&agent_info.content).expect("Should parse custom agent config");

    assert_eq!(
        config.executor_type(),
        swissarmyhammer_config::ModelExecutorType::ClaudeCode
    );
}

// =============================================================================
// LOAD_GITROOT_MODELS TESTS
// =============================================================================

#[test]
fn test_load_gitroot_models_in_git_repo_with_agents() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agents
    let agent1 = test_agent_yaml("First gitroot agent", "claude-code", None);
    let agent2 = test_agent_yaml("Second gitroot agent", "llama-agent", None);
    env.create_gitroot_agent("gitroot-agent-1", &agent1);
    env.create_gitroot_agent("gitroot-agent-2", &agent2);

    let gitroot_models =
        ModelManager::load_gitroot_models().expect("Should load gitroot models in git repository");

    assert_eq!(gitroot_models.len(), 2, "Should load 2 gitroot agents");

    let agent_names: Vec<_> = gitroot_models.iter().map(|a| a.name.as_str()).collect();
    assert!(agent_names.contains(&"gitroot-agent-1"));
    assert!(agent_names.contains(&"gitroot-agent-2"));

    // All should be GitRoot source
    for agent in &gitroot_models {
        assert_eq!(agent.source, ModelConfigSource::GitRoot);
    }
}

#[test]
fn test_load_gitroot_models_in_git_repo_without_models_dir() {
    let env = TestEnvironment::new();
    env.activate();

    // Don't create .swissarmyhammer/models directory
    let gitroot_models = ModelManager::load_gitroot_models()
        .expect("Should return empty vec when models dir doesn't exist");

    assert_eq!(
        gitroot_models.len(),
        0,
        "Should return empty vec when .swissarmyhammer/models doesn't exist"
    );
}

#[test]
fn test_load_gitroot_models_in_git_repo_with_empty_models_dir() {
    let env = TestEnvironment::new();
    env.activate();

    // Create empty .swissarmyhammer/models directory
    let gitroot_dir = env.gitroot_agents_dir();
    fs::create_dir_all(&gitroot_dir).expect("Failed to create gitroot models dir");

    let gitroot_models = ModelManager::load_gitroot_models()
        .expect("Should return empty vec when models dir is empty");

    assert_eq!(
        gitroot_models.len(),
        0,
        "Should return empty vec when models directory is empty"
    );
}

#[test]
fn test_load_gitroot_models_not_in_git_repo() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let non_git_dir = temp_dir.join("non_git_project");
    fs::create_dir_all(&non_git_dir).expect("Failed to create non-git dir");

    // Change to non-git directory
    env::set_current_dir(&non_git_dir).expect("Failed to change to non-git dir");

    let gitroot_models = ModelManager::load_gitroot_models()
        .expect("Should return empty vec when not in git repository");

    assert_eq!(
        gitroot_models.len(),
        0,
        "Should return empty vec when not in a git repository"
    );
}

#[test]
fn test_load_gitroot_models_with_invalid_agents() {
    let env = TestEnvironment::new();
    env.activate();

    // Create valid and invalid gitroot agents
    let valid_agent = test_agent_yaml("Valid gitroot agent", "claude-code", None);
    let invalid_agent = "invalid: yaml: content: [unclosed bracket";

    let gitroot_dir = env.gitroot_agents_dir();
    fs::create_dir_all(&gitroot_dir).expect("Failed to create gitroot models dir");
    fs::write(gitroot_dir.join("valid-agent.yaml"), valid_agent)
        .expect("Failed to write valid agent");
    fs::write(gitroot_dir.join("invalid-agent.yaml"), invalid_agent)
        .expect("Failed to write invalid agent");

    let gitroot_models = ModelManager::load_gitroot_models()
        .expect("Should load valid agents and skip invalid ones");

    assert_eq!(
        gitroot_models.len(),
        1,
        "Should load only valid agent, skipping invalid ones"
    );
    assert_eq!(gitroot_models[0].name, "valid-agent");
    assert_eq!(gitroot_models[0].source, ModelConfigSource::GitRoot);
}

#[test]
fn test_load_gitroot_models_with_non_yaml_files() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agent and non-yaml files
    let agent = test_agent_yaml("Gitroot agent", "claude-code", None);

    let gitroot_dir = env.gitroot_agents_dir();
    fs::create_dir_all(&gitroot_dir).expect("Failed to create gitroot models dir");
    fs::write(gitroot_dir.join("agent.yaml"), agent).expect("Failed to write agent");
    fs::write(gitroot_dir.join("readme.txt"), "Not an agent").expect("Failed to write readme");
    fs::write(gitroot_dir.join("config.json"), r#"{"not": "agent"}"#)
        .expect("Failed to write json");

    let gitroot_models = ModelManager::load_gitroot_models().expect("Should load only yaml files");

    assert_eq!(
        gitroot_models.len(),
        1,
        "Should load only yaml files, ignoring non-yaml files"
    );
    assert_eq!(gitroot_models[0].name, "agent");
}

#[test]
fn test_load_gitroot_models_git_root_detection() {
    let env = TestEnvironment::new();
    env.activate();

    // Verify git root is detected correctly
    let git_root = swissarmyhammer_common::utils::directory_utils::find_git_repository_root();
    assert!(git_root.is_some(), "Should detect git root");

    // Create a subdirectory and change to it
    let subdir = env.project_root.join("src").join("lib");
    fs::create_dir_all(&subdir).expect("Failed to create subdirectory");
    env::set_current_dir(&subdir).expect("Failed to change to subdirectory");

    // Create gitroot agent
    let agent = test_agent_yaml("Gitroot agent from subdir", "claude-code", None);
    env.create_gitroot_agent("subdir-test", &agent);

    // Should still find gitroot models even from subdirectory
    let gitroot_models =
        ModelManager::load_gitroot_models().expect("Should find gitroot models from subdirectory");

    assert_eq!(
        gitroot_models.len(),
        1,
        "Should find gitroot models from subdirectory"
    );
    assert_eq!(gitroot_models[0].name, "subdir-test");
    assert_eq!(gitroot_models[0].source, ModelConfigSource::GitRoot);
}
