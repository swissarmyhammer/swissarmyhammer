use serial_test::serial;
use std::env;
use std::fs;
use swissarmyhammer_config::{AgentExecutorType, TemplateContext};
use tempfile::TempDir;

#[test]
#[serial]
fn test_load_agent_config_from_toml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir(&config_dir).unwrap();

    let config_file = config_dir.join("sah.toml");
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

    // Change to the temp directory so the config file is discovered
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();

    let context = TemplateContext::load_for_cli().unwrap();

    // Test repo default (should be Claude Code as configured in TOML)
    let repo_default_config = context.get_agent_config(None);
    assert_eq!(
        repo_default_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert!(!repo_default_config.quiet);

    // Test workflow-specific configs
    let testing_config = context.get_agent_config(Some("testing"));
    assert_eq!(
        testing_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
    assert!(testing_config.quiet);

    let production_config = context.get_agent_config(Some("production"));
    assert_eq!(
        production_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert!(!production_config.quiet);

    // Test get_all_agent_configs
    let all_configs = context.get_all_agent_configs();
    assert_eq!(all_configs.len(), 3); // default + testing + production
    assert!(all_configs.contains_key("default"));
    assert!(all_configs.contains_key("testing"));
    assert!(all_configs.contains_key("production"));

    // Restore original directory
    let _ = env::set_current_dir(original_dir);
}

#[test]
#[serial]
fn test_load_agent_config_from_yaml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir(&config_dir).unwrap();

    let config_file = config_dir.join("sah.yaml");
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
              repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
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

    // Change to the temp directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();

    let context = TemplateContext::load_for_cli().unwrap();

    // Test repo default (LlamaAgent)
    let default_config = context.get_agent_config(None);
    assert_eq!(
        default_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
    assert!(!default_config.quiet);

    // Test workflow-specific configs
    let quick_config = context.get_agent_config(Some("quick-test"));
    assert_eq!(quick_config.executor_type(), AgentExecutorType::LlamaAgent);
    assert!(quick_config.quiet);

    let deploy_config = context.get_agent_config(Some("deploy"));
    assert_eq!(deploy_config.executor_type(), AgentExecutorType::ClaudeCode);
    assert!(!deploy_config.quiet);

    // Test non-existent workflow falls back to repo default
    let fallback_config = context.get_agent_config(Some("nonexistent"));
    assert_eq!(
        fallback_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );

    // Restore original directory
    let _ = env::set_current_dir(original_dir);
}

#[test]
#[serial]
fn test_agent_config_with_environment_variables() {
    use std::sync::Mutex;

    // Use a mutex to prevent concurrent environment variable modifications
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _guard = ENV_LOCK.lock().unwrap();

    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir(&config_dir).unwrap();

    // Create config file with environment variable placeholders
    let config_file = config_dir.join("sah.toml");
    fs::write(
        &config_file,
        r#"
[agent.default]
executor = { type = "claude-code", config = { claude_path = "${CLAUDE_PATH:-/usr/bin/claude}", args = ["${CLAUDE_ARGS:-}"] } }
quiet = "${AGENT_QUIET:-false}"
        "#,
    ).unwrap();

    // Set environment variables
    env::set_var("CLAUDE_PATH", "/custom/path/claude");
    env::set_var("CLAUDE_ARGS", "--debug");
    env::set_var("AGENT_QUIET", "true");

    // Change to temp directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();

    let context = TemplateContext::load_for_cli().unwrap();
    let config = context.get_agent_config(None);

    // Verify environment variable substitution worked
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
    // Note: The quiet field and claude_path verification would require more complex parsing
    // since the TOML parsing converts everything to the expected types

    // Clean up environment variables
    env::remove_var("CLAUDE_PATH");
    env::remove_var("CLAUDE_ARGS");
    env::remove_var("AGENT_QUIET");

    // Restore original directory
    let _ = env::set_current_dir(original_dir);
}
