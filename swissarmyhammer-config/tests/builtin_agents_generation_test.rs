use swissarmyhammer_config::get_builtin_agents;

#[test]
fn test_builtin_agents_generation() {
    let agents = get_builtin_agents();

    // Extract agent names
    let names: Vec<&str> = agents.iter().map(|(name, _)| *name).collect();

    // Should contain all expected agents
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"qwen-coder"));
    assert!(names.contains(&"qwen-coder-flash"));

    // Verify each agent has valid YAML content
    for (name, content) in agents {
        assert!(!name.is_empty(), "Agent name should not be empty");
        assert!(!content.is_empty(), "Agent content should not be empty");
        assert!(
            content.contains("executor:"),
            "Agent content should contain 'executor:' key for {}",
            name
        );
    }
}

#[test]
fn test_builtin_agents_specific_content() {
    let agents = get_builtin_agents();
    let agents_map: std::collections::HashMap<&str, &str> = agents.into_iter().collect();

    // Test claude-code agent
    let claude_content = agents_map
        .get("claude-code")
        .expect("claude-code agent should exist");
    assert!(claude_content.contains("type: claude-code"));

    // Test qwen-coder agent
    let qwen_content = agents_map
        .get("qwen-coder")
        .expect("qwen-coder agent should exist");
    assert!(qwen_content.contains("type: llama-agent"));
    assert!(qwen_content.contains("unsloth/Qwen3-Coder-480B-A35B-Instruct-GGUF"));

    // Test qwen-coder-flash agent
    let qwen_flash_content = agents_map
        .get("qwen-coder-flash")
        .expect("qwen-coder-flash agent should exist");
    assert!(qwen_flash_content.contains("type: llama-agent"));
    assert!(qwen_flash_content.contains("unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"));
}
