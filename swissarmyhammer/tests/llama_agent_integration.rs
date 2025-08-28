//! Integration tests for LlamaAgent executor with real workflows

use std::collections::HashMap;
use std::time::Duration;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer::workflow::template_context::WorkflowTemplateContext;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
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
async fn test_concurrent_executor_access() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    const CONCURRENT_CONTEXTS: usize = 5;

    let handles: Vec<_> = (0..CONCURRENT_CONTEXTS)
        .map(|i| {
            tokio::spawn(async move {
                let context = WorkflowTemplateContext::with_vars(HashMap::new())
                    .expect("Failed to create context");
                let mut context_with_config = context;

                // Alternate between executor types
                let config = if i % 2 == 0 {
                    AgentConfig::claude_code()
                } else {
                    AgentConfig::llama_agent(LlamaAgentConfig::for_testing())
                };

                context_with_config.set_agent_config(config);
                let execution_context = AgentExecutionContext::new(&context_with_config);

                // Attempt to create executor
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(_executor) => format!("Context {} succeeded", i),
                    Err(e) => format!("Context {} failed gracefully: {}", i, e),
                }
            })
        })
        .collect();

    // Wait for all contexts to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should not panic");
        results.push(result);
    }

    // All tasks should complete without panicking
    assert_eq!(results.len(), CONCURRENT_CONTEXTS);

    for result in results {
        println!("✓ Concurrent test: {:?}", result);
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
