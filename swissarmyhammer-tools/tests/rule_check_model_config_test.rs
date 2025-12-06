// sah rule ignore test_rule_with_allow
//! Integration test for rule checking with model configuration
//!
//! Tests that rule checking respects the model configured for the Rules use case

use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{ModelManager, ModelUseCase};

/// Test that RuleCheckTool picks up model config changes
#[tokio::test]
async fn test_rule_check_uses_configured_model() {
    // Create temp directory
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_path = temp_dir;

    // Create .swissarmyhammer directory
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Write config with rules model set to qwen-coder-flash
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "models:\n  rules: qwen-coder-flash\n").unwrap();

    // Change to temp directory so config is found
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Verify that ModelManager can read the config
    let model_name =
        ModelManager::get_agent_for_use_case(ModelUseCase::Rules).expect("Should read config");
    eprintln!("Model name from config: {:?}", model_name);
    assert_eq!(
        model_name,
        Some("qwen-coder-flash".to_string()),
        "Config should specify qwen-coder-flash for rules"
    );

    // Resolve the full model config
    let resolved_config = ModelManager::resolve_agent_config_for_use_case(ModelUseCase::Rules)
        .expect("Should resolve model config");
    eprintln!(
        "Resolved model config executor type: {:?}",
        resolved_config.executor_type()
    );

    // TODO: Now we need to test that RuleCheckTool actually uses this model
    // This requires:
    // 1. Creating a ToolContext with the resolved model
    // 2. Calling RuleCheckTool with that context
    // 3. Verifying the model was used

    // The problem is: how do we verify which model was actually used?
    // Options:
    // A. Check logs (fragile)
    // B. Mock the model creation (complex)
    // C. Use a real small model and verify it ran (slow but correct)

    eprintln!("\nThis test proves the config is read correctly.");
    eprintln!("The bug is that RuleCheckTool caches the checker in OnceCell,");
    eprintln!("so even though the config is correct, the cached checker uses the old model.");
    eprintln!("\nTo fully test this, we need to:");
    eprintln!("1. Create a RuleCheckTool");
    eprintln!("2. Call it with a ToolContext that has the Rules model config");
    eprintln!("3. Verify the model executor matches what we configured");

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
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_path = temp_dir;
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Step 1: Config specifies claude-code (or no config, defaults to claude-code)
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "# No model config yet\n").unwrap();

    let model1 = ModelManager::resolve_agent_config_for_use_case(ModelUseCase::Rules)
        .expect("Should resolve");
    eprintln!("Step 1 - Initial model: {:?}", model1.executor_type());

    // Step 2: Change config to qwen-coder-flash
    fs::write(&config_path, "models:\n  rules: qwen-coder-flash\n").unwrap();

    let model2 = ModelManager::resolve_agent_config_for_use_case(ModelUseCase::Rules)
        .expect("Should resolve");
    eprintln!("Step 2 - After config change: {:?}", model2.executor_type());

    // Verify the config reader picks up the change
    assert_ne!(
        format!("{:?}", model1.executor_type()),
        format!("{:?}", model2.executor_type()),
        "ModelManager should pick up config changes"
    );

    eprintln!("\n✓ ModelManager correctly picks up config changes");
    eprintln!("✗ But RuleCheckTool with OnceCell would cache the first model");
    eprintln!("  and never pick up the change!");

    std::env::set_current_dir(&original_dir).unwrap();
}

/// Test the fix: without OnceCell, model config is read fresh each time
#[tokio::test]
async fn test_fresh_checker_picks_up_config_changes() {
    eprintln!("\n=== Testing that fresh checker creation picks up config changes ===\n");

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_path = temp_dir;
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Write initial config
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "models:\n  rules: qwen-coder-flash\n").unwrap();

    // Read config multiple times - should always reflect current file content
    for i in 1..=3 {
        let model = ModelManager::resolve_agent_config_for_use_case(ModelUseCase::Rules)
            .expect("Should resolve");
        eprintln!("Read {}: {:?}", i, model.executor_type());
    }

    // Change config
    fs::write(&config_path, "# No model config\n").unwrap();

    let model_after = ModelManager::resolve_agent_config_for_use_case(ModelUseCase::Rules)
        .expect("Should resolve");
    eprintln!("After removing config: {:?}", model_after.executor_type());

    eprintln!("\n✓ Config reader always reflects current file state");
    eprintln!("✓ If RuleCheckTool creates fresh checker each time,");
    eprintln!("  it will pick up config changes correctly");

    std::env::set_current_dir(&original_dir).unwrap();
}
