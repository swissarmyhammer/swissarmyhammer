// sah rule ignore test_rule_with_allow
use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::ModelManager;
use swissarmyhammer_config::AgentUseCase;

#[test]
fn test_resolve_rules_model_from_config() {
    // Create temp directory
    let _env = IsolatedTestEnvironment::new().unwrap();
    let temp_dir = _env.temp_dir();
    let temp_path = temp_dir;

    // Create .swissarmyhammer directory
    let sah_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Write config with rules agent
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, "agents:\n  rules: qwen-next\n").unwrap();

    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_path).unwrap();

    // Test get_agent_for_use_case
    let result = ModelManager::get_agent_for_use_case(AgentUseCase::Rules);
    eprintln!("get_agent_for_use_case result: {:?}", result);

    match result {
        Ok(Some(agent_name)) => {
            eprintln!("Found agent: {}", agent_name);
            assert_eq!(agent_name, "qwen-next", "Should resolve to qwen-next");
        }
        Ok(None) => {
            panic!(
                "Expected agent name but got None. Config file exists at: {}",
                config_path.display()
            );
        }
        Err(e) => {
            panic!(
                "Error getting agent: {}. Config path: {}",
                e,
                config_path.display()
            );
        }
    }

    // Test resolve_agent_config_for_use_case
    let config_result = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules);
    eprintln!(
        "resolve_agent_config_for_use_case result: {:?}",
        config_result
    );

    match config_result {
        Ok(config) => {
            eprintln!("Resolved agent config: {:?}", config.executor_type());
            // The config should be for qwen-next
        }
        Err(e) => {
            eprintln!("Error resolving agent config: {}", e);
            panic!("Failed to resolve agent config");
        }
    }

    // Restore original directory
    std::env::set_current_dir(&original_dir).unwrap();
}
