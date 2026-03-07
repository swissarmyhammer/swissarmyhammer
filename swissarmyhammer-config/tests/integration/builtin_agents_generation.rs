use swissarmyhammer_config::get_builtin_models;

#[test]
fn test_builtin_models_generation() {
    let agents = get_builtin_models();

    // Extract agent names
    let names: Vec<&str> = agents.iter().map(|(name, _)| *name).collect();

    // Should contain all expected agents
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"qwen-coder"));
    assert!(names.contains(&"qwen-embedding"));

    // Verify each agent has valid YAML content with executor(s) key
    for (name, content) in agents {
        assert!(!name.is_empty(), "Agent name should not be empty");
        assert!(!content.is_empty(), "Agent content should not be empty");
        assert!(
            content.contains("executor:") || content.contains("executors:"),
            "Agent content should contain 'executor:' or 'executors:' key for {}",
            name
        );
    }
}

#[test]
fn test_builtin_models_specific_content() {
    let agents = get_builtin_models();
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
    assert!(
        qwen_content.contains("unsloth/Qwen3"),
        "Expected Qwen3 model in qwen-coder"
    );

    // Test qwen-embedding has multi-executor format with ANE + llama fallback
    let embed_content = agents_map
        .get("qwen-embedding")
        .expect("qwen-embedding agent should exist");
    assert!(
        embed_content.contains("executors:"),
        "qwen-embedding should use multi-executor format"
    );
    assert!(embed_content.contains("type: ane-embedding"));
    assert!(embed_content.contains("type: llama-embedding"));
    assert!(embed_content.contains("macos-arm64"));
}

#[test]
fn test_builtin_models_parseable() {
    let agents = get_builtin_models();
    for (name, content) in agents {
        let result = swissarmyhammer_config::parse_model_config(content);
        assert!(
            result.is_ok(),
            "Builtin model '{}' should parse successfully: {:?}",
            name,
            result.err()
        );
    }
}
