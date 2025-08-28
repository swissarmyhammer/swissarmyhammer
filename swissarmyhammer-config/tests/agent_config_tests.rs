use swissarmyhammer_config::{AgentConfig, AgentExecutorType, LlamaAgentConfig};

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
