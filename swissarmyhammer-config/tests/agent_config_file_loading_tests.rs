// sah rule ignore test_rule_with_allow
use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::{AgentExecutorType, TemplateContext};

/// RAII guard for temporary test directory that automatically restores the original directory
struct TempTestDir {
    #[allow(dead_code)] // Must keep temp_dir alive to prevent cleanup during test
    _env: IsolatedTestEnvironment,
    config_dir: PathBuf,
    original_dir: PathBuf,
}

impl TempTestDir {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().unwrap();
        let config_dir = env.temp_dir().join(".swissarmyhammer");
        fs::create_dir(&config_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&env.temp_dir()).unwrap();

        Self {
            _env: env,
            config_dir,
            original_dir,
        }
    }

    fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original_dir);
    }
}

fn assert_default_toml_config(context: &TemplateContext) {
    let repo_default_config = context.get_agent_config(None);
    assert_eq!(
        repo_default_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert!(!repo_default_config.quiet);
}

fn assert_testing_toml_config(context: &TemplateContext) {
    let testing_config = context.get_agent_config(Some("testing"));
    assert_eq!(
        testing_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
    assert!(testing_config.quiet);
}

fn assert_production_toml_config(context: &TemplateContext) {
    let production_config = context.get_agent_config(Some("production"));
    assert_eq!(
        production_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert!(!production_config.quiet);
}

fn assert_all_toml_configs(context: &TemplateContext) {
    let all_configs = context.get_all_agent_configs();
    assert_eq!(all_configs.len(), 3);
    assert!(all_configs.contains_key("default"));
    assert!(all_configs.contains_key("testing"));
    assert!(all_configs.contains_key("production"));
}

#[test]
#[serial]
fn test_load_model_config_from_toml_file() {
    let test_dir = TempTestDir::new();

    let config_file = test_dir.config_dir().join("sah.toml");
    fs::write(
        &config_file,
        r#"
[agent.default]
executor = { type = "claude-code", config = { claude_path = "/usr/local/bin/claude", args = ["--verbose"] } }
quiet = false

[agent.configs.testing]
executor = { type = "llama-agent", config = { model = { source = { HuggingFace = { repo = "unsloth/Qwen3-1.7B-GGUF", filename = "Qwen3-1.7B-UD-Q6_K_XL.gguf" } } }, mcp_server = { port = 0, timeout_seconds = 10 } } }
quiet = true

[agent.configs.production]
executor = { type = "claude-code", config = { args = [] } }
quiet = false
        "#,
    ).unwrap();

    let context = TemplateContext::load_for_cli().unwrap();

    assert_default_toml_config(&context);
    assert_testing_toml_config(&context);
    assert_production_toml_config(&context);
    assert_all_toml_configs(&context);
}

fn assert_default_yaml_config(context: &TemplateContext) {
    let default_config = context.get_agent_config(None);
    assert_eq!(
        default_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
    assert!(!default_config.quiet);
}

fn assert_quick_test_yaml_config(context: &TemplateContext) {
    let quick_config = context.get_agent_config(Some("quick-test"));
    assert_eq!(quick_config.executor_type(), AgentExecutorType::LlamaAgent);
    assert!(quick_config.quiet);
}

fn assert_deploy_yaml_config(context: &TemplateContext) {
    let deploy_config = context.get_agent_config(Some("deploy"));
    assert_eq!(deploy_config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(!deploy_config.quiet);
}

fn assert_fallback_yaml_config(context: &TemplateContext) {
    let fallback_config = context.get_agent_config(Some("nonexistent"));
    assert_eq!(
        fallback_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
}

#[test]
#[serial]
fn test_load_model_config_from_yaml_file() {
    let test_dir = TempTestDir::new();

    let config_file = test_dir.config_dir().join("sah.yaml");
    fs::write(
        &config_file,
        r#"
agent:
  default:
    executor:
      type: llama-agent
      config:
        model:
          source:
            HuggingFace:
              repo: "unsloth/Qwen3-30B-A3B-Instruct-2507-GGUF"
              folder: "UD-Q6_K_XL"
        mcp_server:
          port: 0
          timeout_seconds: 30
    quiet: false
  configs:
    quick-test:
      executor:
        type: llama-agent
        config:
          model:
            source:
              HuggingFace:
                repo: "unsloth/Qwen3-1.7B-GGUF"
                filename: "Qwen3-1.7B-UD-Q6_K_XL.gguf"
          mcp_server:
            port: 0
            timeout_seconds: 10
      quiet: true
    deploy:
      executor:
        type: claude-code
        config:
          args: []
      quiet: false
        "#,
    )
    .unwrap();

    let context = TemplateContext::load_for_cli().unwrap();

    assert_default_yaml_config(&context);
    assert_quick_test_yaml_config(&context);
    assert_deploy_yaml_config(&context);
    assert_fallback_yaml_config(&context);
}

#[test]
#[serial]
fn test_model_config_with_environment_variables() {
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _guard = ENV_LOCK.lock().unwrap();

    let test_dir = TempTestDir::new();

    let config_file = test_dir.config_dir().join("sah.toml");
    fs::write(
        &config_file,
        r#"
[agent.default]
executor = { type = "claude-code", config = { claude_path = "${CLAUDE_PATH:-/usr/bin/claude}", args = ["${CLAUDE_ARGS:-}"] } }
quiet = "${AGENT_QUIET:-false}"
        "#,
    ).unwrap();

    env::set_var("CLAUDE_PATH", "/custom/path/claude");
    env::set_var("CLAUDE_ARGS", "--debug");
    env::set_var("AGENT_QUIET", "true");

    let context = TemplateContext::load_for_cli().unwrap();
    let config = context.get_agent_config(None);

    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);

    env::remove_var("CLAUDE_PATH");
    env::remove_var("CLAUDE_ARGS");
    env::remove_var("AGENT_QUIET");
}
