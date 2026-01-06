//! Tests that prove the Rules use case agent configuration is read correctly.

use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{AgentUseCase, ModelManager};

/// Test proving config is read correctly (fast, no model needed)
#[tokio::test]
async fn test_config_specifies_rules_model() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_path = temp_dir;
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let config_path = sah_dir.join("sah.yaml");

    // Test 1: qwen-next
    fs::write(&config_path, "agents:\n  rules: qwen-next\n").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    let agent1 = ModelManager::get_agent_for_use_case(AgentUseCase::Rules).expect("Should read");
    eprintln!("Test 1 - Agent: {:?}", agent1);
    assert_eq!(agent1, Some("qwen-next".to_string()));

    // Test 2: claude-code
    fs::write(&config_path, "agents:\n  rules: claude-code\n").unwrap();
    let agent2 = ModelManager::get_agent_for_use_case(AgentUseCase::Rules).expect("Should read");
    eprintln!("Test 2 - Agent: {:?}", agent2);
    assert_eq!(agent2, Some("claude-code".to_string()));

    // Test 3: No config (should return None)
    fs::write(&config_path, "# empty\n").unwrap();
    let agent3 = ModelManager::get_agent_for_use_case(AgentUseCase::Rules).expect("Should read");
    eprintln!("Test 3 - Agent: {:?}", agent3);
    assert_eq!(agent3, None);

    eprintln!("\n✓ Config reader correctly reads different agent names");
    eprintln!("✓ This proves the config mechanism works");
    eprintln!("✗ The bug is that RuleCheckTool caches and doesn't re-read config");

    std::env::set_current_dir(&original_dir).unwrap();
}
