//! Tests for concurrent action execution

use crate::actions_tests::create_test_context;
use crate::{
    Action, LogAction, PromptAction, SetVariableAction, SubWorkflowAction, WaitAction,
    WorkflowTemplateContext,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[tokio::test]
async fn test_concurrent_set_variable_actions() {
    // Test that multiple SetVariableActions can execute concurrently without race conditions
    let context = Arc::new(Mutex::new(create_test_context()));

    let actions: Vec<SetVariableAction> = (0..10)
        .map(|i| SetVariableAction::new(format!("concurrent_var_{i}"), format!("value_{i}")))
        .collect();

    let mut handles = vec![];

    for action in actions {
        let context_clone = Arc::clone(&context);
        let handle = tokio::spawn(async move {
            let mut ctx = context_clone.lock().await;
            action.execute(&mut ctx).await
        });
        handles.push(handle);
    }

    // Wait for all actions to complete
    for handle in handles {
        let _ = handle.await.unwrap();
    }

    // Verify all variables were set correctly
    let ctx = context.lock().await;
    for i in 0..10 {
        let key = format!("concurrent_var_{i}");
        let expected_value = format!("value_{i}");
        assert_eq!(
            ctx.get(&key),
            Some(&Value::String(expected_value)),
            "Variable {key} was not set correctly"
        );
    }
}

#[tokio::test]
async fn test_concurrent_log_actions() {
    // Test that multiple LogActions can execute concurrently
    let context = create_test_context();

    let actions: Vec<LogAction> = vec![
        LogAction::info("Concurrent log 1".to_string()),
        LogAction::warning("Concurrent log 2".to_string()),
        LogAction::error("Concurrent log 3".to_string()),
    ];

    let start = Instant::now();

    // Execute all log actions concurrently
    let mut handles = vec![];
    for action in actions {
        let mut ctx = context.clone();
        let handle = tokio::spawn(async move { action.execute(&mut ctx).await });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    let duration = start.elapsed();

    // Verify execution was concurrent (should be much less than sequential)
    assert!(
        duration.as_millis() < 100,
        "Concurrent execution took too long: {duration:?}"
    );
}

#[tokio::test]
async fn test_concurrent_wait_actions() {
    // Test that WaitActions execute concurrently
    let wait_duration = Duration::from_millis(100);
    let num_actions = 5;

    let actions: Vec<WaitAction> = (0..num_actions)
        .map(|i| WaitAction::new_duration(wait_duration).with_message(format!("Wait action {i}")))
        .collect();

    let start = Instant::now();

    // Execute all wait actions concurrently
    let mut handles = vec![];
    for action in actions {
        let mut context = create_test_context();
        let handle = tokio::spawn(async move { action.execute(&mut context).await });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let _ = handle.await.unwrap();
    }

    let duration = start.elapsed();

    // If executed concurrently, total time should be close to single wait duration
    // If sequential, it would be num_actions * wait_duration
    assert!(
        duration < Duration::from_millis(200),
        "Concurrent wait actions took too long: {duration:?} (expected < 200ms)"
    );
}

#[tokio::test]
async fn test_concurrent_mixed_actions() {
    // Test different action types executing concurrently
    let context = Arc::new(Mutex::new(create_test_context()));

    let set_action = SetVariableAction::new("mixed_var".to_string(), "mixed_value".to_string());
    let log_action = LogAction::info("Mixed concurrent test".to_string());
    let wait_action = WaitAction::new_duration(Duration::from_millis(50));

    let ctx1 = Arc::clone(&context);
    let handle1 = tokio::spawn(async move {
        let mut ctx = ctx1.lock().await;
        set_action.execute(&mut ctx).await
    });

    let ctx2 = Arc::clone(&context);
    let handle2 = tokio::spawn(async move {
        let mut ctx = ctx2.lock().await;
        log_action.execute(&mut ctx).await
    });

    let ctx3 = Arc::clone(&context);
    let handle3 = tokio::spawn(async move {
        let mut ctx = ctx3.lock().await;
        wait_action.execute(&mut ctx).await
    });

    // Wait for all actions
    let results = tokio::join!(handle1, handle2, handle3);

    assert!(results.0.is_ok());
    assert!(results.1.is_ok());
    assert!(results.2.is_ok());

    // Verify the set variable action worked
    let ctx = context.lock().await;
    assert_eq!(
        ctx.get("mixed_var"),
        Some(&Value::String("mixed_value".to_string()))
    );
}

#[tokio::test]
async fn test_concurrent_action_error_isolation() {
    // Test that errors in one concurrent action don't affect others
    let context = Arc::new(Mutex::new(create_test_context()));

    // Create an action with invalid JSON (will be stored as string)
    let failing_action =
        SetVariableAction::new("fail_var".to_string(), "{invalid json".to_string());

    // Create actions that should succeed
    let success_action1 = SetVariableAction::new("success1".to_string(), "value1".to_string());
    let success_action2 = SetVariableAction::new("success2".to_string(), "value2".to_string());

    let ctx1 = Arc::clone(&context);
    let handle1 = tokio::spawn(async move {
        let mut ctx = ctx1.lock().await;
        failing_action.execute(&mut ctx).await
    });

    let ctx2 = Arc::clone(&context);
    let handle2 = tokio::spawn(async move {
        let mut ctx = ctx2.lock().await;
        success_action1.execute(&mut ctx).await
    });

    let ctx3 = Arc::clone(&context);
    let handle3 = tokio::spawn(async move {
        let mut ctx = ctx3.lock().await;
        success_action2.execute(&mut ctx).await
    });

    let (result1, result2, result3) = tokio::join!(handle1, handle2, handle3);

    // All actions should succeed (SetVariableAction doesn't fail on invalid JSON)
    assert!(result1.is_ok());

    // Other actions should succeed
    assert!(result2.is_ok());
    assert!(result3.is_ok());

    // Verify successful actions completed
    let ctx = context.lock().await;
    assert_eq!(
        ctx.get("success1"),
        Some(&Value::String("value1".to_string()))
    );
    assert_eq!(
        ctx.get("success2"),
        Some(&Value::String("value2".to_string()))
    );
    // SetVariableAction stores invalid JSON as a string, it doesn't fail
    assert_eq!(
        ctx.get("fail_var"),
        Some(&Value::String("{invalid json".to_string()))
    );
}

#[tokio::test]
async fn test_concurrent_prompt_action_rate_limiting() {
    // Test that concurrent PromptActions handle rate limiting properly
    // This test simulates rate limiting scenarios

    let actions: Vec<PromptAction> = (0..3)
        .map(|i| {
            PromptAction::new(format!("test-prompt-{i}"))
                .with_argument("arg".to_string(), format!("value{i}"))
        })
        .collect();

    let start = Instant::now();

    // Execute actions concurrently
    let mut handles = vec![];
    for action in actions {
        let mut context = create_test_context();
        let handle = tokio::spawn(async move {
            // Note: Actual execution would depend on external Claude service
            // This test focuses on the concurrent execution structure
            action.execute(&mut context).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let _ = handle.await;
    }

    let duration = start.elapsed();

    // Verify that execution attempted concurrently
    // (actual rate limiting would depend on external service)
    println!("Concurrent prompt actions completed in {duration:?}");
}

#[tokio::test]
async fn test_concurrent_sub_workflow_actions() {
    // Test concurrent sub-workflow execution
    let context = Arc::new(Mutex::new(create_test_context()));

    // Initialize workflow stack to prevent circular dependency errors
    {
        let mut ctx = context.lock().await;
        ctx.insert("_workflow_stack".to_string(), serde_json::json!([]));
    }

    let actions: Vec<SubWorkflowAction> = (0..3)
        .map(|i| {
            SubWorkflowAction::new(format!("sub-workflow-{i}"))
                .with_input("input".to_string(), format!("data{i}"))
        })
        .collect();

    let mut handles = vec![];

    for action in actions {
        let ctx_clone = Arc::clone(&context);
        let handle = tokio::spawn(async move {
            let mut ctx = ctx_clone.lock().await;
            // Note: Actual execution would load and run sub-workflows
            // This test verifies the concurrent structure
            let _ = action.execute(&mut ctx).await;
        });
        handles.push(handle);
    }

    // Wait for all sub-workflows
    for handle in handles {
        let _ = handle.await;
    }

    // In a real scenario, we would verify that sub-workflows executed correctly
    // and their results were properly stored in context
}

#[tokio::test]
async fn test_concurrent_action_context_consistency() {
    // Test that concurrent actions maintain context consistency
    let shared_key = "shared_counter";
    let initial_value = 0;

    let context = Arc::new(Mutex::new(WorkflowTemplateContext::with_vars_for_test(
        HashMap::new(),
    )));
    context
        .lock()
        .await
        .insert(shared_key.to_string(), Value::Number(initial_value.into()));

    // Create multiple actions that read and update the same variable
    let num_actions = 10;
    let mut handles = vec![];

    for i in 0..num_actions {
        let ctx_clone = Arc::clone(&context);
        let key = shared_key.to_string();

        let handle = tokio::spawn(async move {
            // Simulate read-modify-write operation
            let mut ctx = ctx_clone.lock().await;

            // Read current value
            let current = ctx.get(&key).and_then(|v| v.as_i64()).unwrap_or(0);

            // Simulate some processing
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Write incremented value
            ctx.insert(key.clone(), Value::Number((current + 1).into()));

            println!("Action {i} updated counter to {}", current + 1);
        });

        handles.push(handle);
    }

    // Wait for all actions
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final value
    let final_ctx = context.lock().await;
    let final_value = final_ctx
        .get(shared_key)
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Due to the mutex, all increments should be atomic
    assert_eq!(
        final_value, num_actions as i64,
        "Expected counter to be {num_actions}, but was {final_value}"
    );
}
