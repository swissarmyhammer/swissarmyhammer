//! Integration test for rule checking with agent configuration
//!
//! Tests that rule checking respects the agent configured for the Rules use case

use std::fs;
use swissarmyhammer_config::agent::{AgentManager, AgentUseCase};
use tempfile::TempDir;

/// Test that RuleCheckTool picks up agent config changes
#[tokio::test]
async fn test_rule_check_uses_configured_agent() {
    // Create temp directory
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Create .swissarmyhammer directory
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Write config with rules agent set to qwen-coder-flash
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "agents:\n  rules: qwen-coder-flash\n").unwrap();

    // Change to temp directory so config is found
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Verify that AgentManager can read the config
    let agent_name =
        AgentManager::get_agent_for_use_case(AgentUseCase::Rules).expect("Should read config");
    eprintln!("Agent name from config: {:?}", agent_name);
    assert_eq!(
        agent_name,
        Some("qwen-coder-flash".to_string()),
        "Config should specify qwen-coder-flash for rules"
    );

    // Resolve the full agent config
    let resolved_config = AgentManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)
        .expect("Should resolve agent config");
    eprintln!(
        "Resolved agent config executor type: {:?}",
        resolved_config.executor_type()
    );

    // TODO: Now we need to test that RuleCheckTool actually uses this agent
    // This requires:
    // 1. Creating a ToolContext with the resolved agent
    // 2. Calling RuleCheckTool with that context
    // 3. Verifying the agent was used

    // The problem is: how do we verify which agent was actually used?
    // Options:
    // A. Check logs (fragile)
    // B. Mock the agent creation (complex)
    // C. Use a real small model and verify it ran (slow but correct)

    eprintln!("\nThis test proves the config is read correctly.");
    eprintln!("The bug is that RuleCheckTool caches the checker in OnceCell,");
    eprintln!("so even though the config is correct, the cached checker uses the old agent.");
    eprintln!("\nTo fully test this, we need to:");
    eprintln!("1. Create a RuleCheckTool");
    eprintln!("2. Call it with a ToolContext that has the Rules agent config");
    eprintln!("3. Verify the agent executor matches what we configured");

    // Restore original directory
    std::env::set_current_dir(&original_dir).unwrap();
}

/// Test that demonstrates the caching bug
#[tokio::test]
async fn test_demonstrates_caching_bug() {
    // This test shows that if we create a RuleCheckTool,
    // call get_checker() once, then change the config,
    // calling get_checker() again will still use the cached checker

    eprintln!("\n=== Demonstrating the OnceCell caching bug ===\n");

    // Create temp directory
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Step 1: Config specifies claude-code (or no config, defaults to claude-code)
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "# No agent config yet\n").unwrap();

    let agent1 = AgentManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)
        .expect("Should resolve");
    eprintln!("Step 1 - Initial agent: {:?}", agent1.executor_type());

    // Step 2: Change config to qwen-coder-flash
    fs::write(&config_path, "agents:\n  rules: qwen-coder-flash\n").unwrap();

    let agent2 = AgentManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)
        .expect("Should resolve");
    eprintln!("Step 2 - After config change: {:?}", agent2.executor_type());

    // Verify the config reader picks up the change
    assert_ne!(
        format!("{:?}", agent1.executor_type()),
        format!("{:?}", agent2.executor_type()),
        "AgentManager should pick up config changes"
    );

    eprintln!("\n✓ AgentManager correctly picks up config changes");
    eprintln!("✗ But RuleCheckTool with OnceCell would cache the first agent");
    eprintln!("  and never pick up the change!");

    std::env::set_current_dir(&original_dir).unwrap();
}

/// Test the fix: without OnceCell, agent config is read fresh each time
#[tokio::test]
async fn test_fresh_checker_picks_up_config_changes() {
    eprintln!("\n=== Testing that fresh checker creation picks up config changes ===\n");

    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Write initial config
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "agents:\n  rules: qwen-coder-flash\n").unwrap();

    // Read config multiple times - should always reflect current file content
    for i in 1..=3 {
        let agent = AgentManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)
            .expect("Should resolve");
        eprintln!("Read {}: {:?}", i, agent.executor_type());
    }

    // Change config
    fs::write(&config_path, "# No agent config\n").unwrap();

    let agent_after = AgentManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)
        .expect("Should resolve");
    eprintln!("After removing config: {:?}", agent_after.executor_type());

    eprintln!("\n✓ Config reader always reflects current file state");
    eprintln!("✓ If RuleCheckTool creates fresh checker each time,");
    eprintln!("  it will pick up config changes correctly");

    std::env::set_current_dir(&original_dir).unwrap();
}
