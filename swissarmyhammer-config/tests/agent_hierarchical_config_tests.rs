use serde_json::json;
use swissarmyhammer_config::{AgentConfig, AgentExecutorType, LlamaAgentConfig, TemplateContext};

#[test]
fn test_hierarchical_configuration_system_default() {
    let context = TemplateContext::new();

    // System default (Claude Code) when no configuration is set
    let system_default = context.get_agent_config(None);
    assert_eq!(
        system_default.executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert!(!system_default.quiet);
}

#[test]
fn test_hierarchical_configuration_repo_default() {
    let mut context = TemplateContext::new();

    // Set repo default to LlamaAgent
    let llama_config = AgentConfig::llama_agent(LlamaAgentConfig::default());
    context.set(
        "agent.default".to_string(),
        serde_json::to_value(&llama_config).expect("Failed to serialize agent config"),
    );

    // Should use repo default instead of system default
    let repo_default = context.get_agent_config(None);
    assert_eq!(repo_default.executor_type(), AgentExecutorType::LlamaAgent);
    assert!(!repo_default.quiet);

    // Non-existent workflow should also use repo default
    let fallback = context.get_agent_config(Some("nonexistent-workflow"));
    assert_eq!(fallback.executor_type(), AgentExecutorType::LlamaAgent);
}

#[test]
fn test_hierarchical_configuration_workflow_specific() {
    let mut context = TemplateContext::new();

    // Set repo default to LlamaAgent
    let llama_config = AgentConfig::llama_agent(LlamaAgentConfig::default());
    context.set(
        "agent.default".to_string(),
        serde_json::to_value(&llama_config).expect("Failed to serialize llama config"),
    );

    // Set workflow-specific config to Claude Code
    let claude_config = AgentConfig::claude_code();
    context.set(
        "agent.configs.test-workflow".to_string(),
        serde_json::to_value(&claude_config).expect("Failed to serialize claude config"),
    );

    // Workflow-specific should override repo default
    let workflow_config = context.get_agent_config(Some("test-workflow"));
    assert_eq!(
        workflow_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );

    // But repo default should still be used for other workflows
    let repo_config = context.get_agent_config(None);
    assert_eq!(repo_config.executor_type(), AgentExecutorType::LlamaAgent);

    // Non-existent workflow should use repo default
    let fallback_config = context.get_agent_config(Some("other-workflow"));
    assert_eq!(
        fallback_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
}

#[test]
fn test_get_all_agent_configs_empty() {
    let context = TemplateContext::new();
    let configs = context.get_all_agent_configs();
    assert!(configs.is_empty());
}

#[test]
fn test_get_all_agent_configs_with_default() {
    let mut context = TemplateContext::new();

    // Set repo default
    let llama_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
    context.set(
        "agent.default".to_string(),
        serde_json::to_value(&llama_config).expect("Failed to serialize config"),
    );

    let configs = context.get_all_agent_configs();
    assert_eq!(configs.len(), 1);
    assert!(configs.contains_key("default"));

    let default_config = configs.get("default").unwrap();
    assert_eq!(
        default_config.executor_type(),
        AgentExecutorType::LlamaAgent
    );
}

#[test]
fn test_get_all_agent_configs_with_workflows() {
    let mut context = TemplateContext::new();

    // Set repo default
    let llama_config = AgentConfig::llama_agent(LlamaAgentConfig::default());
    context.set(
        "agent.default".to_string(),
        serde_json::to_value(&llama_config).expect("Failed to serialize config"),
    );

    // Set workflow-specific configs
    let claude_config = AgentConfig::claude_code();
    let testing_config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());

    context.set(
        "agent.configs.production".to_string(),
        serde_json::to_value(&claude_config).expect("Failed to serialize claude config"),
    );
    context.set(
        "agent.configs.testing".to_string(),
        serde_json::to_value(&testing_config).expect("Failed to serialize testing config"),
    );

    let configs = context.get_all_agent_configs();
    assert_eq!(configs.len(), 3); // default + 2 workflow configs

    assert!(configs.contains_key("default"));
    assert!(configs.contains_key("production"));
    assert!(configs.contains_key("testing"));

    assert_eq!(
        configs.get("default").unwrap().executor_type(),
        AgentExecutorType::LlamaAgent
    );
    assert_eq!(
        configs.get("production").unwrap().executor_type(),
        AgentExecutorType::ClaudeCode
    );
    assert_eq!(
        configs.get("testing").unwrap().executor_type(),
        AgentExecutorType::LlamaAgent
    );
}

#[test]
fn test_malformed_agent_config_falls_back() {
    let mut context = TemplateContext::new();

    // Set malformed agent config that can't be deserialized
    context.set("agent.default".to_string(), json!("invalid-config"));

    // Should fall back to system default
    let config = context.get_agent_config(None);
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);

    // Should also fall back for workflow-specific malformed config
    context.set("agent.configs.broken".to_string(), json!(42));
    let workflow_config = context.get_agent_config(Some("broken"));
    assert_eq!(
        workflow_config.executor_type(),
        AgentExecutorType::ClaudeCode
    );
}

#[test]
fn test_quiet_mode_configuration() {
    let mut context = TemplateContext::new();

    // Create config with quiet mode enabled
    let mut quiet_config = AgentConfig::claude_code();
    quiet_config.quiet = true;

    context.set(
        "agent.default".to_string(),
        serde_json::to_value(&quiet_config).expect("Failed to serialize config"),
    );

    let config = context.get_agent_config(None);
    assert!(config.quiet);
    assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
}
