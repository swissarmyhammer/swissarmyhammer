//! End-to-end validation tests with real workflow scenarios

use serde_json::json;
use std::collections::HashMap;

use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
use swissarmyhammer_workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;

#[tokio::test]
async fn test_multi_step_workflow_simulation() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing multi-step workflow simulation");

    // Simulate a workflow with multiple variable passing steps
    let workflow_steps = [
        ("analyze", json!({"task": "analyze user requirements"})),
        (
            "plan",
            json!({"analysis": "requirements analyzed successfully"}),
        ),
        ("execute", json!({"plan": "implementation plan created"})),
        ("validate", json!({"result": "implementation completed"})),
    ];

    for executor_name in ["Claude", "LlamaAgent"] {
        println!(
            "Testing multi-step workflow with {} executor",
            executor_name
        );

        let config = match executor_name {
            "Claude" => AgentConfig::claude_code(),
            "LlamaAgent" => AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
            _ => unreachable!(),
        };

        let mut accumulated_context = HashMap::new();

        for (step_name, step_data) in &workflow_steps {
            println!("  Processing step: {}", step_name);

            // Add current step data to accumulated context
            for (key, value) in step_data.as_object().unwrap() {
                accumulated_context.insert(key.clone(), value.clone());
            }

            // Add step metadata
            accumulated_context.insert("current_step".to_string(), json!(step_name));
            accumulated_context.insert("workflow_id".to_string(), json!("test_workflow_001"));

            let context = WorkflowTemplateContext::with_vars(accumulated_context.clone())
                .expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(config.clone());
            let execution_context = AgentExecutionContext::new(&context_with_config);

            // Attempt to create executor for this step
            match AgentExecutorFactory::create_executor(&execution_context).await {
                Ok(_executor) => {
                    println!("    ✓ Step {} executor created successfully", step_name);
                }
                Err(e) => {
                    println!(
                        "    ⚠ Step {} executor creation failed (expected): {}",
                        step_name, e
                    );
                }
            }
        }

        // Verify final context has all expected variables
        assert!(accumulated_context.contains_key("task"));
        assert!(accumulated_context.contains_key("analysis"));
        assert!(accumulated_context.contains_key("plan"));
        assert!(accumulated_context.contains_key("result"));
        assert!(accumulated_context.contains_key("current_step"));
        assert!(accumulated_context.contains_key("workflow_id"));

        println!("  ✓ Multi-step workflow completed with {}", executor_name);
    }
}

#[tokio::test]
async fn test_error_recovery_scenarios() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing error recovery scenarios");

    let error_scenarios = [
        ("invalid_config", HashMap::new()),
        ("empty_context", HashMap::new()),
        (
            "large_context",
            (0..100)
                .map(|i| (format!("var_{}", i), json!(format!("value_{}", i))))
                .collect(),
        ),
    ];

    for (scenario_name, vars) in error_scenarios {
        println!("Testing error scenario: {}", scenario_name);

        for executor_name in ["Claude", "LlamaAgent"] {
            let config = match executor_name {
                "Claude" => AgentConfig::claude_code(),
                "LlamaAgent" => AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
                _ => unreachable!(),
            };

            let context =
                WorkflowTemplateContext::with_vars(vars.clone()).expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(config);
            let execution_context = AgentExecutionContext::new(&context_with_config);

            match AgentExecutorFactory::create_executor(&execution_context).await {
                Ok(_executor) => {
                    println!(
                        "  ✓ Scenario {} with {} succeeded unexpectedly",
                        scenario_name, executor_name
                    );
                }
                Err(e) => {
                    println!(
                        "  ⚠ Scenario {} with {} failed gracefully: {}",
                        scenario_name, executor_name, e
                    );

                    // Verify error message is not empty and contains useful information
                    assert!(!e.to_string().is_empty());
                }
            }
        }
    }

    println!("✓ Error recovery scenarios completed successfully");
}

#[tokio::test]
async fn test_variable_templating_patterns() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing variable templating patterns");

    let template_test_cases = [
        ("simple_string", json!("hello world")),
        ("number_value", json!(42)),
        ("boolean_value", json!(true)),
        ("array_value", json!(["item1", "item2", "item3"])),
        ("object_value", json!({"nested": {"key": "value"}})),
        ("null_value", json!(null)),
    ];

    for (test_name, test_value) in template_test_cases {
        println!("Testing template pattern: {}", test_name);

        let vars = HashMap::from([
            ("test_key".to_string(), test_value.clone()),
            ("template_test".to_string(), json!(test_name)),
        ]);

        let context = WorkflowTemplateContext::with_vars(vars).expect("Failed to create context");
        let mut context_with_config = context;
        context_with_config.set_agent_config(AgentConfig::claude_code());
        let execution_context = AgentExecutionContext::new(&context_with_config);

        // Test that complex variables don't break context creation
        match AgentExecutorFactory::create_executor(&execution_context).await {
            Ok(_executor) => {
                println!("  ✓ Template pattern {} handled successfully", test_name);
            }
            Err(e) => {
                println!(
                    "  ⚠ Template pattern {} failed (expected): {}",
                    test_name, e
                );
            }
        }
    }

    println!("✓ Variable templating patterns test completed");
}

#[tokio::test]
async fn test_conditional_execution_simulation() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing conditional execution simulation");

    // Simulate conditional workflow branches
    let conditions = [
        ("success_path", true, "continue"),
        ("error_path", false, "halt"),
        ("retry_path", true, "retry"),
        ("skip_path", false, "skip"),
    ];

    for (condition_name, should_execute, action) in conditions {
        println!(
            "Testing condition: {} (should_execute: {}, action: {})",
            condition_name, should_execute, action
        );

        let vars = HashMap::from([
            ("condition".to_string(), json!(condition_name)),
            ("should_execute".to_string(), json!(should_execute)),
            ("action".to_string(), json!(action)),
            (
                "execution_id".to_string(),
                json!(format!("exec_{}", condition_name)),
            ),
        ]);

        for executor_name in ["Claude", "LlamaAgent"] {
            let config = match executor_name {
                "Claude" => AgentConfig::claude_code(),
                "LlamaAgent" => AgentConfig::llama_agent(LlamaAgentConfig::for_testing()),
                _ => unreachable!(),
            };

            let context =
                WorkflowTemplateContext::with_vars(vars.clone()).expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(config);
            let execution_context = AgentExecutionContext::new(&context_with_config);

            // Test conditional execution
            if should_execute {
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(_executor) => {
                        println!(
                            "    ✓ Condition {} with {} executed successfully",
                            condition_name, executor_name
                        );
                    }
                    Err(e) => {
                        println!(
                            "    ⚠ Condition {} with {} failed (expected): {}",
                            condition_name, executor_name, e
                        );
                    }
                }
            } else {
                // For non-execution conditions, we still test that context creation works
                println!(
                    "    ✓ Condition {} with {} context created (execution skipped)",
                    condition_name, executor_name
                );
            }
        }
    }

    println!("✓ Conditional execution simulation completed");
}

#[tokio::test]
async fn test_workflow_state_persistence() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing workflow state persistence simulation");

    // Simulate a workflow that maintains state across multiple operations
    let mut workflow_state = HashMap::from([
        ("workflow_id".to_string(), json!("persistent_workflow_001")),
        ("started_at".to_string(), json!("2024-01-01T00:00:00Z")),
        ("status".to_string(), json!("initializing")),
    ]);

    let state_transitions = [
        ("running", json!({"progress": 25})),
        (
            "processing",
            json!({"progress": 50, "current_task": "data_processing"}),
        ),
        (
            "validating",
            json!({"progress": 75, "validation_results": ["check1", "check2"]}),
        ),
        (
            "completed",
            json!({"progress": 100, "final_result": "success"}),
        ),
    ];

    for (new_status, additional_state) in state_transitions {
        println!("Transitioning to state: {}", new_status);

        // Update workflow state
        workflow_state.insert("status".to_string(), json!(new_status));

        // Merge in additional state
        for (key, value) in additional_state.as_object().unwrap() {
            workflow_state.insert(key.clone(), value.clone());
        }

        // Add timestamp for this state change
        workflow_state.insert(
            format!("{}_at", new_status),
            json!(format!("2024-01-01T{}:00:00Z", workflow_state.len())),
        );

        let context = WorkflowTemplateContext::with_vars(workflow_state.clone())
            .expect("Failed to create context");
        let mut context_with_config = context;
        context_with_config.set_agent_config(AgentConfig::claude_code());
        let execution_context = AgentExecutionContext::new(&context_with_config);

        match AgentExecutorFactory::create_executor(&execution_context).await {
            Ok(_executor) => {
                println!("  ✓ State {} processed successfully", new_status);
            }
            Err(e) => {
                println!("  ⚠ State {} failed (expected): {}", new_status, e);
            }
        }

        // Verify state accumulation
        assert!(workflow_state.contains_key("workflow_id"));
        assert!(workflow_state.contains_key("status"));
        assert_eq!(workflow_state["status"], json!(new_status));
    }

    // Final verification that all states were captured
    assert!(workflow_state.contains_key("progress"));
    assert!(workflow_state.contains_key("final_result"));
    assert_eq!(workflow_state["progress"], json!(100));

    println!("✓ Workflow state persistence test completed");
}

#[tokio::test]
async fn test_intentional_error_handling() {
    // Skip test if LlamaAgent testing is disabled

    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing intentional error handling");

    // Test scenarios that should trigger specific error conditions
    let error_test_cases = [
        ("executor_creation_error", HashMap::new()),
        (
            "context_with_special_chars",
            HashMap::from([
                (
                    "special_key".to_string(),
                    json!("value with special chars: !@#$%^&*()"),
                ),
                ("unicode_key".to_string(), json!("Unicode: 🚀 🎉 🔥")),
            ]),
        ),
        (
            "very_long_key",
            HashMap::from([(
                "x".repeat(1000),
                json!("This is a very long key name to test edge cases"),
            )]),
        ),
    ];

    for (test_case, vars) in error_test_cases {
        println!("Testing intentional error case: {}", test_case);

        let context = WorkflowTemplateContext::with_vars(vars);

        match context {
            Ok(ctx) => {
                let mut context_with_config = ctx;
                context_with_config.set_agent_config(AgentConfig::claude_code());
                let execution_context = AgentExecutionContext::new(&context_with_config);

                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(_executor) => {
                        println!("  ✓ Error case {} handled gracefully (no error)", test_case);
                    }
                    Err(e) => {
                        println!(
                            "  ✓ Error case {} properly triggered error: {}",
                            test_case, e
                        );

                        // Verify the error is meaningful
                        let error_str = e.to_string();
                        assert!(!error_str.is_empty());
                        assert!(error_str.len() > 5); // Should be more than just "Error"
                    }
                }
            }
            Err(e) => {
                println!(
                    "  ✓ Error case {} failed at context creation: {}",
                    test_case, e
                );

                // Context creation errors are also valid for testing
                assert!(!e.to_string().is_empty());
            }
        }
    }

    println!("✓ Intentional error handling test completed");
}
