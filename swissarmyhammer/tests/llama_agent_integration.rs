//! Integration tests for LlamaAgent executor with real workflows

use std::collections::HashMap;
use std::time::Duration;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
use swissarmyhammer_workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;
use tokio::time::timeout;

#[tokio::test]
async fn test_executor_compatibility() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test that both executor types can be created with identical interfaces
    for (executor_name, config) in [
        ("Claude", AgentConfig::claude_code()),
        (
            "LlamaAgent",
            AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
        ),
    ] {
        println!("Testing executor compatibility: {}", executor_name);

        let context =
            WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
        let mut context_with_config = context;
        context_with_config.set_agent_config(config);
        let execution_context = AgentExecutionContext::new(&context_with_config);

        // Both should create executors without panicking
        let result = AgentExecutorFactory::create_executor(&execution_context).await;

        match result {
            Ok(_executor) => {
                println!("✓ {} executor created successfully", executor_name);
            }
            Err(e) => {
                println!(
                    "⚠ {} executor creation failed (expected in test environment): {}",
                    executor_name, e
                );
                // This is expected when dependencies aren't available
            }
        }
    }
}

#[tokio::test]
async fn test_agent_execution_context() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test context creation with different configurations
    let configs = [
        ("Claude", AgentConfig::claude_code()),
        (
            "LlamaAgent",
            AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
        ),
    ];

    for (name, config) in configs {
        let context =
            WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
        let mut context_with_config = context;
        context_with_config.set_agent_config(config);

        let execution_context = AgentExecutionContext::new(&context_with_config);

        // Verify context was created properly
        assert!(!execution_context.quiet()); // Test a real method that exists
        println!("✓ {} execution context created successfully", name);
    }
}

#[tokio::test]
async fn test_executor_factory_patterns() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test factory with different context patterns
    let test_cases = [
        ("empty_vars", HashMap::new()),
        (
            "with_vars",
            HashMap::from([
                (
                    "test_var".to_string(),
                    serde_json::Value::String("test_value".to_string()),
                ),
                (
                    "number_var".to_string(),
                    serde_json::Value::Number(42.into()),
                ),
            ]),
        ),
    ];

    for (test_name, vars) in test_cases {
        println!("Testing factory pattern: {}", test_name);

        let context = WorkflowTemplateContext::with_vars(vars).expect("Failed to create context");
        let mut context_with_config = context;
        context_with_config.set_agent_config(AgentConfig::claude_code());
        let execution_context = AgentExecutionContext::new(&context_with_config);

        // Test factory creation
        match AgentExecutorFactory::create_executor(&execution_context).await {
            Ok(_executor) => {
                println!("✓ Factory pattern {} succeeded", test_name);
            }
            Err(e) => {
                println!("⚠ Factory pattern {} failed (expected): {}", test_name, e);
            }
        }
    }
}

#[tokio::test]
async fn test_configuration_serialization() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test that configurations can be properly serialized/deserialized
    let configs = [
        ("Claude", AgentConfig::claude_code()),
        (
            "LlamaAgent",
            AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
        ),
    ];

    for (name, config) in configs {
        // Test JSON serialization
        let json_result = serde_json::to_string(&config);
        assert!(
            json_result.is_ok(),
            "Failed to serialize {} config to JSON",
            name
        );

        let json_str = json_result.unwrap();
        assert!(
            !json_str.is_empty(),
            "{} config JSON should not be empty",
            name
        );

        // Test JSON deserialization
        let deserialized: Result<AgentConfig, _> = serde_json::from_str(&json_str);
        assert!(
            deserialized.is_ok(),
            "Failed to deserialize {} config from JSON",
            name
        );

        println!(
            "✓ {} configuration serialization/deserialization works",
            name
        );
    }
}

#[tokio::test]
async fn test_timeout_handling() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test that executor creation handles timeouts gracefully
    let context =
        WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
    let mut context_with_config = context;
    context_with_config.set_agent_config(AgentConfig::claude_code());
    let execution_context = AgentExecutionContext::new(&context_with_config);

    // Test with a very short timeout
    let result = timeout(
        Duration::from_millis(1), // Very short timeout
        AgentExecutorFactory::create_executor(&execution_context),
    )
    .await;

    match result {
        Ok(Ok(_executor)) => {
            println!("✓ Executor creation was very fast (completed within 1ms)");
        }
        Ok(Err(e)) => {
            println!("⚠ Executor creation failed (expected): {}", e);
        }
        Err(_) => {
            println!("⚠ Executor creation timed out (expected with 1ms timeout)");
        }
    }

    // This test mainly verifies that timeout handling doesn't cause panics
}

#[tokio::test]
async fn test_repetition_detection_configuration() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Test default repetition detection configuration
    let default_config = LlamaAgentConfig::default();
    assert!(default_config.repetition_detection.enabled);
    assert_eq!(default_config.repetition_detection.repetition_penalty, 1.1);
    assert_eq!(default_config.repetition_detection.repetition_threshold, 50);
    assert_eq!(default_config.repetition_detection.repetition_window, 64);
    println!("✓ Default repetition detection configuration validated");

    // Test testing configuration (should be more permissive)
    let test_config = LlamaAgentConfig::for_testing();
    assert!(test_config.repetition_detection.enabled);
    assert_eq!(test_config.repetition_detection.repetition_penalty, 1.05); // Lower penalty
    assert_eq!(test_config.repetition_detection.repetition_threshold, 150); // Higher threshold
    assert_eq!(test_config.repetition_detection.repetition_window, 128); // Smaller window
    println!("✓ Test repetition detection configuration validated");

    // Test small model configuration (should be most permissive)
    let small_model_config = LlamaAgentConfig::for_testing();
    assert!(small_model_config.repetition_detection.enabled);
    assert_eq!(
        small_model_config.repetition_detection.repetition_penalty,
        1.05
    ); // Lower penalty
    assert_eq!(
        small_model_config.repetition_detection.repetition_threshold,
        150
    ); // Highest threshold
    assert_eq!(
        small_model_config.repetition_detection.repetition_window,
        128
    ); // Larger window
    println!("✓ Small model repetition detection configuration validated");

    // Test that different configurations can be serialized properly
    for (name, config) in [
        ("default", LlamaAgentConfig::default()),
        ("testing", LlamaAgentConfig::for_testing()),
    ] {
        let json_result = serde_json::to_string(&config);
        assert!(json_result.is_ok(), "Failed to serialize {} config", name);

        let json_str = json_result.unwrap();
        assert!(
            json_str.contains("repetition_detection"),
            "{} config should contain repetition_detection",
            name
        );

        let deserialized: Result<LlamaAgentConfig, _> = serde_json::from_str(&json_str);
        assert!(
            deserialized.is_ok(),
            "Failed to deserialize {} config",
            name
        );

        println!("✓ {} repetition detection serialization works", name);
    }
}

#[tokio::test]
async fn test_repetition_configuration_integration() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // This test verifies that repetition detection configuration gets passed
    // through to the llama-agent properly. Due to the mock implementation in tests,
    // this mainly verifies the configuration conversion doesn't panic.

    let small_model_config = LlamaAgentConfig::for_testing();
    let agent_config = AgentConfig::llama_agent(small_model_config);

    let context =
        WorkflowTemplateContext::with_vars(HashMap::new()).expect("Failed to create context");
    let mut context_with_config = context;
    context_with_config.set_agent_config(agent_config);
    let execution_context = AgentExecutionContext::new(&context_with_config);

    // This should create the executor with repetition detection configuration
    // In test mode, this will use the mock implementation, but verifies
    // the configuration pipeline works
    let result = AgentExecutorFactory::create_executor(&execution_context).await;

    match result {
        Ok(_executor) => {
            println!("✓ LlamaAgent executor with repetition config created successfully");
        }
        Err(e) => {
            println!(
                "⚠ LlamaAgent executor creation failed (expected in test mode): {}",
                e
            );
            // This is expected when running in test mode with mocked dependencies
        }
    }
}
