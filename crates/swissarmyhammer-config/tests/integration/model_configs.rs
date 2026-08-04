use std::{env, fs};
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_config::model::{
    parse_model_config, parse_model_description, ModelConfigSource, ModelManager,
};
use swissarmyhammer_config::ModelExecutorType;

// =============================================================================
// TEST CONSTANTS
// =============================================================================

// =============================================================================
// TEST HELPER FUNCTIONS
// =============================================================================

/// Generate a test embedding model YAML configuration with optional description
fn test_agent_yaml(description: &str, agent_type: &str, custom_config: Option<&str>) -> String {
    let config_section = match agent_type {
        "llama-embedding" | "ane-embedding" => custom_config.map_or_else(
            || {
                r#"config:
    source: !HuggingFace
      repo: "test/model"
      filename: "test.gguf"
    normalize: true"#
                    .to_string()
            },
            str::to_string,
        ),
        _ => panic!("Unsupported executor type: {}", agent_type),
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
    _dir_guard: CurrentDirGuard,
}

impl TestEnvironment {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let project_root = env.temp_dir().join("project");

        fs::create_dir_all(&project_root).expect("Failed to create project root");

        // Create .git marker to prevent config discovery from walking up to real repo
        fs::create_dir(env.temp_dir().join(".git")).expect("Failed to create .git marker");

        // Initialize as git repository for gitroot model discovery
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&project_root)
            .output()
            .expect("Failed to init git repo");

        let dir_guard = CurrentDirGuard::new(&project_root).expect("Failed to set current dir");

        Self {
            _env: env,
            project_root,
            _dir_guard: dir_guard,
        }
    }

    fn activate(&self) {
        env::set_var("HOME", self._env.home_path());
        env::set_current_dir(&self.project_root).expect("Failed to change to project dir");
    }

    fn user_agents_dir(&self) -> std::path::PathBuf {
        self._env.home_path().join(".models")
    }

    fn project_agents_dir(&self) -> std::path::PathBuf {
        self.project_root.join("models")
    }

    fn gitroot_agents_dir(&self) -> std::path::PathBuf {
        self.project_root.join("models")
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
        // CurrentDirGuard automatically restores the original directory
        // IsolatedTestEnvironment handles HOME restoration
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

// =============================================================================
// BASIC CONFIGURATION TESTS
// =============================================================================

// =============================================================================
// BUILTIN AGENTS TESTS
// =============================================================================

#[serial_test::serial(cwd)]
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

    // Verify the builtin embedding models are present
    let agent_names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(
        agent_names.contains(&"qwen-embedding"),
        "Should have qwen-embedding as a builtin embedding model"
    );
}

// =============================================================================
// AGENT PRECEDENCE TESTS
// =============================================================================

#[serial_test::serial(cwd)]
#[test]
fn test_user_agent_overrides_builtin() {
    let env = TestEnvironment::new();

    let user_override = test_agent_yaml(
        "User override of the builtin embedding model",
        "llama-embedding",
        Some(
            r#"config:
    source: !HuggingFace
      repo: "user/override"
    normalize: true"#,
        ),
    );

    env.create_user_agent("qwen-embedding", &user_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "qwen-embedding", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "qwen-embedding", "User override");
}

#[serial_test::serial(cwd)]
#[test]
fn test_project_agent_overrides_user() {
    let env = TestEnvironment::new();

    let project_override = test_agent_yaml(
        "Project override of Nomic Embed Code",
        "ane-embedding",
        Some(
            r#"config:
    source: !HuggingFace
      repo: "project/custom-embed"
      filename: "model.gguf"
    normalize: true"#,
        ),
    );

    env.create_project_agent("nomic-embed-code", &project_override);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    // When cwd == git root, both Project and GitRoot scan the same models/ dir;
    // GitRoot runs after Project in the merge order and overwrites, so source is GitRoot.
    assert_agent_has_source(&agents, "nomic-embed-code", ModelConfigSource::GitRoot);
    assert_agent_description_contains(&agents, "nomic-embed-code", "Project override");
}

#[serial_test::serial(cwd)]
#[test]
fn test_custom_agents_from_each_source() {
    let env = TestEnvironment::new();

    let user_custom = test_agent_yaml(
        "User custom agent",
        "llama-embedding",
        Some(
            r#"config:
    source: !HuggingFace
      repo: "user/custom"
    normalize: true"#,
        ),
    );

    let project_specific = test_agent_yaml(
        "Project specific agent",
        "llama-embedding",
        Some(
            r#"config:
    source: !HuggingFace
      repo: "project/specific"
    normalize: true"#,
        ),
    );

    env.create_user_agent("user-custom", &user_custom);
    env.create_project_agent("project-specific", &project_specific);
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "user-custom", ModelConfigSource::User);
    // When cwd == git root, both Project and GitRoot scan the same models/ dir;
    // GitRoot runs after Project in the merge order and overwrites, so source is GitRoot.
    assert_agent_has_source(&agents, "project-specific", ModelConfigSource::GitRoot);
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_precedence_verification() {
    let env = TestEnvironment::new();
    env.activate();

    let agents = ModelManager::list_agents().expect("Should list all agents with precedence");

    assert_agent_has_source(&agents, "qwen-embedding", ModelConfigSource::Builtin);
}

#[serial_test::serial(cwd)]
#[test]
fn test_gitroot_agent_loading() {
    let env = TestEnvironment::new();
    env.activate();

    // Verify we're in a git repo
    let git_root = swissarmyhammer_common::utils::directory_utils::find_git_repository_root();
    assert!(git_root.is_some(), "Should be in a git repository");

    // Create gitroot agent
    let gitroot_agent = test_agent_yaml("Test gitroot agent", "llama-embedding", None);
    env.create_gitroot_agent("gitroot-test", &gitroot_agent);

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // Should include gitroot agent
    assert_agent_has_source(&agents, "gitroot-test", ModelConfigSource::GitRoot);
    assert_agent_description_contains(&agents, "gitroot-test", "Test gitroot agent");
}

#[serial_test::serial(cwd)]
#[test]
fn test_gitroot_agent_overrides_project() {
    let env = TestEnvironment::new();
    env.activate();

    // Create project agent
    let project_agent = test_agent_yaml("Project version", "llama-embedding", None);
    env.create_project_agent("override-test", &project_agent);

    // Create gitroot agent with same name (should override project)
    let gitroot_agent = test_agent_yaml("GitRoot version", "llama-embedding", None);
    env.create_gitroot_agent("override-test", &gitroot_agent);

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // GitRoot should override Project
    assert_agent_has_source(&agents, "override-test", ModelConfigSource::GitRoot);
    assert_agent_description_contains(&agents, "override-test", "GitRoot version");
}

#[serial_test::serial(cwd)]
#[test]
fn test_user_agent_overrides_gitroot() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agent
    let gitroot_agent = test_agent_yaml("GitRoot version", "llama-embedding", None);
    env.create_gitroot_agent("override-test", &gitroot_agent);

    // Create user agent with same name (should override gitroot)
    let user_agent = test_agent_yaml("User version", "llama-embedding", None);
    env.create_user_agent("override-test", &user_agent);

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // User should override GitRoot
    assert_agent_has_source(&agents, "override-test", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "override-test", "User version");
}

#[serial_test::serial(cwd)]
#[test]
fn test_full_precedence_hierarchy_with_gitroot() {
    let env = TestEnvironment::new();
    env.activate();

    // Override qwen-embedding at each level to test full precedence
    let project_agent = test_agent_yaml("Project model", "llama-embedding", None);
    env.create_project_agent("qwen-embedding", &project_agent);

    let gitroot_agent = test_agent_yaml("GitRoot model", "llama-embedding", None);
    env.create_gitroot_agent("qwen-embedding", &gitroot_agent);

    let user_agent = test_agent_yaml("User model", "llama-embedding", None);
    env.create_user_agent("qwen-embedding", &user_agent);

    let agents = ModelManager::list_agents().expect("Should list all agents");

    // User should win (highest precedence)
    assert_agent_has_source(&agents, "qwen-embedding", ModelConfigSource::User);
    assert_agent_description_contains(&agents, "qwen-embedding", "User model");
}

// =============================================================================
// AGENTS MAP AND USE CASE TESTS
// =============================================================================

// =============================================================================
// INVALID FILE HANDLING TESTS
// =============================================================================

#[serial_test::serial(cwd)]
#[test]
fn test_valid_agent_loads_successfully() {
    let env = TestEnvironment::new();

    let valid_agent = test_agent_yaml("Valid test agent", "llama-embedding", None);
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

#[serial_test::serial(cwd)]
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

#[serial_test::serial(cwd)]
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

#[serial_test::serial(cwd)]
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

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_yaml_frontmatter() {
    let content = r#"---
description: "YAML frontmatter description"
version: "1.0"
author: "Test Author"
---
executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(
        description,
        Some("YAML frontmatter description".to_string())
    );
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_comment_based() {
    let content = r#"# Description: Comment-based description
# Additional comment
executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("Comment-based description".to_string()));
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_yaml_precedence() {
    let content = r#"---
description: "YAML takes precedence"
---
# Description: This should be ignored
executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("YAML takes precedence".to_string()));
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_no_description() {
    let content = r#"executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, None);
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_empty_description() {
    let content = r#"---
description: ""
---
executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("".to_string()));
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_description_whitespace_trimmed() {
    let content = r#"---
description: "  Trimmed description  "
---
executor:
  type: llama-embedding
  config: {}
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("Trimmed description".to_string()));
}

// =============================================================================
// CONFIG PARSING TESTS
// =============================================================================

#[serial_test::serial(cwd)]
#[test]
fn test_agent_config_parsing_with_frontmatter() {
    let content = r#"---
description: "Test agent with frontmatter"
version: "1.0"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: "test/model"
    normalize: true
quiet: true"#;

    let config = parse_model_config(content).expect("Should parse config with frontmatter");
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::LlamaEmbedding
    );
    assert!(config.quiet);
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_config_parsing_pure_config() {
    let content = r#"executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: "test/model"
      filename: "test.gguf"
    normalize: true
    max_sequence_length: 256
quiet: false"#;

    let config = parse_model_config(content).expect("Should parse pure config");
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::AneEmbedding
    );
    assert!(!config.quiet);
}

#[serial_test::serial(cwd)]
#[test]
fn test_agent_config_parsing_with_comments() {
    let content = r#"# Description: Test agent with comments
# Version: 1.0
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: "test/model"
quiet: false"#;

    let config = parse_model_config(content).expect("Should parse config with comments");
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::LlamaEmbedding
    );
    assert!(!config.quiet);
}

#[serial_test::serial(cwd)]
#[test]
fn test_parse_invalid_config() {
    let invalid_content = "invalid yaml content [unclosed";
    let result = parse_model_config(invalid_content);
    assert!(result.is_err(), "Should fail to parse invalid YAML");
}

// =============================================================================
// DIRECTORY LOADING EDGE CASES
// =============================================================================

#[serial_test::serial(cwd)]
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
// LOAD_GITROOT_MODELS TESTS
// =============================================================================

#[serial_test::serial(cwd)]
#[test]
fn test_load_gitroot_models_in_git_repo_with_agents() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agents
    let agent1 = test_agent_yaml("First gitroot agent", "llama-embedding", None);
    let agent2 = test_agent_yaml("Second gitroot agent", "ane-embedding", None);
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

#[serial_test::serial(cwd)]
#[test]
fn test_load_gitroot_models_in_git_repo_without_models_dir() {
    let env = TestEnvironment::new();
    env.activate();

    // Don't create models directory
    let gitroot_models = ModelManager::load_gitroot_models()
        .expect("Should return empty vec when models dir doesn't exist");

    assert_eq!(
        gitroot_models.len(),
        0,
        "Should return empty vec when models dir doesn't exist"
    );
}

#[serial_test::serial(cwd)]
#[test]
fn test_load_gitroot_models_in_git_repo_with_empty_models_dir() {
    let env = TestEnvironment::new();
    env.activate();

    // Create empty models directory
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

#[serial_test::serial(cwd)]
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

#[serial_test::serial(cwd)]
#[test]
fn test_load_gitroot_models_with_invalid_agents() {
    let env = TestEnvironment::new();
    env.activate();

    // Create valid and invalid gitroot agents
    let valid_agent = test_agent_yaml("Valid gitroot agent", "llama-embedding", None);
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

#[serial_test::serial(cwd)]
#[test]
fn test_load_gitroot_models_with_non_yaml_files() {
    let env = TestEnvironment::new();
    env.activate();

    // Create gitroot agent and non-yaml files
    let agent = test_agent_yaml("Gitroot agent", "llama-embedding", None);

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

#[serial_test::serial(cwd)]
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
    let agent = test_agent_yaml("Gitroot agent from subdir", "llama-embedding", None);
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
